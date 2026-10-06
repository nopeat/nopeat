use std::collections::BTreeMap;

use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};

use super::metafile::{MetafileVisitor, ingest_metafile};
use super::{RawAsset, RawChunk, RawModule, join_key, package_of};
use crate::model::{
    Asset, Chunk, InputArtifact, Module, PackageRef, SizeDimension, SizeSet, Totals,
    UnifiedBundleGraph,
};
use crate::{Error, Result};

pub fn ingest_reader<R: std::io::Read>(mut reader: R, tool: &str) -> Result<UnifiedBundleGraph> {
    use serde::de::Deserializer as _;

    if tool == "vite" || tool == "next" {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).map_err(Error::Io)?;
        let graph = if tool == "vite" {
            crate::manifest::ingest_vite_manifest(&bytes)
        } else {
            crate::manifest::ingest_next_manifest(&bytes)
        }?;
        return Ok(graph);
    }

    let buffered = std::io::BufReader::with_capacity(1 << 20, crate::bom::BomSkip::new(reader));
    let mut de = serde_json::Deserializer::from_reader(buffered);
    if tool == "esbuild" {
        let graph = de.deserialize_map(MetafileVisitor)?;
        de.end()?;
        return Ok(graph);
    }
    let graph = de.deserialize_map(StatsVisitor { tool: tool.to_string() })?;
    de.end()?;
    Ok(graph)
}

pub fn ingest_stats(bytes: &[u8], tool: &str) -> Result<UnifiedBundleGraph> {
    ingest_reader(bytes, tool)
}

struct StatsVisitor {
    tool: String,
}

impl<'de> Visitor<'de> for StatsVisitor {
    type Value = UnifiedBundleGraph;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a webpack/rspack stats object")
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        let mut graph = UnifiedBundleGraph::new();
        let mut saw_modules = false;
        let mut nameless_modules = 0u64;

        while let Some(key) = map.next_key::<std::borrow::Cow<str>>()? {
            match key.as_ref() {
                "assets" => {
                    let mut assets = Vec::new();
                    map.next_value_seed(SeqCollector::<RawAsset>::new(&mut assets))?;
                    graph.assets = assets
                        .into_iter()
                        .map(|a| Asset {
                            name: a.name,
                            size: a.size,
                            chunks: a.chunks,
                            sizes: SizeSet { stat: a.size, ..SizeSet::default() },
                        })
                        .collect();
                }
                "chunks" => {
                    let mut chunks = Vec::new();
                    map.next_value_seed(SeqCollector::<RawChunk>::new(&mut chunks))?;
                    graph.chunks = chunks
                        .into_iter()
                        .map(|c| Chunk {
                            id: c.id,
                            names: c.names,
                            initial: c.initial,
                            assets: c.files,
                            size: SizeSet { stat: c.size, ..SizeSet::default() },
                        })
                        .collect();
                }
                "modules" => {
                    saw_modules = true;
                    map.next_value_seed(ModulesInto {
                        modules: &mut graph.modules,
                        nameless: &mut nameless_modules,
                    })?;
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        if !saw_modules {
            graph.diagnostics.push(crate::model::Diagnostic {
                severity: crate::model::Severity::Warning,
                code: "NPT0001".into(),
                message: "stats file has no `modules` array (dev-server export?)".into(),
                subject: None,
                data: serde_json::Value::Null,
            });
        }

        if nameless_modules > 0 {
            graph.diagnostics.push(crate::model::Diagnostic {
                severity: crate::model::Severity::Info,
                code: "NPT0052".into(),
                message: format!(
                    "skipped {nameless_modules} module(s) with no name; webpack emits one for its \
                     runtime and a few synthetic entries, and a module with no path cannot be \
                     attributed to a file"
                ),
                subject: None,
                data: serde_json::json!({ "modules": nameless_modules }),
            });
        }

        graph.inputs.push(InputArtifact::Stats { tool: self.tool });
        graph.totals.size_dimension = SizeDimension::Parsed;
        recompute_totals(&mut graph);
        Ok(graph)
    }
}

struct SeqCollector<'a, T> {
    out: &'a mut Vec<T>,
}

impl<'a, T> SeqCollector<'a, T> {
    fn new(out: &'a mut Vec<T>) -> Self {
        Self { out }
    }
}

impl<'de, T: serde::Deserialize<'de>> DeserializeSeed<'de> for SeqCollector<'_, T> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> std::result::Result<(), D::Error> {
        struct V<'a, T> {
            out: &'a mut Vec<T>,
        }
        impl<'de, T: serde::Deserialize<'de>> Visitor<'de> for V<'_, T> {
            type Value = ();
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<(), A::Error> {
                while let Some(item) = seq.next_element::<T>()? {
                    self.out.push(item);
                }
                Ok(())
            }
        }
        d.deserialize_seq(V { out: self.out })
    }
}

