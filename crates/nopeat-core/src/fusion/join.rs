use std::collections::HashMap;

use crate::model::Module;

pub(crate) const MAX_SUFFIX_SEGMENTS: usize = 16;

pub(crate) fn suffixes_of(normalised: &str) -> Vec<String> {
    let segs: Vec<&str> = normalised.split('/').filter(|s| !s.is_empty()).collect();
    if segs.is_empty() {
        return vec![normalised.to_string()];
    }
    let keep = segs.len().min(MAX_SUFFIX_SEGMENTS);
    let start = segs.len() - keep;
    (start..segs.len()).map(|i| segs[i..].join("/")).collect()
}

pub(crate) struct PathIndex<'a> {
    by_key: HashMap<String, Vec<(&'a String, u64)>>,
}

impl<'a> PathIndex<'a> {
    pub(crate) fn build(table: &'a HashMap<String, u64>) -> Self {
        let mut by_key: HashMap<String, Vec<(&'a String, u64)>> = HashMap::new();
        for (path, bytes) in table {
            for key in suffixes_of(&normalise(path)) {
                by_key.entry(key).or_default().push((path, *bytes));
            }
        }

        for bucket in by_key.values_mut() {
            bucket.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        }
        Self { by_key }
    }

    pub(crate) fn get(&self, normalised_name: &str) -> Vec<(&'a String, u64)> {
        let mut out: Vec<(&'a String, u64)> = Vec::new();
        for key in suffixes_of(normalised_name) {
            if let Some(bucket) = self.by_key.get(&key) {
                for entry in bucket {
                    if !out.iter().any(|(p, _)| *p == entry.0) {
                        out.push(*entry);
                    }
                }
            }
        }
        out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        out
    }
}

pub(crate) fn matching_sources_indexed(
    module: &Module,
    index: &PathIndex<'_>,
) -> Vec<(String, u64)> {
    index
        .get(&normalise(&module.name))
        .into_iter()
        .map(|(path, bytes)| (path.clone(), bytes))
        .collect()
}

#[cfg(test)]
pub(crate) fn matching_sources(
    module: &Module,
    by_path: &HashMap<String, u64>,
) -> Vec<(String, u64)> {
    let mut out: Vec<(String, u64)> = Vec::new();
    let name = normalise(&module.name);
    for (path, bytes) in by_path {
        if suffix_match(&name, path) {
            out.push((path.clone(), *bytes));
        }
    }
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

pub(crate) fn normalise(path: &str) -> String {
    crate::sourcemap::normalise_source_path(path, None)
}

pub fn suffix_match(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let a_segs: Vec<&str> = a.split('/').filter(|s| !s.is_empty()).collect();
    let b_segs: Vec<&str> = b.split('/').filter(|s| !s.is_empty()).collect();
    let overlap = a_segs.len().min(b_segs.len());
    for take in (1..=overlap).rev() {
        if a_segs[a_segs.len() - take..] == b_segs[b_segs.len() - take..] {
            return take > 1 || a_segs.last() == b_segs.last();
        }
    }
    false
}

pub(crate) fn module_assets<'a>(
    module: &Module,
    graph: &'a crate::model::UnifiedBundleGraph,
) -> impl Iterator<Item = &'a String> {
    graph
        .assets
        .iter()
        .filter(move |a| module.chunks.iter().any(|c| a.chunks.contains(c)))
        .map(|a| &a.name)
}
