use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupNode {
    pub name: String,
    pub size: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<GroupNode>,

    pub module_count: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub dropped: u64,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(v: &u64) -> bool {
    *v == 0
}

pub const MAX_CHILDREN: usize = 256;

pub const MAX_ASSETS: usize = 64;
const OTHER: &str = "other";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    SourceFile,

    Package,

    Chunk,

    Extension,
}

impl Dimension {
    pub fn as_str(self) -> &'static str {
        match self {
            Dimension::SourceFile => "source_file",
            Dimension::Package => "package",
            Dimension::Chunk => "chunk",
            Dimension::Extension => "extension",
        }
    }
}

impl Dimension {
    fn key_for(self, m: &crate::model::Module) -> String {
        match self {
            Dimension::SourceFile => {
                m.sources.first().map_or_else(|| m.name.clone(), |s| s.file.clone())
            }
            Dimension::Package => {
                m.package.as_ref().map_or_else(|| "<app>".to_string(), |p| p.name.clone())
            }
            Dimension::Chunk => {
                if m.chunks.is_empty() {
                    "no chunk".to_string()
                } else {
                    let ids: Vec<String> = m.chunks.iter().map(u32::to_string).collect();
                    format!("chunk {}", ids.join("+"))
                }
            }
            Dimension::Extension => {
                let name = m.name.rsplit('/').next().unwrap_or(&m.name);
                let ext = name.rsplit_once('.').map_or("", |(_, e)| e).to_ascii_lowercase();
                if ext.is_empty() { "(no extension)".to_string() } else { format!(".{ext}") }
            }
        }
    }
}

pub fn treemap_tree(graph: &crate::model::UnifiedBundleGraph, dim: Dimension) -> GroupNode {
    let mut per_asset: BTreeMap<String, BTreeMap<String, (u64, u64)>> = BTreeMap::new();
    let mut loose: BTreeMap<String, (u64, u64)> = BTreeMap::new();

    let mut chunk_owner: BTreeMap<u32, &str> = BTreeMap::new();
    for asset in &graph.assets {
        for chunk in &asset.chunks {
            chunk_owner.entry(*chunk).or_insert(asset.name.as_str());
        }
    }

    for module in graph.modules.values() {
        let size = module.sizes.effective();
        let group = dim.key_for(module);

        let owner = module.chunks.iter().find_map(|c| chunk_owner.get(c).copied());

        let bucket = match owner {
            Some(asset) => per_asset.entry(asset.to_string()).or_default(),
            None => &mut loose,
        };
        bucket
            .entry(group)
            .and_modify(|(s, c)| {
                *s += size;
                *c += 1;
            })
            .or_insert((size, 1u64));
    }

    let mut children: Vec<GroupNode> = graph
        .assets
        .iter()
        .map(|asset| {
            let groups = per_asset.remove(&asset.name).unwrap_or_default();
            let (nodes, dropped) = groups_to_nodes(groups);
            let size: u64 = nodes.iter().map(|n| n.size).sum();
            let module_count: u64 = nodes.iter().map(|n| n.module_count).sum();
            GroupNode {
                name: asset.name.clone(),
                size: size.max(asset.sizes.effective()),
                module_count,
                children: nodes,
                dropped,
            }
        })
        .collect();

    if !loose.is_empty() {
        let (nodes, dropped) = groups_to_nodes(loose);
        let size = nodes.iter().map(|n| n.size).sum();
        let module_count = nodes.iter().map(|n| n.module_count).sum();
        children.push(GroupNode {
            name: "(modules outside any asset)".to_string(),
            size,
            module_count,
            children: nodes,
            dropped,
        });
    }

    let total: u64 = children.iter().map(|c| c.size).sum();
    let (children, folded_assets) = fold_asset_tail(children);

    GroupNode {
        name: "bundle".to_string(),
        size: total.max(graph.totals.total_size),
        module_count: graph.modules.len() as u64,
        dropped: children.iter().map(|c| c.dropped).sum::<u64>() + folded_assets.1,
        children,
    }
}

