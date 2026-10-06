use std::collections::HashMap;
use std::collections::btree_map::Entry;

use rayon::prelude::*;

use crate::model::{
    GhostModule, HiddenSource, Module, Severity, SizeDimension, SizeSet, SourceRef,
    UnifiedBundleGraph,
};
use crate::sourcemap::ParsedSourceMap;
use crate::stats::recompute_totals;

mod ghost;
mod join;
mod maps;

pub use ghost::classify_ghost;
pub use join::suffix_match;
pub use maps::{maps_in_dir, maps_in_dir_detailed};

use join::{PathIndex, matching_sources_indexed, module_assets};
use maps::{asset_for_map, generated_len_for};

#[cfg(test)]
use join::matching_sources;

pub const GHOSTS_IN_SUMMARY: usize = 500;

pub const HIDDEN_SOURCES_IN_SUMMARY: usize = 500;

pub fn analyse(
    graph: &mut UnifiedBundleGraph,
    maps: &[(String, ParsedSourceMap)],
) -> FusionOutcome {
    let mut outcome = FusionOutcome { ghosts_detectable: true, ..FusionOutcome::default() };

    let mut by_asset: HashMap<String, HashMap<String, u64>> = HashMap::new();
    let mut mapped_bytes_per_asset: Vec<(String, u64)> = Vec::new();
    let mut total_mapped = 0u64;
    for (name, map) in maps {
        let asset_name = asset_for_map(graph, name);
        let generated = generated_len_for(graph, name, map);
        let bytes_for_map: u64 = {
            let entry = by_asset.entry(asset_name).or_default();
            map.attribution_by_path_in(None, generated)
                .into_iter()
                .map(|(path, bytes)| {
                    *entry.entry(path).or_insert(0) += bytes;
                    bytes
                })
                .sum()
        };
        total_mapped += bytes_for_map;
        outcome.mapped_bytes = total_mapped;
        mapped_bytes_per_asset.push((name.clone(), bytes_for_map));
    }

    let mut claimed: HashMap<String, u64> = HashMap::new();
    for (id, sources, attributed) in attribute_modules(graph, &by_asset) {
        for s in &sources {
            *claimed.entry(s.file.clone()).or_insert(0) += s.bytes;
        }
        if let Some(module) = graph.modules.get_mut(&id) {
            module.sources = sources;
            module.sizes.attributed = Some(attributed);
            module.attribution_delta =
                attributed.cast_signed().saturating_sub(module.sizes.stat.cast_signed());
            outcome.attributed_modules += 1;
            outcome.attributed_bytes += attributed;
        }
    }

    let mut ghosts: Vec<GhostModule> = Vec::new();
    let mut ghost_total = 0usize;
    let mut ghost_bytes_total = 0u64;

    let mut owned_chunks: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    for asset in &graph.assets {
        owned_chunks.extend(asset.chunks.iter().copied());
    }
    for module in graph.modules.values() {
        if module.sizes.attributed.is_none() && module.sizes.stat > 0 {
            let in_an_asset = module.chunks.iter().any(|c| owned_chunks.contains(c));
            let reason = classify_ghost(module.sizes.stat, module.sizes.attributed, in_an_asset)
                .unwrap_or(crate::model::GhostReason::Unmapped);
            ghost_total += 1;
            ghost_bytes_total += module.sizes.stat;
            ghosts.push(GhostModule {
                module: module.name.clone(),
                declared_size: module.sizes.stat,
                chunks: module.chunks.clone(),
                reason,
            });

            if ghosts.len() > GHOSTS_IN_SUMMARY * 2 {
                ghosts.sort_by(|a, b| {
                    b.declared_size.cmp(&a.declared_size).then_with(|| a.module.cmp(&b.module))
                });
                ghosts.truncate(GHOSTS_IN_SUMMARY);
            }
        }
    }
    ghosts.sort_by(|a, b| {
        b.declared_size.cmp(&a.declared_size).then_with(|| a.module.cmp(&b.module))
    });
    ghosts.truncate(GHOSTS_IN_SUMMARY);
    let ghosts_dropped = ghost_total.saturating_sub(ghosts.len()) as u64;

    outcome.ghost_count = ghost_total;
    outcome.ghost_bytes = ghost_bytes_total;

    let mut hidden: Vec<HiddenSource> = Vec::new();
    for (asset, table) in &by_asset {
        for (path, bytes) in table {
            let claim = claimed.get(path).copied().unwrap_or(0);
            if bytes > &claim {
                hidden.push(HiddenSource {
                    file: path.clone(),
                    bytes: bytes - claim,
                    location: Some(asset.clone()),
                });
            }
        }
    }
    hidden.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.file.cmp(&b.file)));

    let hidden_count_total = hidden.len();
    let hidden_bytes_total: u64 = hidden.iter().map(|h| h.bytes).sum();
    hidden.truncate(HIDDEN_SOURCES_IN_SUMMARY);
    let hidden_dropped = hidden_count_total.saturating_sub(hidden.len()) as u64;
    outcome.hidden_count = hidden_count_total;
    outcome.hidden_bytes = hidden_bytes_total;

    let total_bytes: u64 = graph.assets.iter().map(|a| a.sizes.effective()).sum();
    let mut covered_bytes = 0u64;
    let mut covered_assets = 0usize;
    for asset in &graph.assets {
        let stem = format!("{}.map", asset.name);
        if let Some((_, bytes)) =
            mapped_bytes_per_asset.iter().find(|(name, _)| name == &stem || name == &asset.name)
        {
            covered_bytes += (*bytes).min(asset.sizes.effective());
            covered_assets += 1;
        }
    }
    let coverage = if total_bytes == 0 { 0.0 } else { covered_bytes as f64 / total_bytes as f64 };
    outcome.coverage = coverage;
    outcome.covered_assets = covered_assets;
    let dimension = if total_bytes > 0 && coverage >= COVERAGE_THRESHOLD {
        SizeDimension::Attributed
    } else {
        SizeDimension::Parsed
    };
    graph.totals.size_dimension = dimension;

    let delta: i64 = graph.modules.values().map(|m| m.attribution_delta).sum();
    let significant = graph
        .modules
        .values()
        .filter(|m| {
            m.sizes.stat > 0
                && (m.attribution_delta.unsigned_abs() as f64) / m.sizes.stat as f64 > 0.01
        })
        .count() as u64;

    graph.fusion = Some(crate::model::FusionSummary {
        ghost_modules: ghosts,
        ghost_modules_truncated: ghosts_dropped,
        hidden_sources: hidden,
        hidden_sources_truncated: hidden_dropped,
        total_attribution_delta: delta,
        significant_corrections: significant,
    });

    recompute_totals(graph);
    if dimension == SizeDimension::Attributed
        && let Err(e) = graph.check_size_invariant(1_000)
    {
        graph.diagnostics.push(crate::model::Diagnostic {
            severity: Severity::Error,
            code: "NPT0042".into(),
            message: e.to_string(),
            subject: None,
            data: serde_json::Value::Null,
        });
    }

    outcome
}

