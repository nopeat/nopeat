pub const SCHEMA_VERSION: u32 = nopeat_core::model::UnifiedBundleGraph::SCHEMA_VERSION;

pub fn schema_version() -> u32 {
    nopeat_core::model::UnifiedBundleGraph::new().schema_version
}

#[cfg(test)]
mod tests {
    #[test]
    fn core_is_reachable_from_the_wasm_surface() {
        assert_eq!(super::schema_version(), super::SCHEMA_VERSION);
    }
}
