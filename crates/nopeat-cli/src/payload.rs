use nopeat_core::model::UnifiedBundleGraph;
use nopeat_core::report::Dimension;

use crate::cli_args::Sizes;
use crate::discover::Input;
use crate::pipeline::dimension_label;

pub fn build_payload(
    graph: &UnifiedBundleGraph,
    input: &Input,
    dims: &[Dimension],
    sizes: Sizes,
    include_modules: bool,
) -> serde_json::Value {
    let mut tree = serde_json::Map::new();
    tree.insert("schema_version".into(), graph.schema_version.into());
    tree.insert("target".into(), input.label().into());
    tree.insert("sizeDimension".into(), dimension_label(sizes).into());
    tree.insert("totals".into(), serde_json::to_value(&graph.totals).unwrap_or_default());
    tree.insert("inputs".into(), serde_json::to_value(&graph.inputs).unwrap_or_default());
    tree.insert("diagnostics".into(), serde_json::to_value(&graph.diagnostics).unwrap_or_default());
    tree.insert("assets".into(), serde_json::to_value(&graph.assets).unwrap_or_default());
    tree.insert("chunks".into(), serde_json::to_value(&graph.chunks).unwrap_or_default());

    if include_modules {
        tree.insert("modules".into(), serde_json::to_value(&graph.modules).unwrap_or_default());
    }
    tree.insert(
        "trees".into(),
        serde_json::to_value(
            dims.iter()
                .map(|d| {
                    serde_json::json!({
                        "dimension": d.as_str(),
                        "tree": nopeat_core::report::treemap_tree(graph, *d),
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default(),
    );
    tree.insert("fusion".into(), serde_json::to_value(&graph.fusion).unwrap_or_default());
    serde_json::Value::Object(tree)
}