pub fn analyse_sources_only(
    graph: &mut UnifiedBundleGraph,
    maps: &[(String, ParsedSourceMap)],
) -> FusionOutcome {
    let mut outcome = FusionOutcome { ghosts_detectable: false, ..FusionOutcome::default() };
    if maps.is_empty() {
        graph.diagnostics.push(crate::folder::no_declared_graph_diagnostic());
        graph.diagnostics.push(crate::model::Diagnostic {
            severity: Severity::Warning,
            code: "NPT0050".into(),
            message: "no source maps next to the output: only file sizes are known. Build with \
                      --sourcemap to get per-source attribution."
                .into(),
            subject: None,
            data: serde_json::Value::Null,
        });
        recompute_totals(graph);
        graph.totals.size_dimension = SizeDimension::Parsed;
        return outcome;
    }

    let mut hidden: Vec<HiddenSource> = Vec::new();
    let mut hidden_total = 0u64;
    let mut hidden_count = 0usize;

    for (map_name, map) in maps {
        let asset_name = asset_for_map(graph, map_name);
        let generated = generated_len_for(graph, map_name, map);
        let table = map.attribution_by_path_in(None, generated);
        let mapped: u64 = table.values().sum();

        for (path, bytes) in table {
            if path.is_empty() {
                continue;
            }
            let id = format!("source:{path}");
            match graph.modules.entry(id) {
                Entry::Occupied(mut occupied) => {
                    let module = occupied.get_mut();
                    let total = module.sizes.attributed.unwrap_or(0) + bytes;
                    module.sizes.attributed = Some(total);
                    for source in &mut module.sources {
                        if source.file == path {
                            source.bytes = total;
                            break;
                        }
                    }
                }
                Entry::Vacant(vacant) => {
                    outcome.attributed_modules += 1;
                    vacant.insert(Module {
                        id: format!("source:{path}"),
                        name: path.clone(),
                        issuer: None,
                        reasons: Vec::new(),
                        package: crate::folder::package_ref(&path),
                        chunks: Vec::new(),
                        sizes: SizeSet { stat: 0, attributed: Some(bytes), ..SizeSet::default() },
                        attribution_delta: 0,
                        sources: vec![SourceRef { file: path.clone(), bytes, line: None }],
                    });
                }
            }
            outcome.attributed_bytes += bytes;
        }

        let asset_size =
            graph.assets.iter().find(|a| a.name == asset_name).map_or(0, |a| a.sizes.effective());
        if asset_size > mapped {
            let unowned = asset_size - mapped;
            hidden_total += unowned;
            hidden_count += 1;
            hidden.push(HiddenSource {
                file: format!("{asset_name} (no source owns these bytes)"),
                bytes: unowned,
                location: Some(asset_name.clone()),
            });
            outcome.mapped_bytes += mapped;
        }
    }

    hidden.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.file.cmp(&b.file)));
    let hidden_dropped = (hidden_count.saturating_sub(HIDDEN_SOURCES_IN_SUMMARY)) as u64;
    hidden.truncate(HIDDEN_SOURCES_IN_SUMMARY);

    outcome.hidden_count = hidden_count;
    outcome.hidden_bytes = hidden_total;

    let total_bytes: u64 = graph.assets.iter().map(|a| a.sizes.effective()).sum();
    let covered: u64 = graph
        .assets
        .iter()
        .filter(|asset| {
            maps.iter().any(|(name, _)| {
                let stem = name.strip_suffix(".map").unwrap_or(name);
                stem == asset.name
            })
        })
        .map(|a| a.sizes.effective())
        .sum();
    outcome.coverage = if total_bytes == 0 { 0.0 } else { covered as f64 / total_bytes as f64 };
    outcome.covered_assets = maps.len();

    graph.totals.size_dimension = if total_bytes > 0 && outcome.coverage >= COVERAGE_THRESHOLD {
        SizeDimension::Attributed
    } else {
        SizeDimension::Parsed
    };

    graph.diagnostics.push(crate::folder::no_declared_graph_diagnostic());
    graph.fusion = Some(crate::model::FusionSummary {
        ghost_modules: Vec::new(),
        ghost_modules_truncated: 0,
        hidden_sources: hidden,
        hidden_sources_truncated: hidden_dropped,
        total_attribution_delta: 0,
        significant_corrections: 0,
    });

    recompute_totals(graph);
    outcome
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FusionOutcome {
    pub ghosts_detectable: bool,
    pub mapped_bytes: u64,
    pub attributed_modules: usize,
    pub attributed_bytes: u64,
    pub coverage: f64,
    pub ghost_count: usize,
    pub ghost_bytes: u64,
    pub hidden_count: usize,
    pub hidden_bytes: u64,
    pub covered_assets: usize,
}

