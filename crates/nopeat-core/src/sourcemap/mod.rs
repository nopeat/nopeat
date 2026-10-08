use std::collections::HashMap;
use std::io::Read;

use crate::{Error, Result};

mod vlq;

pub use vlq::{decode, decode_vlq};

use vlq::base_offset;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mapping {
    pub generated_offset: u32,
    pub source_index: u32,
    pub original_line: u32,
    pub original_column: u32,
    pub name_index: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedSourceMap {
    pub file: Option<String>,
    pub sources: Vec<String>,
    pub names: Vec<String>,
    pub sources_content: Vec<Option<String>>,

    pub mappings: Vec<Mapping>,
}

#[derive(serde::Deserialize, Default)]
struct RawMap {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    sources: Vec<String>,
    #[serde(default, rename = "sourcesContent")]
    sources_content: Vec<Option<String>>,
    #[serde(default)]
    names: Vec<String>,
    #[serde(default)]
    mappings: String,
    #[serde(default)]
    sections: Vec<RawSection>,
}

#[derive(serde::Deserialize, Default)]
struct RawSection {
    #[serde(default)]
    offset: RawOffset,
    #[serde(default)]
    map: Box<RawMap>,
}

#[derive(serde::Deserialize, Default)]
struct RawOffset {
    #[serde(default)]
    line: u32,
    #[serde(default)]
    #[allow(dead_code)]
    column: u32,
}

pub fn parse_reader<R: Read>(reader: R) -> Result<ParsedSourceMap> {
    let raw: RawMap = serde_json::from_reader(std::io::BufReader::with_capacity(
        1 << 20,
        crate::bom::BomSkip::new(reader),
    ))
    .map_err(Error::Json)?;

    if raw.version != 0 && raw.version != 3 {
        return Err(Error::Unsupported(format!(
            "source map version {} (only v3 is supported)",
            raw.version
        )));
    }

    if !raw.sections.is_empty() {
        let mut merged = ParsedSourceMap { file: raw.file, ..ParsedSourceMap::default() };
        for section in raw.sections {
            let base = base_offset(&section.map.mappings, section.offset.line);
            let sub = decode(&section.map.mappings, section.map.sources.len())?;

            let source_base = u32::try_from(merged.sources.len()).unwrap_or(u32::MAX);
            let mut sub = remap_indices(sub, section.map.names.len(), source_base);
            for m in &mut sub {
                m.generated_offset = m.generated_offset.saturating_add(base);
            }
            merged.sources.extend(section.map.sources);
            merged.names.extend(section.map.names);
            merged.sources_content.extend(section.map.sources_content);
            merged.mappings.extend(sub);
        }
        merged.mappings.sort_unstable_by_key(|m| m.generated_offset);
        return Ok(merged);
    }

    if raw.mappings.is_empty() {
        return Ok(ParsedSourceMap {
            file: raw.file,
            sources: raw.sources,
            names: raw.names,
            sources_content: raw.sources_content,
            mappings: Vec::new(),
        });
    }

    let mappings = decode(&raw.mappings, raw.sources.len())?;
    Ok(ParsedSourceMap {
        file: raw.file,
        sources: raw.sources,
        names: raw.names,
        sources_content: raw.sources_content,
        mappings,
    })
}

pub fn parse_bytes(bytes: &[u8]) -> Result<ParsedSourceMap> {
    parse_reader(bytes)
}

fn remap_indices(mut mappings: Vec<Mapping>, name_count: usize, source_base: u32) -> Vec<Mapping> {
    for m in &mut mappings {
        m.source_index = m.source_index.saturating_add(source_base);
        if m.name_index.is_some() && name_count == 0 {
            m.name_index = None;
        }
    }
    mappings
}

impl ParsedSourceMap {
    pub fn attribute_by_source(&self) -> Vec<(usize, u64)> {
        self.attribute_by_source_in(0)
    }

    pub fn attribute_by_source_in(&self, generated_len: u64) -> Vec<(usize, u64)> {
        let mut out = vec![0u64; self.sources.len()];
        for pair in self.mappings.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let len = u64::from(b.generated_offset.saturating_sub(a.generated_offset));
            if len > 0
                && let Some(slot) = out.get_mut(a.source_index as usize)
            {
                *slot += len;
            }
        }
        if let Some(last) = self.mappings.last() {
            let tail = generated_len.saturating_sub(u64::from(last.generated_offset));
            if tail > 0
                && let Some(slot) = out.get_mut(last.source_index as usize)
            {
                *slot += tail;
            }
        }
        out.into_iter().enumerate().filter(|(_, b)| *b > 0).collect()
    }

    pub fn attribution_by_path(&self, map_dir: Option<&std::path::Path>) -> HashMap<String, u64> {
        self.attribution_by_path_in(map_dir, 0)
    }

    pub fn attribution_by_path_in(
        &self,
        map_dir: Option<&std::path::Path>,
        generated_len: u64,
    ) -> HashMap<String, u64> {
        let mut out = HashMap::new();
        for (index, bytes) in self.attribute_by_source_in(generated_len) {
            let Some(raw) = self.sources.get(index) else { continue };
            out.insert(normalise_source_path(raw, map_dir), bytes);
        }
        out
    }

    pub fn total_generated_bytes(&self) -> u64 {
        self.mappings.last().map_or(0, |m| u64::from(m.generated_offset))
    }

    pub fn content_for(&self, index: usize) -> Option<&str> {
        self.sources_content.get(index).and_then(|c| c.as_deref())
    }
}

