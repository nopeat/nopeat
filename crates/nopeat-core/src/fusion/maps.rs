use std::path::Path;

use crate::model::UnifiedBundleGraph;
use crate::sourcemap::ParsedSourceMap;

pub fn maps_in_dir(dir: &Path) -> Vec<(String, ParsedSourceMap)> {
    maps_in_dir_detailed(dir).0
}

pub fn maps_in_dir_detailed(dir: &Path) -> (Vec<(String, ParsedSourceMap)>, Vec<String>) {
    let mut paths = Vec::new();
    collect_maps(dir, dir, &mut paths, 0);
    paths.sort_by(|a, b| a.0.cmp(&b.0));

    let mut maps = Vec::new();
    let mut unreadable = Vec::new();
    for (name, path) in paths {
        let parsed = std::fs::File::open(&path)
            .map_err(|e| e.to_string())
            .and_then(|file| crate::sourcemap::parse_reader(file).map_err(|e| e.to_string()));
        match parsed {
            Ok(map) => maps.push((name, map)),
            Err(why) => unreadable.push(format!("{name}: {why}")),
        }
    }
    (maps, unreadable)
}

const MAX_MAP_DEPTH: usize = 12;

fn collect_maps(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, std::path::PathBuf)>,
    depth: usize,
) {
    if depth > MAX_MAP_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut dirs = Vec::new();
    for entry in entries.filter_map(std::result::Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
            continue;
        }
        if path.extension().is_none_or(|e| e != "map") {
            continue;
        }
        let name = path.strip_prefix(root).map_or_else(
            |_| path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            |p| p.to_string_lossy().replace('\\', "/"),
        );
        out.push((name, path));
    }
    for dir in dirs {
        collect_maps(root, &dir, out, depth + 1);
    }
}

pub(crate) fn asset_for_map(graph: &UnifiedBundleGraph, map_name: &str) -> String {
    let stem = map_name.strip_suffix(".map").unwrap_or(map_name);
    if graph.assets.iter().any(|a| a.name == stem) {
        return stem.to_string();
    }
    stem.to_string()
}

pub(crate) fn generated_len_for(
    graph: &UnifiedBundleGraph,
    map_name: &str,
    map: &ParsedSourceMap,
) -> u64 {
    let stem = map_name.strip_suffix(".map").unwrap_or(map_name);
    if let Some(asset) = graph.assets.iter().find(|a| a.name == stem || a.name == map_name) {
        let measured = asset.sizes.parsed;
        if measured > 0 {
            return measured;
        }
    }
    if let Some(file) = &map.file
        && let Some(asset) = graph.assets.iter().find(|a| &a.name == file)
        && asset.sizes.parsed > 0
    {
        return asset.sizes.parsed;
    }
    map.total_generated_bytes()
}