fn attribute_modules(
    graph: &UnifiedBundleGraph,
    by_asset: &HashMap<String, HashMap<String, u64>>,
) -> Vec<(String, Vec<SourceRef>, u64)> {
    let indexes: HashMap<&String, PathIndex<'_>> =
        by_asset.iter().map(|(asset, table)| (asset, PathIndex::build(table))).collect();
    let modules: Vec<&Module> = graph.modules.values().collect();
    modules
        .par_iter()
        .filter_map(|module| {
            let tables: Vec<&PathIndex<'_>> =
                module_assets(module, graph).filter_map(|a| indexes.get(a)).collect();
            let mut sources: Vec<SourceRef> = tables
                .into_iter()
                .flat_map(|index| matching_sources_indexed(module, index))
                .map(|(file, bytes)| SourceRef { file, bytes, line: None })
                .collect();
            if sources.is_empty() {
                return None;
            }
            let total: u64 = sources.iter().map(|s| s.bytes).sum();
            sources.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.file.cmp(&b.file)));
            Some((module.id.clone(), sources, total))
        })
        .collect()
}

pub const COVERAGE_THRESHOLD: f64 = 0.99;

#[cfg(test)]
mod tests {
    use super::{GHOSTS_IN_SUMMARY, analyse, classify_ghost, suffix_match};
    use crate::model::{
        Asset, Chunk, Module, PackageRef, SizeDimension, SizeSet, UnifiedBundleGraph,
    };
    use crate::sourcemap::parse_bytes;
    use std::collections::{BTreeMap, HashMap};