struct ModulesInto<'a> {
    modules: &'a mut BTreeMap<String, Module>,
    nameless: &'a mut u64,
}

impl<'de> DeserializeSeed<'de> for ModulesInto<'_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> std::result::Result<(), D::Error> {
        struct V<'a> {
            modules: &'a mut BTreeMap<String, Module>,
            interned: &'a mut std::collections::HashMap<String, std::sync::Arc<PackageRef>>,
            nameless: &'a mut u64,
        }

        impl V<'_> {
            fn put(
                raw: RawModule,
                modules: &mut BTreeMap<String, Module>,
                interned: &mut std::collections::HashMap<String, std::sync::Arc<PackageRef>>,
                nameless: &mut u64,
            ) {
                if !raw.children.is_empty() {
                    for child in raw.children {
                        Self::put(child, modules, interned, nameless);
                    }
                    return;
                }

                let Some(name) = raw.name.clone() else {
                    *nameless += 1;
                    return;
                };

                let chunk = raw.chunks.first().copied();
                let id = join_key(raw.identifier.as_deref(), &name, chunk);
                let package = package_of(&name).map(|(pkg, path)| {
                    let cache_key = format!("{pkg}\u{1}{path}");
                    match interned.entry(cache_key) {
                        std::collections::hash_map::Entry::Occupied(e) => e.get().clone(),
                        std::collections::hash_map::Entry::Vacant(e) => {
                            let shared =
                                std::sync::Arc::new(PackageRef { name: pkg, version: None, path });
                            e.insert(shared.clone());
                            shared
                        }
                    }
                });
                let module = Module {
                    id: id.clone(),
                    name,
                    issuer: raw.issuer_name,
                    reasons: raw.reasons.into_iter().filter_map(|r| r.module_name).collect(),
                    package,
                    chunks: raw.chunks,
                    sizes: SizeSet { stat: raw.size, ..SizeSet::default() },
                    attribution_delta: 0,
                    sources: Vec::new(),
                };
                modules.insert(id, module);
            }
        }

        impl<'de> Visitor<'de> for V<'_> {
            type Value = ();

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a modules array")
            }

            fn visit_seq<A: SeqAccess<'de>>(
                mut self,
                mut seq: A,
            ) -> std::result::Result<(), A::Error> {
                let V { modules, interned, nameless } = &mut self;
                while let Some(raw) = seq.next_element::<RawModule>()? {
                    V::put(raw, modules, interned, nameless);
                }
                Ok(())
            }
        }

        let mut interned: std::collections::HashMap<String, std::sync::Arc<PackageRef>> =
            std::collections::HashMap::new();
        let mut nameless = 0u64;
        d.deserialize_seq(V {
            modules: self.modules,
            interned: &mut interned,
            nameless: &mut nameless,
        })?;
        *self.nameless += nameless;
        Ok(())
    }
}

pub fn recompute_totals(graph: &mut UnifiedBundleGraph) {
    let mut total = 0u64;
    let mut sum = 0u64;
    let mut packages: Vec<&str> = Vec::new();

    for asset in &graph.assets {
        total += asset.sizes.effective();
    }
    for module in graph.modules.values() {
        sum += module.sizes.effective();
        if let Some(pkg) = &module.package {
            packages.push(pkg.name.as_str());
        }
    }
    packages.sort_unstable();
    packages.dedup();

    graph.totals = Totals {
        total_size: total,
        module_count: graph.modules.len() as u64,
        asset_count: graph.assets.len() as u64,
        package_count: packages.len() as u64,
        size_dimension: graph.totals.size_dimension,
        module_size_sum: sum,
    };
}

pub fn ingest_bytes(bytes: &[u8], name: &str) -> Result<UnifiedBundleGraph> {
    let head = &bytes[..bytes.len().min(512 * 1024)];
    let has = |needle: &[u8]| head.windows(needle.len()).any(|w| w == needle);

    let looks_like_stats = has(b"\"modules\"") || has(b"\"chunks\"") || has(b"\"assets\"");
    let looks_like_metafile = has(b"\"inputs\"") && has(b"\"outputs\"");

    if looks_like_stats {
        return ingest_stats(bytes, "webpack");
    }
    if looks_like_metafile {
        return ingest_metafile(bytes);
    }
    Err(Error::Unsupported(format!(
        "{name}: no recognised artifact. Looked for webpack/rspack stats.json \
         (assets/chunks/modules) and esbuild metafile.json (inputs/outputs)."
    )))
}

pub fn ingest_file(path: &std::path::Path, tool: &str) -> Result<UnifiedBundleGraph> {
    let file = std::fs::File::open(path).map_err(Error::Io)?;
    ingest_reader(file, tool)
}

