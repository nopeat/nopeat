use std::collections::BTreeMap;

use serde::Deserialize;

use crate::model::{
    Asset, Chunk, InputArtifact, Module, PackageRef, SizeDimension, SizeSet, UnifiedBundleGraph,
};
use crate::stats::recompute_totals;
use crate::{Error, Result};

#[derive(Deserialize, Default)]
struct ViteEntry {
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    src: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    is_entry: bool,
    #[serde(default, rename = "isEntry")]
    is_entry_camel: bool,
    #[serde(default)]
    imports: Vec<String>,
    #[serde(default)]
    css: Vec<String>,
}

#[derive(Deserialize, Default)]
struct ViteManifest {
    #[serde(flatten)]
    entries: BTreeMap<String, ViteEntry>,
}

#[derive(Deserialize, Default)]
struct NextManifest {
    #[serde(default)]
    pages: Option<BTreeMap<String, Vec<String>>>,
    #[serde(default, rename = "app")]
    app: Option<BTreeMap<String, Vec<String>>>,
}

fn is_entry(e: &ViteEntry) -> bool {
    e.is_entry || e.is_entry_camel
}

fn package_ref(path: &str) -> Option<std::sync::Arc<PackageRef>> {
    crate::stats::package_of(path)
        .map(|(name, root)| std::sync::Arc::new(PackageRef { name, version: None, path: root }))
}

pub fn ingest_vite_manifest(bytes: &[u8]) -> Result<UnifiedBundleGraph> {
    let manifest: ViteManifest = serde_json::from_slice(bytes).map_err(Error::Json)?;

    let mut graph = UnifiedBundleGraph::new();
    graph.inputs.push(InputArtifact::ViteManifest);
    graph.totals.size_dimension = SizeDimension::Parsed;

    let mut chunk_of_file: BTreeMap<String, u32> = BTreeMap::new();
    let mut chunk_entry_flags: BTreeMap<u32, bool> = BTreeMap::new();
    let mut next_chunk = 0u32;

    for (key, entry) in &manifest.entries {
        let module_name =
            entry.src.clone().or_else(|| entry.name.clone()).unwrap_or_else(|| key.clone());
        let file = match &entry.file {
            Some(f) => f.clone(),
            None => continue,
        };
        let id = *chunk_of_file.entry(file.clone()).or_insert_with(|| {
            let id = next_chunk;
            next_chunk += 1;
            id
        });
        chunk_entry_flags.insert(id, is_entry(entry));

        let module = Module {
            id: module_name.clone(),
            name: module_name.clone(),
            issuer: None,
            reasons: entry.imports.clone(),
            package: package_ref(&module_name),
            chunks: vec![id],
            sizes: SizeSet { stat: 0, ..SizeSet::default() },
            attribution_delta: 0,
            sources: Vec::new(),
        };
        graph.modules.insert(module_name.clone(), module);
    }

    let mut asset_chunks: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for (file, id) in &chunk_of_file {
        asset_chunks.entry(file.clone()).or_default().push(*id);
    }
    for entry in manifest.entries.values() {
        let Some(file) = &entry.file else { continue };
        let Some(id) = chunk_of_file.get(file) else { continue };
        for css in &entry.css {
            asset_chunks.entry(css.clone()).or_default().push(*id);
        }
    }

    for (file, id) in &chunk_of_file {
        graph.chunks.push(Chunk {
            id: *id,
            names: vec![file.clone()],
            initial: chunk_entry_flags.get(id).copied().unwrap_or(false),
            assets: vec![file.clone()],
            size: SizeSet::default(),
        });
    }

    let mut files: Vec<String> = asset_chunks.keys().cloned().collect();
    files.sort();
    for file in files {
        let chunks = asset_chunks[&file].clone();
        graph.assets.push(Asset {
            name: file,
            size: 0,
            chunks,
            sizes: SizeSet { stat: 0, ..SizeSet::default() },
        });
    }

    recompute_totals(&mut graph);
    Ok(graph)
}