    const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    fn vlq(value: i64) -> String {
        let mut v = if value < 0 { ((-value) << 1) | 1 } else { value << 1 };
        let mut out = String::new();
        loop {
            let mut digit = usize::try_from(v & 31).unwrap_or(0);
            v >>= 5;
            if v > 0 {
                digit |= 32;
            }
            out.push(B64[digit] as char);
            if v == 0 {
                return out;
            }
        }
    }

    fn segment(col: i64, src: i64, line: i64, orig_col: i64) -> String {
        format!("{}{}{}{}", vlq(col), vlq(src), vlq(line), vlq(orig_col))
    }

    fn module(name: &str, stat: u64) -> Module {
        Module {
            id: name.to_string(),
            name: name.to_string(),
            issuer: None,
            reasons: vec![],
            package: package_of(name).map(std::sync::Arc::new),
            chunks: vec![0],
            sizes: SizeSet { stat, ..SizeSet::default() },
            attribution_delta: 0,
            sources: vec![],
        }
    }

    fn package_of(name: &str) -> Option<PackageRef> {
        let idx = name.find("node_modules/")?;
        let rest = &name[idx + "node_modules/".len()..];
        let seg = rest.split('/').next()?;
        Some(PackageRef {
            name: seg.to_string(),
            version: None,
            path: format!("node_modules/{seg}"),
        })
    }