#[cfg(test)]
mod tests {
    use super::{ingest_stats, package_of};
    use crate::stats::ingest_metafile;

    const STATS: &str = r#"{
        "version": "5.90.0",
        "assets": [{"name": "main.js", "size": 3000, "chunks": [0], "chunkNames": ["main"]}],
        "chunks": [{"id": 0, "names": ["main"], "initial": true, "files": ["main.js"], "size": 3000}],
        "modules": [
            {"id": 1, "identifier": "/repo/node_modules/preact/dist/preact.js",
             "name": "./node_modules/preact/dist/preact.js", "size": 1200, "chunks": [0],
             "reasons": [{"moduleName": "./src/app.js", "type": "harmony side effect evaluation"}],
             "source": "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"},
            {"id": 2, "name": "./src/app.js", "size": 800, "chunks": [0],
             "issuerName": null, "reasons": []}
        ],
        "entrypoints": {},
        "errors": []
    }"#;

    #[test]
    fn ingests_assets_chunks_and_modules() {
        let g = ingest_stats(STATS.as_bytes(), "webpack").unwrap();
        assert_eq!(g.assets.len(), 1);
        assert_eq!(g.chunks.len(), 1);
        assert_eq!(g.modules.len(), 2);
        assert_eq!(g.totals.asset_count, 1);
        assert_eq!(g.totals.module_count, 2);
        assert_eq!(g.totals.total_size, 3000, "total comes from the assets");
        assert_eq!(g.totals.module_size_sum, 2000);
    }

    #[test]
    fn reasons_and_package_are_extracted() {
        let g = ingest_stats(STATS.as_bytes(), "webpack").unwrap();
        let preact = g.modules.values().find(|m| m.name.contains("preact")).expect("preact module");
        assert_eq!(preact.package.as_ref().unwrap().name, "preact");
        assert_eq!(preact.reasons, vec!["./src/app.js".to_string()]);

        let app = g.modules.values().find(|m| m.name.contains("app.js")).unwrap();
        assert!(app.package.is_none(), "application code has no package");
    }

    #[test]
    fn missing_modules_array_is_a_diagnostic_not_an_error() {
        let g = ingest_stats(br#"{"version":"5.0.0","assets":[],"chunks":[]}"#, "webpack").unwrap();
        assert!(g.diagnostics.iter().any(|d| d.code == "NPT0001"));
    }

    #[test]
    fn unknown_fields_are_ignored_cheaply() {
        let big = format!(
            r#"{{"version":"5.0.0","assets":[],"chunks":[],"modules":[
                {{"id":1,"name":"./src/a.js","size":10,"source":"{}"}}]}}"#,
            "x".repeat(50_000)
        );
        let g = ingest_stats(big.as_bytes(), "webpack").unwrap();
        let stored: usize = g.modules.values().map(|m| m.name.len() + m.id.len()).sum();
        assert!(stored < 100, "only the key and name are kept, got {stored}");
    }

    #[test]
    fn esbuild_metafile_is_dispatched() {
        let meta = r#"{"inputs":{"src/a.js":{"bytes":100},"node_modules/marked/lib/marked.js":{"bytes":900}},
                      "outputs":{"dist/out.js":{"bytes":700,"inputs":{"src/a.js":{"bytesInOutput":80},
                      "node_modules/marked/lib/marked.js":{"bytesInOutput":600}}}}}"#;
        let g = ingest_metafile(meta.as_bytes()).unwrap();
        assert_eq!(g.modules.len(), 2);
        assert_eq!(g.assets.len(), 1);
        assert_eq!(g.totals.total_size, 700);
        assert_eq!(g.totals.package_count, 1);
        assert_eq!(package_of("node_modules/marked/lib/marked.js").unwrap().0, "marked");
    }

    #[test]
    fn metafile_is_dispatched_by_the_streaming_reader_the_cli_uses() {
        let meta = r#"{"inputs":{"src/a.js":{"bytes":100},"node_modules/marked/lib/marked.js":{"bytes":900}},
                      "outputs":{"dist/out.js":{"bytes":700,"inputs":{"src/a.js":{"bytesInOutput":80},
                      "node_modules/marked/lib/marked.js":{"bytesInOutput":600}}}}}"#;
        let g = crate::stats::ingest_reader(meta.as_bytes(), "esbuild").unwrap();
        assert_eq!(g.modules.len(), 2);
        assert_eq!(g.assets.len(), 1);
        assert_eq!(g.totals.total_size, 700);
        assert_eq!(g.totals.package_count, 1);
        assert!(matches!(g.inputs.as_slice(), [crate::model::InputArtifact::EsbuildMetafile]));
    }
}