pub fn ingest_next_manifest(bytes: &[u8]) -> Result<UnifiedBundleGraph> {
    let m: NextManifest = serde_json::from_slice(bytes).map_err(Error::Json)?;

    let mut graph = UnifiedBundleGraph::new();
    graph.inputs.push(InputArtifact::NextManifest);
    graph.totals.size_dimension = SizeDimension::Parsed;

    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    if let Some(pages) = &m.pages {
        for (page, files) in pages {
            groups.push((format!("page {page}"), files.clone()));
        }
    }
    if let Some(app) = &m.app {
        for (route, files) in app {
            groups.push((format!("app {route}"), files.clone()));
        }
    }
    if groups.is_empty() {
        return Err(Error::Unsupported(
            "next build manifest: no `pages` or `app` mapping found".into(),
        ));
    }

    let mut file_chunk: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for (id, (name, files)) in groups.iter().enumerate() {
        // Chunk ids are u32 everywhere else in the graph. The route count cannot
        // reach that bound, so a failure here is a bug rather than bad input, but
        // it is still not a reason to truncate.
        let id = u32::try_from(id).map_err(|_| {
            Error::Malformed(format!("{} route groups exceeds the chunk id range", groups.len()))
        })?;
        graph.chunks.push(Chunk {
            id,
            names: vec![name.clone()],
            initial: false,
            assets: files.clone(),
            size: SizeSet::default(),
        });
        for f in files {
            file_chunk.entry(f.clone()).or_default().push(id);
        }
    }

    for (file, chunks) in &file_chunk {
        let module = Module {
            id: file.clone(),
            name: file.clone(),
            issuer: None,
            reasons: Vec::new(),
            package: package_ref(file),
            chunks: chunks.clone(),
            sizes: SizeSet { stat: 0, ..SizeSet::default() },
            attribution_delta: 0,
            sources: Vec::new(),
        };
        graph.modules.insert(file.clone(), module);
        graph.assets.push(Asset {
            name: file.clone(),
            size: 0,
            chunks: chunks.clone(),
            sizes: SizeSet { stat: 0, ..SizeSet::default() },
        });
    }

    recompute_totals(&mut graph);
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::{ingest_next_manifest, ingest_vite_manifest};

    #[test]
    fn vite_manifest_becomes_modules_assets_and_chunks() {
        let manifest = br#"{
            "main.tsx": {"file": "assets/main-a1b2.js", "src": "src/main.tsx", "isEntry": true, "imports": ["chunk-xyz"], "css": ["assets/main-a1b2.css"]},
            "chunk-xyz": {"file": "assets/chunk-xyz.js", "imports": []}
        }"#;
        let g = ingest_vite_manifest(manifest).unwrap();
        assert_eq!(g.modules.len(), 2);
        assert_eq!(g.chunks.len(), 2);
        assert!(g.assets.iter().any(|a| a.name == "assets/main-a1b2.js"));
        assert!(g.assets.iter().any(|a| a.name == "assets/main-a1b2.css"));
        assert!(g.assets.iter().all(|a| a.sizes.stat == 0));
        assert!(matches!(g.inputs[0], crate::model::InputArtifact::ViteManifest));
    }

    #[test]
    fn next_build_manifest_groups_files_by_page() {
        let manifest = br#"{
            "pages": {
                "/_app": ["static/chunks/main-a.js"],
                "/": ["static/chunks/main-a.js", "static/chunks/pages-index-b.js"]
            }
        }"#;
        let g = ingest_next_manifest(manifest).unwrap();
        assert_eq!(g.chunks.len(), 2);
        assert_eq!(g.modules.len(), 2);
        let shared = &g.modules["static/chunks/main-a.js"];
        assert_eq!(shared.chunks, vec![0, 1]);
        assert!(matches!(g.inputs[0], crate::model::InputArtifact::NextManifest));
    }
}