    fn fixture() -> (UnifiedBundleGraph, crate::sourcemap::ParsedSourceMap) {
        let mut g = UnifiedBundleGraph::new();
        g.assets.push(Asset {
            name: "main.js".into(),
            size: 10_000,
            chunks: vec![0],
            sizes: SizeSet { stat: 10_000, parsed: 10_000, ..SizeSet::default() },
        });
        g.chunks.push(Chunk {
            id: 0,
            names: vec!["main".into()],
            initial: true,
            assets: vec!["main.js".into()],
            size: SizeSet { stat: 10_000, parsed: 10_000, ..SizeSet::default() },
        });
        let mut modules = BTreeMap::new();
        for (name, stat) in [("./src/mapped.ts", 4_000u64), ("./src/ghost.ts", 6_000)] {
            let m = module(name, stat);
            modules.insert(m.id.clone(), m);
        }
        g.modules = modules;
        crate::stats::recompute_totals(&mut g);
        g.totals.size_dimension = SizeDimension::Parsed;

        g.assets.push(Asset {
            name: "vendor.js".into(),
            size: 5_000,
            chunks: vec![1],
            sizes: SizeSet { stat: 5_000, parsed: 5_000, ..SizeSet::default() },
        });
        g.chunks.push(Chunk {
            id: 1,
            names: vec!["vendor".into()],
            initial: true,
            assets: vec!["vendor.js".into()],
            size: SizeSet { stat: 5_000, parsed: 5_000, ..SizeSet::default() },
        });
        crate::stats::recompute_totals(&mut g);

        let mappings = format!("{},{}", segment(0, 0, 0, 0), segment(4_000, 1, 0, 0));
        let raw = format!(
            r#"{{"version":3,"file":"main.js",
               "sources":["webpack://app/./src/mapped.ts","webpack://app/./runtime/inject.js"],
               "sourcesContent":["mapped","injected"],"names":[],"mappings":"{mappings}"}}"#
        );
        let map = parse_bytes(raw.as_bytes()).unwrap();
        (g, map)
    }

    #[test]
    fn fusion_attributes_ghost_and_hidden() {
        let (mut g, map) = fixture();
        let outcome = analyse(&mut g, &[("main.js.map".into(), map)]);

        let mapped = &g.modules["./src/mapped.ts"];
        assert_eq!(mapped.sizes.attributed, Some(4_000), "ground truth, not the declared size");
        assert_eq!(mapped.attribution_delta, 0, "the bundler happened to be right here");
        assert_eq!(mapped.sources.len(), 1);
        assert_eq!(mapped.sources[0].file, "src/mapped.ts");

        assert_eq!(outcome.ghost_count, 1, "exactly the unmapped module");
        let fusion = g.fusion.as_ref().unwrap();
        assert_eq!(fusion.ghost_modules.len(), 1);
        assert_eq!(fusion.ghost_modules[0].module, "./src/ghost.ts");
        assert_eq!(fusion.ghost_modules[0].reason, crate::model::GhostReason::Unmapped);
        assert_eq!(fusion.ghost_modules[0].declared_size, 6_000);

        let hidden = fusion
            .hidden_sources
            .iter()
            .find(|h| h.file == "runtime/inject.js")
            .expect("injected source must be reported, not dropped");

        assert_eq!(hidden.bytes, 6_000, "tail after the last mapping is credited to it");
        assert_eq!(outcome.attributed_bytes, 4_000, "only mapped.ts is claimed by a module");
        assert_eq!(outcome.mapped_bytes, 10_000, "but the map accounts for all of main.js");
    }

    #[test]
    fn a_partial_map_never_claims_the_attributed_dimension() {
        let (mut g, map) = fixture();
        analyse(&mut g, &[("main.js.map".into(), map)]);

        assert_eq!(g.totals.size_dimension, SizeDimension::Parsed);
        let outcome = analyse(&mut g, &[]);
        assert_eq!(outcome.covered_assets, 0);
    }

    #[test]
    fn a_complete_map_claims_attributed() {
        let mut g = UnifiedBundleGraph::new();
        g.assets.push(Asset {
            name: "main.js".into(),
            size: 1_000,
            chunks: vec![0],
            sizes: SizeSet { stat: 1_000, parsed: 1_000, ..SizeSet::default() },
        });
        g.chunks.push(Chunk {
            id: 0,
            names: vec!["main".into()],
            initial: true,
            assets: vec!["main.js".into()],
            size: SizeSet::default(),
        });
        let mut modules = BTreeMap::new();
        let m = module("./src/only.ts", 1_000);
        modules.insert(m.id.clone(), m);
        g.modules = modules;
        crate::stats::recompute_totals(&mut g);

        let raw = format!(
            r#"{{"version":3,"file":"main.js","sources":["src/only.ts"],
               "sourcesContent":["only"],"names":[],"mappings":"{}"}}"#,
            segment(0, 0, 0, 0)
        );
        let map = parse_bytes(raw.as_bytes()).unwrap();
        analyse(&mut g, &[("main.js.map".into(), map)]);
        assert_eq!(g.totals.size_dimension, SizeDimension::Attributed);
        assert!(g.fusion.as_ref().unwrap().ghost_modules.is_empty());
    }

    #[test]
    fn suffix_matching_is_segment_aware() {
        assert!(suffix_match("src/a.ts", "webpack://app/./src/a.ts"));
        assert!(suffix_match("/repo/src/a.ts", "src/a.ts"));
        assert!(!suffix_match("src/a.ts", "src/ab.ts"));
        assert!(!suffix_match("index.ts", "src/a.ts"));
    }

    #[test]
    fn ghost_precedence_is_stable() {
        assert_eq!(classify_ghost(10, None, false), Some(crate::model::GhostReason::MissingAsset));
        assert_eq!(classify_ghost(10, None, true), Some(crate::model::GhostReason::Unmapped));
        assert_eq!(
            classify_ghost(10, Some(0), true),
            Some(crate::model::GhostReason::EmptyAttribution)
        );
        assert_eq!(classify_ghost(10, Some(10), true), None);
    }

    #[test]
    fn empty_input_is_a_no_op_not_a_panic() {
        let mut g = UnifiedBundleGraph::new();
        let outcome = analyse(&mut g, &[]);
        assert_eq!(outcome.ghost_count, 0);
        assert!(g.fusion.is_some());
    }

    #[test]
    fn ghost_list_is_bounded_but_the_counts_are_exact() {
        let mut g = UnifiedBundleGraph::default();
        for i in 0..(GHOSTS_IN_SUMMARY * 3 + 17) {
            let id = format!("./src/m{i}.ts");
            g.modules.insert(
                id.clone(),
                Module {
                    id,
                    name: format!("./src/m{i}.ts"),
                    issuer: None,
                    package: None,
                    chunks: vec![0],
                    sources: Vec::new(),
                    reasons: vec!["orphan".into()],
                    sizes: crate::model::SizeSet { stat: 1_000 + i as u64, ..Default::default() },
                    attribution_delta: 0,
                },
            );
        }
        let outcome = analyse(&mut g, &[]);
        let f = g.fusion.as_ref().unwrap();
        assert_eq!(f.ghost_modules.len(), GHOSTS_IN_SUMMARY);
        assert_eq!(
            f.ghost_modules_truncated,
            (GHOSTS_IN_SUMMARY * 3 + 17 - GHOSTS_IN_SUMMARY) as u64
        );

        assert_eq!(outcome.ghost_count, GHOSTS_IN_SUMMARY * 3 + 17);
        let expected_bytes: u64 = (0..(GHOSTS_IN_SUMMARY * 3 + 17)).map(|i| 1_000 + i as u64).sum();
        assert_eq!(outcome.ghost_bytes, expected_bytes);

        assert!(f.ghost_modules[0].declared_size >= f.ghost_modules[1].declared_size);
    }

    #[test]
    fn webpack_url_sources_join_with_relative_module_names() {
        let map_path =
            crate::sourcemap::normalise_source_path("webpack://fixture/./src/module-0.ts", None);
        let mod_path = crate::sourcemap::normalise_source_path("./src/module-0.ts", None);
        assert!(
            suffix_match(&mod_path, &map_path),
            "normalised map path {map_path:?} should match module path {mod_path:?}"
        );
    }

    #[test]
    fn the_suffix_index_agrees_with_the_linear_scan() {
        use super::{PathIndex, matching_sources, matching_sources_indexed};
        let mut table: HashMap<String, u64> = HashMap::new();
        for (path, bytes) in [
            ("webpack://app/./src/a.ts", 100u64),
            ("webpack://app/./src/nested/b.ts", 200),
            ("webpack://app/./vendor/c.ts", 300),
            ("webpack://app/./src/nested/dup.ts", 50),
            ("webpack://other/./src/nested/dup.ts", 70),
        ] {
            table.insert(path.to_string(), bytes);
        }
        let index = PathIndex::build(&table);
        for name in [
            "./src/a.ts",
            "src/nested/b.ts",
            "./vendor/c.ts",
            "nested/dup.ts",
            "./src/nested/dup.ts",
            "./src/missing.ts",
            "a.ts",
        ] {
            let m = module(name, 1);
            let scanned = matching_sources(&m, &table);
            let indexed = matching_sources_indexed(&m, &index);
            assert_eq!(scanned, indexed, "index and scan disagree for {name}");
        }
    }

    #[test]
    fn maps_are_found_where_the_bundles_are_and_named_relatively() {
        let dir = std::env::temp_dir().join("nopeat-fusion-nested-maps");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        let map = br#"{"version":3,"file":"index-a1b2c3.js","sources":["../src/main.ts"],
            "names":[],"mappings":"AAAA","sourcesContent":["// main"]}"#;
        std::fs::write(dir.join("assets/index-a1b2c3.js.map"), map).unwrap();
        std::fs::write(dir.join("assets/index-a1b2c3.js"), b"console.log(1)").unwrap();
        std::fs::write(dir.join("top.js.map"), map).unwrap();

        let found = super::maps_in_dir(&dir);
        let names: Vec<&str> = found.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            vec!["assets/index-a1b2c3.js.map", "top.js.map"],
            "sorted, relative to the folder root"
        );

        let mut graph = UnifiedBundleGraph::new();
        graph.assets.push(crate::model::Asset {
            name: "assets/index-a1b2c3.js".into(),
            size: 14,
            chunks: Vec::new(),
            sizes: crate::model::SizeSet { stat: 0, parsed: 14, gzip: 0, attributed: None },
        });
        let joined = super::asset_for_map(&graph, &found[0].0);
        assert_eq!(joined, "assets/index-a1b2c3.js");
    }
}
