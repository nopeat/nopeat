use std::collections::BTreeMap;

pub fn detail_payload(graph: &crate::model::UnifiedBundleGraph) -> serde_json::Value {
    let mut modules = serde_json::Map::new();
    for module in graph.modules.values() {
        modules.insert(
            module.id.clone(),
            serde_json::json!({
                "name": module.name,
                "package": module.package.as_ref().map(|p| p.name.clone()),
                "chunks": module.chunks,
                "reasons": module.reasons,
                "sources": module.sources.iter().map(|s| s.file.clone()).collect::<Vec<_>>(),
                "sizes": module.sizes,
                "attribution_delta": module.attribution_delta,
            }),
        );
    }
    serde_json::json!({
        "schema_version": graph.schema_version,
        "modules": modules,
        "fusion": graph.fusion,
    })
}

pub fn write_detail<W: std::io::Write>(
    graph: &crate::model::UnifiedBundleGraph,
    out: &mut W,
) -> std::io::Result<u64> {
    serde_json::to_writer(
        &mut *out,
        &Detail {
            schema_version: graph.schema_version,
            modules: DetailModules(&graph.modules),
            fusion: &graph.fusion,
        },
    )
    .map_err(std::io::Error::other)?;

    out.write_all(b"\n")?;
    Ok(0)
}

#[derive(serde::Serialize)]
struct Detail<'a> {
    schema_version: u32,
    modules: DetailModules<'a>,
    fusion: &'a Option<crate::model::FusionSummary>,
}

struct DetailModules<'a>(&'a BTreeMap<String, crate::model::Module>);

impl serde::Serialize for DetailModules<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (id, module) in self.0 {
            map.serialize_entry(
                id,
                &DetailModule {
                    name: &module.name,
                    package: module.package.as_ref().map(|p| p.name.as_str()),
                    chunks: &module.chunks,
                    reasons: &module.reasons,
                    sources: module.sources.iter().map(|s| s.file.as_str()).collect(),
                    sizes: module.sizes,
                    attribution_delta: module.attribution_delta,
                },
            )?;
        }
        map.end()
    }
}

#[derive(serde::Serialize)]
struct DetailModule<'a> {
    name: &'a str,
    package: Option<&'a str>,
    chunks: &'a [u32],
    reasons: &'a [String],
    sources: Vec<&'a str>,
    sizes: crate::model::SizeSet,
    attribution_delta: i64,
}

pub fn write_csv<W: std::io::Write>(
    graph: &crate::model::UnifiedBundleGraph,
    out: &mut W,
) -> std::io::Result<()> {
    writeln!(out, "module_id,name,package,chunks,stat,parsed,gzip,attributed,delta")?;
    for (id, module) in &graph.modules {
        let package = module.package.as_ref().map_or("", |p| p.name.as_str());
        let chunks = module.chunks.iter().map(u32::to_string).collect::<Vec<_>>().join("|");
        writeln!(
            out,
            "{},{},{},{},{},{},{},{},{}",
            csv_field(id),
            csv_field(&module.name),
            csv_field(package),
            chunks,
            module.sizes.stat,
            module.sizes.parsed,
            module.sizes.gzip,
            module.sizes.attributed.map(|v| v.to_string()).unwrap_or_default(),
            module.attribution_delta,
        )?;
    }
    Ok(())
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
