use anyhow::Result;

use nopeat_core::model::UnifiedBundleGraph;

use crate::pipeline::human_bytes;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetConfig {
    pub limits: Vec<BudgetLimit>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetLimit {
    pub scope: String,

    #[serde(default)]
    pub r#match: Option<String>,

    pub max: u64,

    #[serde(default)]
    pub dimension: Option<String>,
}

fn size_for(sizes: &nopeat_core::model::SizeSet, dimension: Option<&str>) -> Result<u64, String> {
    match dimension {
        None => Ok(sizes.effective()),
        Some("stat") => Ok(sizes.stat),
        Some("parsed") => Ok(sizes.parsed),
        Some("gzip") => Ok(sizes.gzip),
        Some("attributed") => sizes.attributed.ok_or_else(|| {
            "dimension `attributed` is not available: no source map covered this build".into()
        }),
        Some(other) => {
            Err(format!("unknown dimension `{other}` (expected stat, parsed, gzip or attributed)"))
        }
    }
}

fn dimension_name(dimension: Option<&str>) -> String {
    dimension.unwrap_or("(report default)").to_string()
}

#[allow(clippy::too_many_lines)]
pub fn evaluate_budget(
    graph: &UnifiedBundleGraph,
    config: &BudgetConfig,
) -> (Vec<anyhow::Error>, bool, Vec<nopeat_core::model::Diagnostic>) {
    let mut errors = Vec::new();

    if config.limits.is_empty() {
        errors.push(anyhow::anyhow!(
            "budget: the config has no limits, so nothing would be checked. \
             Add at least one, for example {{\"limits\":[{{\"scope\":\"total\",\"max\":{}}}]}}",
            graph.totals.total_size
        ));
    }
    let mut breached = 0usize;
    let mut diagnostics = Vec::new();

    for limit in &config.limits {
        let dimension = limit.dimension.as_deref();
        let (scope, value) = match limit.scope.as_str() {
            "total" => {
                let value = if dimension.is_none() {
                    graph.totals.total_size
                } else {
                    let mut sum = 0u64;
                    let mut usable = true;
                    for asset in &graph.assets {
                        match size_for(&asset.sizes, dimension) {
                            Ok(v) => sum = sum.saturating_add(v),
                            Err(e) => {
                                errors.push(anyhow::anyhow!("budget: {e}"));
                                usable = false;
                                break;
                            }
                        }
                    }
                    if usable { sum } else { continue }
                };
                (limit.scope.clone(), value)
            }
            "chunk" => {
                let Some(needle) = limit.r#match.as_deref() else {
                    errors.push(anyhow::anyhow!("budget: `chunk` scope needs a `match`"));
                    continue;
                };
                let mut sum = 0u64;
                let mut matched = 0usize;
                for chunk in &graph.chunks {
                    if chunk.names.iter().any(|n| n == needle) {
                        match size_for(&chunk.size, dimension) {
                            Ok(v) => sum = sum.saturating_add(v),
                            Err(e) => {
                                errors.push(anyhow::anyhow!("budget: {e}"));
                                continue;
                            }
                        }
                        matched += 1;
                    }
                }
                if matched == 0 {
                    errors.push(anyhow::anyhow!(
                        "budget: no chunk matches `{needle}`; a rule that matches nothing is an error"
                    ));
                    continue;
                }
                (limit.scope.clone(), sum)
            }
            "package" => {
                let Some(needle) = limit.r#match.as_deref() else {
                    errors.push(anyhow::anyhow!("budget: `package` scope needs a `match`"));
                    continue;
                };
                let mut sum = 0u64;
                let mut matched = 0usize;
                for module in graph.modules.values() {
                    if module.package.as_ref().is_some_and(|p| {
                        p.name == needle || p.name.starts_with(&format!("{needle}/"))
                    }) {
                        match size_for(&module.sizes, dimension) {
                            Ok(v) => sum = sum.saturating_add(v),
                            Err(e) => {
                                errors.push(anyhow::anyhow!("budget: {e}"));
                                continue;
                            }
                        }
                        matched += 1;
                    }
                }
                if matched == 0 {
                    errors.push(anyhow::anyhow!(
                        "budget: no package matches `{needle}`; a rule that matches nothing is an error"
                    ));
                    continue;
                }
                (limit.scope.clone(), sum)
            }
            other => {
                errors.push(anyhow::anyhow!(
                    "budget: unknown scope `{other}` (expected total, chunk or package)"
                ));
                continue;
            }
        };

        if value > limit.max {
            breached += 1;
            diagnostics.push(nopeat_core::model::Diagnostic {
                severity: nopeat_core::model::Severity::Error,
                code: "NPT0040".into(),
                message: format!(
                    "budget {scope}{} on {} is {} over its {} limit",
                    limit.r#match.as_ref().map(|m| format!(" `{m}`")).unwrap_or_default(),
                    dimension_name(dimension),
                    human_bytes(value - limit.max),
                    human_bytes(limit.max)
                ),
                subject: None,
                data: serde_json::json!({
                    "scope": scope,
                    "dimension": dimension.unwrap_or("report default"),
                    "actual": value,
                    "max": limit.max
                }),
            });
        }
    }

    (errors, breached == 0, diagnostics)
}