fn fold_asset_tail(mut nodes: Vec<GroupNode>) -> (Vec<GroupNode>, (u64, u64)) {
    if nodes.len() <= MAX_ASSETS {
        return (nodes, (0, 0));
    }
    nodes.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    let folded: Vec<GroupNode> = nodes.split_off(MAX_ASSETS);
    let (size, module_count) =
        folded.iter().fold((0u64, 0u64), |(s, m), n| (s + n.size, m + n.module_count));
    nodes.push(GroupNode {
        name: format!("(+{} more assets)", folded.len()),
        size,
        module_count,
        children: Vec::new(),
        dropped: 0,
    });
    (nodes, (folded.len() as u64, module_count))
}

fn groups_to_nodes(groups: BTreeMap<String, (u64, u64)>) -> (Vec<GroupNode>, u64) {
    let mut ordered: Vec<(String, u64, u64)> =
        groups.into_iter().map(|(k, (size, count))| (k, size, count)).collect();

    ordered.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut dropped = 0u64;
    let mut nodes: Vec<GroupNode> = Vec::new();
    for (name, size, count) in ordered.iter().take(MAX_CHILDREN) {
        nodes.push(GroupNode {
            name: name.clone(),
            size: *size,
            children: Vec::new(),
            module_count: *count,
            dropped: 0,
        });
    }
    for (_, size, count) in ordered.iter().skip(MAX_CHILDREN) {
        dropped += *count;
        match nodes.iter_mut().find(|n| n.name == OTHER) {
            Some(other) => {
                other.size += size;
                other.module_count += count;
            }
            None => nodes.push(GroupNode {
                name: OTHER.to_string(),
                size: *size,
                children: Vec::new(),
                module_count: *count,
                dropped: 0,
            }),
        }
    }
    (nodes, dropped)
}

mod detail;

pub use detail::{detail_payload, write_csv, write_detail};

#[cfg(test)]
mod tests {
    use super::{Dimension, MAX_ASSETS, MAX_CHILDREN, treemap_tree};
    use crate::model::{
        Asset, Chunk, Module, PackageRef, SizeDimension, SizeSet, UnifiedBundleGraph,
    };
    use std::collections::BTreeMap;

    fn module(name: &str, size: u64, chunks: Vec<u32>, pkg: Option<&str>) -> Module {
        Module {
            id: name.to_string(),
            name: name.to_string(),
            issuer: None,
            reasons: vec!["./src/app.js".to_string()],
            package: pkg.map(|p| {
                std::sync::Arc::new(PackageRef {
                    name: p.to_string(),
                    version: None,
                    path: format!("node_modules/{p}"),
                })
            }),
            chunks,
            sizes: SizeSet { stat: size, parsed: size, ..SizeSet::default() },
            attribution_delta: 0,
            sources: Vec::new(),
        }
    }

    fn fixture(module_count: usize) -> UnifiedBundleGraph {
        let mut g = UnifiedBundleGraph::new();
        g.chunks.push(Chunk {
            id: 0,
            names: vec!["main".into()],
            initial: true,
            assets: vec!["main.js".into()],
            size: SizeSet::default(),
        });
        let mut modules = BTreeMap::new();
        let mut asset_size = 0u64;
        for i in 0..module_count {
            let m = module(
                &format!("./node_modules/pkg{i}/i.js"),
                100 + i as u64,
                vec![0],
                Some(&format!("pkg{i}")),
            );
            asset_size += m.sizes.parsed;
            modules.insert(m.id.clone(), m);
        }

        g.assets.push(Asset {
            name: "main.js".into(),
            size: asset_size,
            chunks: vec![0],
            sizes: SizeSet { stat: asset_size, parsed: asset_size, ..SizeSet::default() },
        });
        g.modules = modules;
        crate::stats::recompute_totals(&mut g);
        g.totals.size_dimension = SizeDimension::Parsed;
        g
    }

    #[test]
    fn tree_is_two_levels_and_totals_match() {
        let g = fixture(30);
        let tree = treemap_tree(&g, Dimension::Package);
        assert_eq!(tree.children.len(), 1, "one asset");
        let asset = &tree.children[0];
        assert_eq!(asset.children.len(), 30, "one node per package");
        assert_eq!(tree.size, g.totals.total_size);
        assert_eq!(tree.module_count, 30);
    }