pub fn normalise_source_path(raw: &str, map_dir: Option<&std::path::Path>) -> String {
    let mut path = raw.replace('\\', "/");
    if let Some(rest) = path.strip_prefix("file://") {
        path = rest.to_string();
    }

    if let Some(idx) = path.find("://") {
        let after_scheme = &path[idx + 3..];
        path = match after_scheme.find('/') {
            Some(slash) => after_scheme[slash + 1..].to_string(),
            None => after_scheme.to_string(),
        };
    }
    if let Some(dir) = map_dir {
        let prefix = dir.to_string_lossy().replace('\\', "/");
        if let Some(rest) = path.strip_prefix(&format!("{prefix}/")) {
            path = rest.to_string();
        }
    }
    while let Some(rest) = path.strip_prefix("./") {
        path = rest.to_string();
    }
    while let Some(rest) = path.strip_prefix("../") {
        path = rest.to_string();
    }
    path
}

#[cfg(test)]
mod tests {
    use super::{ParsedSourceMap, decode, decode_vlq, normalise_source_path, parse_bytes};

    const TINY: &str = r#"{
      "version": 3,
      "file": "out.js",
      "sources": ["a.ts", "b.ts"],
      "sourcesContent": ["const a = 1;", "const b = 2;"],
      "names": [],
      "mappings": "AAAA,IACE;AACD"
    }"#;

    #[test]
    fn decodes_the_canonical_example() {
        let map = parse_bytes(TINY.as_bytes()).unwrap();
        assert_eq!(map.sources, vec!["a.ts", "b.ts"]);
        assert_eq!(map.mappings.len(), 3);
        assert_eq!(map.mappings[0].generated_offset, 0);
        assert_eq!(map.mappings[0].source_index, 0);
        assert_eq!(map.mappings[0].original_line, 0);
        assert_eq!(map.mappings[1].generated_offset, 4);
        assert_eq!(map.mappings[1].source_index, 0, "'IACE' has a zero source delta");
        assert_eq!(map.mappings[1].original_line, 1);
        assert_eq!(map.mappings[2].generated_offset, 5, "the newline advances by one");
        assert_eq!(map.mappings[2].source_index, 0);
        assert_eq!(map.mappings[2].original_line, 2);
    }

    #[test]
    fn fields_are_positional_not_colon_separated() {
        let raw = r#"{"version":3,"file":"o.js","sources":["a","b"],
                      "sourcesContent":[],"names":[],"mappings":"AAAA,UCAA,UDAA"}"#;
        let map = parse_bytes(raw.as_bytes()).unwrap();
        assert_eq!(map.mappings.len(), 3);
        assert_eq!(map.mappings[0].generated_offset, 0);
        assert_eq!(map.mappings[1].generated_offset, 10);
        assert_eq!(map.mappings[1].source_index, 1);
        assert_eq!(map.mappings[2].generated_offset, 20);
        assert_eq!(map.mappings[2].source_index, 0, "negative source deltas are legal");

        let got = map.attribute_by_source();
        assert_eq!(got, vec![(0, 10), (1, 10)], "a owns 0..10, b owns 10..20");
    }

    #[test]
    fn vlq_sign_handling() {
        assert_eq!(decode_vlq("A"), 0);
        assert_eq!(decode_vlq("C"), 1);
        assert_eq!(decode_vlq("D"), -1);
        assert_eq!(decode_vlq("gB"), 16);
    }

    #[test]
    fn attribution_uses_gaps_between_sorted_mappings() {
        let map = parse_bytes(TINY.as_bytes()).unwrap();
        let got = map.attribute_by_source();

        assert_eq!(got, vec![(0, 5)], "only source 0 is referenced: {got:?}");
        assert_eq!(map.total_generated_bytes(), 5);
    }

    #[test]
    fn non_v3_maps_are_rejected_not_silently_parsed() {
        let v2 = r#"{"version":2,"sources":[],"names":[],"mappings":""}"#;
        let err = parse_bytes(v2.as_bytes()).unwrap_err();
        assert!(matches!(err, crate::Error::Unsupported(_)), "got {err:?}");
    }

    #[test]
    fn indexed_maps_are_concatenated_with_offsets() {
        let indexed = r#"{
          "version": 3,
          "file": "out.js",
          "sections": [
            {"offset": {"line": 0, "column": 0}, "map": {"version": 3, "sources": ["a.ts"], "names": [], "mappings": "AAAA"}},
            {"offset": {"line": 1, "column": 0}, "map": {"version": 3, "sources": ["b.ts"], "names": [], "mappings": "AAAA"}}
          ]
        }"#;
        let map = parse_bytes(indexed.as_bytes()).unwrap();
        assert_eq!(map.sources, vec!["a.ts", "b.ts"]);
        assert_eq!(map.mappings.len(), 2);
        assert_eq!(map.mappings[0].source_index, 0);
        assert_eq!(map.mappings[1].source_index, 1, "section sources are re-indexed");
        assert!(map.mappings[1].generated_offset > map.mappings[0].generated_offset);
    }

    #[test]
    fn large_map_decodes_without_quadratic_allocation() {
        let mut mappings = String::new();
        for i in 0..200_000 {
            if i > 0 {
                mappings.push(',');
            }
            mappings.push_str("AAAA");
        }
        let raw = format!(
            r#"{{"version":3,"file":"o.js","sources":["s.js"],"sourcesContent":[],"names":[],"mappings":"{mappings}"}}"#
        );
        let t0 = std::time::Instant::now();
        let map = parse_bytes(raw.as_bytes()).unwrap();
        let dt = t0.elapsed();
        assert_eq!(map.mappings.len(), 200_000);
        assert!(dt.as_secs_f64() < 5.0, "decode took {:.2}s", dt.as_secs_f64());
    }

    #[test]
    fn path_normalisation_handles_the_shapes_bundlers_emit() {
        assert_eq!(normalise_source_path("webpack://app/./src/a.js", None), "src/a.js");
        assert_eq!(normalise_source_path("./src/b.ts", None), "src/b.ts");

        assert_eq!(normalise_source_path("file:///repo/src/c.ts", None), "/repo/src/c.ts");
        assert_eq!(
            normalise_source_path("/repo/src/d.ts", Some(std::path::Path::new("/repo"))),
            "src/d.ts"
        );
    }

    #[test]
    fn attribution_by_path_keys_on_normalised_names() {
        let m = ParsedSourceMap {
            sources: vec!["webpack://fixture/./src/module-0.ts".into()],
            names: vec![],
            sources_content: vec![Some("x".into())],
            mappings: vec![
                super::Mapping {
                    generated_offset: 0,
                    source_index: 0,
                    original_line: 0,
                    original_column: 0,
                    name_index: None,
                },
                super::Mapping {
                    generated_offset: 500,
                    source_index: 0,
                    original_line: 1,
                    original_column: 0,
                    name_index: None,
                },
            ],
            file: None,
        };
        let by_path = m.attribution_by_path(None);
        assert_eq!(by_path.get("src/module-0.ts"), Some(&500));
    }

    #[test]
    fn empty_mappings_are_valid() {
        let empty = r#"{"version":3,"file":"o.js","sources":[],"names":[],"mappings":""}"#;
        let map = parse_bytes(empty.as_bytes()).unwrap();
        assert!(map.mappings.is_empty());
        assert_eq!(decode("", 0).unwrap().len(), 0);
    }

    #[test]
    fn an_out_of_range_source_index_is_rejected() {
        let raw =
            br#"{"version":3,"file":"b.js","sources":["a.ts"],"names":[],"mappings":"AAAA,CCAA"}"#;
        let err = parse_bytes(raw).expect_err("source index 1 of 1 must be rejected");
        assert!(err.to_string().contains("source"), "the error should name the problem: {err}");
    }

    #[test]
    fn a_well_formed_map_still_parses() {
        let raw = br#"{"version":3,"file":"b.js","sources":["a.ts","b.ts"],"names":[],"mappings":"AAAA,CCAA"}"#;
        let map = parse_bytes(raw).expect("valid map");
        assert_eq!(map.mappings.len(), 2);
        assert_eq!(map.mappings[1].source_index, 1);
    }
}
