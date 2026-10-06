use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result};

use crate::discover::file_name;

pub fn bench_source_map(path: &Path) -> Result<ExitCode> {
    let started = std::time::Instant::now();
    let dir = path.parent();
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;

    let parse_started = std::time::Instant::now();
    let map = nopeat_core::sourcemap::parse_reader(file)
        .with_context(|| format!("parsing {}", path.display()))?;
    let parse_ms = parse_started.elapsed().as_millis();

    let attr_started = std::time::Instant::now();
    let by_path = map.attribution_by_path(dir);
    let attributed_total: u64 = by_path.values().sum();
    let attribute_ms = attr_started.elapsed().as_millis();

    println!(
        "{{\"tool\":\"nopeat@{}\",\"input\":\"{}\",\"parse_ms\":{},\"attribute_ms\":{},\"total_ms\":{},\"sources\":{},\"mappings\":{},\"attributed_files\":{},\"attributed_total\":{}}}",
        env!("CARGO_PKG_VERSION"),
        file_name(path),
        parse_ms,
        attribute_ms,
        started.elapsed().as_millis(),
        map.sources.len(),
        map.mappings.len(),
        by_path.len(),
        attributed_total,
    );
    Ok(ExitCode::SUCCESS)
}
