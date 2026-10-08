use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedBundleGraph {
    pub schema_version: u32,

    pub inputs: Vec<InputArtifact>,
    pub assets: Vec<Asset>,
    pub chunks: Vec<Chunk>,
    pub modules: BTreeMap<String, Module>,

    pub totals: Totals,
    pub fusion: Option<FusionSummary>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputArtifact {
    Stats { tool: String },

    SourceMap { name: String },

    EsbuildMetafile,

    VisualizerStats { tool: String },

    ViteManifest,

    NextManifest,

    DistFolder,
}

impl InputArtifact {
    pub fn trusts_exact_sizes(&self) -> bool {
        matches!(self, InputArtifact::SourceMap { .. })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,

    pub size: u64,
    pub chunks: Vec<u32>,

    pub sizes: SizeSet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: u32,
    pub names: Vec<String>,

    pub initial: bool,
    pub assets: Vec<String>,
    pub size: SizeSet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub id: String,
    pub name: String,
    pub issuer: Option<String>,

    pub reasons: Vec<String>,

    pub package: Option<std::sync::Arc<PackageRef>>,
    pub chunks: Vec<u32>,
    pub sizes: SizeSet,

    pub attribution_delta: i64,
    pub sources: Vec<SourceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageRef {
    pub name: String,
    pub version: Option<String>,

    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceRef {
    pub file: String,

    pub bytes: u64,

    pub line: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SizeSet {
    pub stat: u64,

    pub parsed: u64,

    pub gzip: u64,

    pub attributed: Option<u64>,
}

impl SizeSet {
    pub fn effective(&self) -> u64 {
        if let Some(attributed) = self.attributed.filter(|v| *v > 0) {
            return attributed;
        }
        if self.parsed > 0 { self.parsed } else { self.stat }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Totals {
    pub total_size: u64,
    pub module_count: u64,
    pub asset_count: u64,
    pub package_count: u64,

    pub size_dimension: SizeDimension,

    pub module_size_sum: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeDimension {
    Stat,
    #[default]
    Parsed,
    Gzip,
    Attributed,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FusionSummary {
    pub ghost_modules: Vec<GhostModule>,

    pub ghost_modules_truncated: u64,

    pub hidden_sources: Vec<HiddenSource>,

    pub hidden_sources_truncated: u64,

    pub total_attribution_delta: i64,

    pub significant_corrections: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostModule {
    pub module: String,
    pub declared_size: u64,
    pub chunks: Vec<u32>,

    pub reason: GhostReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GhostReason {
    Unmapped,

    EmptyAttribution,

    MissingAsset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiddenSource {
    pub file: String,
    pub bytes: u64,

    pub location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,

    pub subject: Option<String>,

    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl UnifiedBundleGraph {
    pub const SCHEMA_VERSION: u32 = 1;

    pub fn new() -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            inputs: Vec::new(),
            assets: Vec::new(),
            chunks: Vec::new(),
            modules: BTreeMap::new(),
            totals: Totals::default(),
            fusion: None,
            diagnostics: Vec::new(),
        }
    }

    pub fn check_size_invariant(&self, tolerance_ppm: u64) -> Result<(), super::Error> {
        if self.totals.size_dimension != SizeDimension::Attributed {
            return Ok(());
        }
        let total = self.totals.total_size;
        if total == 0 {
            return Ok(());
        }
        let drift = self.totals.module_size_sum.abs_diff(total);
        let allowed = u128::from(total) * u128::from(tolerance_ppm);
        if u128::from(drift) * 1_000_000 > allowed {
            return Err(super::Error::Invariant(format!(
                "module sizes sum to {} but total is {total} (drift {drift} > allowed {allowed} ppm-scale)",
                self.totals.module_size_sum
            )));
        }
        Ok(())
    }
}

impl Default for UnifiedBundleGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{SizeDimension, SizeSet, UnifiedBundleGraph};

    #[test]
    fn new_graph_is_at_the_current_schema_version() {
        assert_eq!(UnifiedBundleGraph::new().schema_version, UnifiedBundleGraph::SCHEMA_VERSION);
    }

    #[test]
    fn size_preference_is_ground_truth_then_measurement_then_claim() {
        let mut sizes = SizeSet { stat: 10, parsed: 20, gzip: 5, attributed: None };
        assert_eq!(sizes.effective(), 20, "measurement wins over the claim");
        sizes.attributed = Some(30);
        assert_eq!(sizes.effective(), 30, "ground truth wins over everything");
        sizes.parsed = 0;
        assert_eq!(sizes.effective(), 30);
        sizes.attributed = None;
        assert_eq!(sizes.effective(), 10, "falls back to the claim");
    }

    #[test]
    fn invariant_only_applies_to_attributed_totals() {
        let mut graph = UnifiedBundleGraph::new();
        graph.totals.total_size = 100_000;
        graph.totals.module_size_sum = 50;
        graph.totals.size_dimension = SizeDimension::Parsed;
        assert!(graph.check_size_invariant(1_000).is_ok());

        graph.totals.size_dimension = SizeDimension::Attributed;
        graph.totals.module_size_sum = 50;
        assert!(graph.check_size_invariant(1_000).is_err());

        graph.totals.module_size_sum = 100_050;
        assert!(graph.check_size_invariant(1_000).is_ok());
        graph.totals.module_size_sum = 100_200;
        assert!(graph.check_size_invariant(1_000).is_err());
    }
}