    #[test]
    fn children_are_capped_and_the_tail_is_visible() {
        let g = fixture(MAX_CHILDREN + 40);
        let tree = treemap_tree(&g, Dimension::Package);
        let asset = &tree.children[0];
        assert_eq!(asset.children.len(), MAX_CHILDREN + 1, "capped, plus the `other` node");
        let other = asset.children.iter().find(|n| n.name == "other").expect("other bucket");
        assert_eq!(other.module_count, 40);
        assert_eq!(asset.dropped, 40, "the cap must be reported, not hidden");
    }

    #[test]
    fn asset_level_is_capped_and_the_fold_is_visible() {
        let mut g = fixture(4);

        for i in 0..(MAX_ASSETS + 10) {
            let chunk_id = 100 + u32::try_from(i).unwrap_or(u32::MAX);
            g.chunks.push(Chunk {
                id: chunk_id,
                names: vec![format!("c{i}")],
                initial: false,
                assets: vec![format!("extra-{i}.js")],
                size: SizeSet::default(),
            });
            g.assets.push(Asset {
                name: format!("extra-{i}.js"),
                size: 10,
                chunks: vec![chunk_id],
                sizes: SizeSet { stat: 10, parsed: 10, ..SizeSet::default() },
            });

            let m = module(
                &format!("./node_modules/extra{i}/i.js"),
                10,
                vec![chunk_id],
                Some(&format!("extra{i}")),
            );
            g.modules.insert(m.id.clone(), m);
        }
        let tree = treemap_tree(&g, Dimension::Package);
        assert!(tree.children.len() <= MAX_ASSETS + 1, "asset level must be capped");
        let folded =
            tree.children.iter().find(|c| c.name.starts_with("(+")).expect("folded asset node");
        assert!(folded.size >= 100, "folded node keeps the bytes: {}", folded.size);
        assert!(tree.dropped >= 10, "the fold is reported, not hidden");
    }

    #[test]
    fn chunk_dimension_groups_by_chunk_id() {
        let g = fixture(3);
        let tree = treemap_tree(&g, Dimension::Chunk);
        assert_eq!(tree.children[0].children.len(), 1);
        assert_eq!(tree.children[0].children[0].name, "chunk 0");
    }

    #[test]
    fn extension_dimension_groups_by_lowercase_file_extension() {
        let mut g = UnifiedBundleGraph::new();
        g.chunks.push(Chunk {
            id: 0,
            names: vec!["main".into()],
            initial: true,
            assets: vec!["main.js".into()],
            size: SizeSet::default(),
        });
        for (name, size) in [
            ("./src/app.tsx", 400u64),
            ("./src/util.TS", 300),
            ("./node_modules/pkg/index.js", 200),
            ("./LICENSE", 100),
            ("./v1.2/README", 50),
        ] {
            g.modules.insert(
                name.to_string(),
                Module {
                    id: name.to_string(),
                    name: name.to_string(),
                    issuer: None,
                    reasons: vec![],
                    package: None,
                    chunks: vec![0],
                    sizes: SizeSet { stat: size, parsed: size, ..SizeSet::default() },
                    attribution_delta: 0,
                    sources: Vec::new(),
                },
            );
        }
        crate::stats::recompute_totals(&mut g);
        g.totals.size_dimension = SizeDimension::Parsed;

        let tree = treemap_tree(&g, Dimension::Extension);
        let names: Vec<&str> = tree.children[0].children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec![".tsx", ".ts", ".js", "(no extension)"]);
        let ts = tree.children[0].children.iter().find(|c| c.name == ".ts").unwrap();
        assert_eq!(ts.size, 300, "extension match is case-insensitive");
        let none = tree.children[0].children.iter().find(|c| c.name == "(no extension)").unwrap();
        assert_eq!(none.size, 150, "version dots in directories do not become extensions");
    }

    #[test]
    fn package_dimension_falls_back_to_app() {
        let mut g = fixture(1);
        g.modules.clear();
        let m = module("./src/app.js", 500, vec![0], None);
        g.modules.insert(m.id.clone(), m);
        crate::stats::recompute_totals(&mut g);
        let tree = treemap_tree(&g, Dimension::Package);
        assert_eq!(tree.children[0].children[0].name, "<app>");
    }
}
