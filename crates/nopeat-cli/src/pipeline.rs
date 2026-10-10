use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};

use nopeat_core::model::UnifiedBundleGraph;
use nopeat_core::report::Dimension;
use nopeat_core::sizes;
use nopeat_core::stats;

use crate::budget::{BudgetConfig, evaluate_budget};
use crate::cli_args::{Cli, Dims, Level, Mode, Sizes};
use crate::discover::{Input, discover, file_name, read_head, tool_for};
use crate::payload::build_payload;
use crate::report;

pub fn run(cli: &Cli) -> Result<ExitCode> {
    crate::cli_args::validate(cli)?;
    if cli.bench_map {
        return crate::bench::bench_source_map(&cli.path);
    }
    if cli.mode == Mode::Server && !cli.json {
        return crate::server::run_server(cli);
    }
    let mode = if cli.json { Mode::Json } else { cli.mode };

    let started = std::time::Instant::now();
    let found = discover(&cli.path)?;
    emit(cli, Level::Debug, &format!("input: {}", found.label()));
    let phase = std::time::Instant::now();
    let mut fusion: Option<(nopeat_core::fusion::FusionOutcome, usize)> = None;

    let graph = build_graph(cli, &found, &mut fusion)?;

    let ingest_ms = phase.elapsed().as_millis();
    let mut graph = graph;
    if !cli.include.is_empty() {
        apply_includes(&mut graph, &cli.include)?;
    }
    if !cli.exclude.is_empty() {
        apply_excludes(&mut graph, &cli.exclude)?;
    }
    if let Some(min_size) = cli.min_size {
        apply_min_size(&mut graph, min_size);
    }
    stats::recompute_totals(&mut graph);

    let mut budget_failed = false;
    let mut budget_config_error = false;
    if let Some(config_path) = &cli.budget {
        let mut text = String::new();
        {
            use std::io::Read as _;
            let mut reader = nopeat_core::bom::BomSkip::new(
                std::fs::File::open(config_path)
                    .with_context(|| format!("opening {}", config_path.display()))?,
            );
            reader
                .read_to_string(&mut text)
                .with_context(|| format!("reading {}", config_path.display()))?;
        }
        let config: BudgetConfig = serde_json::from_str(&text)
            .with_context(|| format!("parsing {}", config_path.display()))?;
        let (errors, ok, breaches) = evaluate_budget(&graph, &config);
        if !errors.is_empty() {
            budget_config_error = true;
        }
        for e in errors {
            emit(cli, Level::Error, &format!("nopeat: {e}"));
        }
        for d in &breaches {
            emit(cli, Level::Error, &format!("nopeat: {} {}", d.code, d.message));
            graph.diagnostics.push(d.clone());
        }
        budget_failed = !ok;
    }

    if cli.bench {
        let total_ms = started.elapsed().as_millis();
        println!(
            "{{\"tool\":\"nopeat@{}\",\"input\":\"{}\",\"ingest_ms\":{},\"total_ms\":{},\"modules\":{},\"assets\":{},\"packages\":{},\"total_size\":{},\"dimension\":\"{}\"}}",
            env!("CARGO_PKG_VERSION"),
            found.label(),
            ingest_ms,
            total_ms,
            graph.totals.module_count,
            graph.totals.asset_count,
            graph.totals.package_count,
            graph.totals.total_size,
            dimension_label(cli.default_sizes),
        );
        return Ok(ExitCode::SUCCESS);
    }

    let dims: Vec<Dimension> = dims_selected(cli);

    let payload = build_payload(&graph, &found, &dims, cli.default_sizes, mode == Mode::Json);

    match mode {
        Mode::Json => match &cli.report {
            Some(out) => {
                let json = serde_json::to_string_pretty(&payload)?;
                std::fs::write(out, format!("{json}\n"))
                    .with_context(|| format!("writing {}", out.display()))?;
                emit(cli, Level::Info, &format!("wrote {}", out.display()));
            }
            None => println!("{}", serde_json::to_string_pretty(&payload)?),
        },
        Mode::Static => {
            write_static_report(cli, &graph, payload, fusion.as_ref(), started, ingest_ms)?;
        }
        Mode::Server => unreachable!("server mode is handled before the pipeline"),
    }

    if budget_config_error {
        return Ok(ExitCode::from(3));
    }
    Ok(if budget_failed { ExitCode::from(1) } else { ExitCode::SUCCESS })
}

/// Write the HTML report and everything the terminal says about it.
///
/// Split out of `run` so that function stays a readable sequence of steps. The
/// order here is the order the output appears in.
#[allow(clippy::cast_precision_loss)]
fn write_static_report(
    cli: &Cli,
    graph: &UnifiedBundleGraph,
    payload: serde_json::Value,
    fusion: Option<&(nopeat_core::fusion::FusionOutcome, usize)>,
    started: std::time::Instant,
    ingest_ms: u128,
) -> Result<()> {
    let label = payload.get("target").and_then(|v| v.as_str()).unwrap_or("bundle").to_string();
    let title = cli.title.as_deref().unwrap_or(&label);
    let out = cli.report.clone().unwrap_or_else(|| PathBuf::from("nopeat-report.html"));

    let used = dimension_used(graph);
    let requested = dimension_label(cli.default_sizes);
    let mut payload = payload;
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("sizeDimension".into(), used.into());
        obj.insert("compression".into(), cli.compression_algorithm.as_str().into());
    }

    let detail_bytes = report::write_detail_file(graph, &out)?;
    report::write(&payload, &label, title, &out, detail_bytes)?;
    if used != requested {
        let reason = if used == "attributed" {
            "source maps cover the build, so this is ground truth"
        } else {
            "that dimension is not measurable for this input"
        };
        emit(
            cli,
            Level::Info,
            &format!("showing `{used}` instead of `{requested}`: {reason} (NPT0050)"),
        );
    }
    let report_bytes = std::fs::metadata(&out).map_or(0, |m| m.len());
    emit(
        cli,
        Level::Info,
        &format!(
            "{}{}  ·  {} modules  ·  {} assets  ·  {} packages  ·  ingest {} ms  ·  total {} ms  ·  dimension {}",
            label,
            compressed_summary(&graph.assets, cli.compression_algorithm),
            graph.totals.module_count,
            graph.totals.asset_count,
            graph.totals.package_count,
            ingest_ms,
            started.elapsed().as_millis(),
            used,
        ),
    );
    if let Some(&(outcome, maps)) = fusion {
        let ghost = if outcome.ghosts_detectable {
            format!(
                "{} ghost ({} of declared)",
                outcome.ghost_count,
                human_bytes(outcome.ghost_bytes)
            )
        } else {
            "ghost code needs a stats.json to detect".to_string()
        };
        emit(
            cli,
            Level::Info,
            &format!(
                "fusion: {maps} map(s) · coverage {:.0}% · {}/{} modules attributed · {ghost} · {} hidden source(s) ({})",
                outcome.coverage * 100.0,
                outcome.attributed_modules,
                graph.totals.module_count,
                outcome.hidden_count,
                human_bytes(outcome.hidden_bytes),
            ),
        );
    }

    let report_mb = report_bytes as f64 / 1_048_576.0;
    let csv_note = if let Some(csv_path) = &cli.csv {
        let mut out = std::io::BufWriter::new(
            std::fs::File::create(csv_path)
                .with_context(|| format!("creating {}", csv_path.display()))?,
        );
        nopeat_core::report::write_csv(graph, &mut out)
            .with_context(|| format!("writing {}", csv_path.display()))?;
        format!("  ·  csv {}", csv_path.display())
    } else {
        String::new()
    };
    emit(
        cli,
        Level::Info,
        &format!(
            "wrote {} ({report_mb:.1} MB){csv_note}{}",
            out.display(),
            if detail_bytes <= report::INLINE_LIMIT as u64 {
                ", detail inlined"
            } else {
                ", detail in a companion script (loaded on demand)"
            }
        ),
    );
    Ok(())
}

pub fn build_graph(
    cli: &Cli,
    found: &Input,
    fusion: &mut Option<(nopeat_core::fusion::FusionOutcome, usize)>,
) -> Result<UnifiedBundleGraph> {
    match found {
        Input::File(path) => {
            let head = read_head(path, 512 * 1024)?;
            let name = file_name(path);
            let tool = tool_for(path, &head);
            let mut graph =
                stats::ingest_file(path, tool).with_context(|| format!("parsing {name}"))?;

            let dir = cli.bundle_dir.clone().or_else(|| path.parent().map(Path::to_path_buf));
            if let Some(dir) = &dir {
                let explicit = cli.bundle_dir.is_some();
                let resolves = graph.assets.iter().any(|a| dir.join(&a.name).is_file());
                if explicit || resolves {
                    sizes::attribute_from_disk(
                        &mut graph,
                        dir,
                        cli.compression_algorithm.to_core(),
                    )?;

                    let (maps, unreadable) = nopeat_core::fusion::maps_in_dir_detailed(dir);
                    for why in &unreadable {
                        emit(cli, Level::Warn, &format!("could not read {why}"));
                    }
                    if !maps.is_empty() {
                        let outcome = if matches!(tool, "webpack" | "esbuild") {
                            nopeat_core::fusion::analyse(&mut graph, &maps)
                        } else {
                            nopeat_core::fusion::analyse_sources_only(&mut graph, &maps)
                        };
                        *fusion = Some((outcome, maps.len()));
                    }
                }
            }
            Ok(graph)
        }
        Input::Folder(dir) => {
            let candidates = [
                "stats.json",
                "stats.stats.json",
                "metafile.json",
                "manifest.json",
                ".vite/manifest.json",
                "build-manifest.json",
                "app-build-manifest.json",
            ];
            let picked = candidates.iter().map(|n| dir.join(n)).find(|p| p.is_file());

            let tool =
                picked.as_ref().map(|p| tool_for(p, &read_head(p, 512 * 1024).unwrap_or_default()));
            let has_module_graph = matches!(tool, Some("webpack" | "esbuild"));

            let mut graph = if let Some(path) = &picked {
                stats::ingest_file(path, tool.unwrap_or("webpack"))?
            } else {
                let mut graph = nopeat_core::folder::ingest(dir).with_context(|| {
                    format!(
                        "no bundler metadata (stats.json, metafile.json, manifest.json, \
                         build-manifest.json) in {}, and it holds no build output either",
                        dir.display()
                    )
                })?;

                let report_name = cli
                    .report
                    .as_ref()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
                if let Some(name) = &report_name {
                    graph.assets.retain(|a| &a.name != name);

                    if let Some(stem) = name.strip_suffix(".html") {
                        let companion = format!("{stem}.data.js");
                        graph.assets.retain(|a| a.name != companion);
                    }
                }
                graph
            };
            sizes::attribute_from_disk(&mut graph, dir, cli.compression_algorithm.to_core())?;

            let (maps, unreadable) = nopeat_core::fusion::maps_in_dir_detailed(dir);
            for why in &unreadable {
                emit(cli, Level::Warn, &format!("could not read {why}"));
            }
            if !maps.is_empty() {
                let outcome = if has_module_graph {
                    nopeat_core::fusion::analyse(&mut graph, &maps)
                } else {
                    nopeat_core::fusion::analyse_sources_only(&mut graph, &maps)
                };
                *fusion = Some((outcome, maps.len()));
            }
            Ok(graph)
        }
    }
}

pub fn dims_selected(cli: &Cli) -> Vec<Dimension> {
    if cli.dims.is_empty() {
        vec![Dimension::Package, Dimension::SourceFile, Dimension::Chunk, Dimension::Extension]
    } else {
        cli.dims
            .iter()
            .map(|d| match d {
                Dims::Package => Dimension::Package,
                Dims::Source => Dimension::SourceFile,
                Dims::Chunk => Dimension::Chunk,
                Dims::Ext => Dimension::Extension,
            })
            .collect()
    }
}

pub fn dimension_used(graph: &UnifiedBundleGraph) -> &'static str {
    match graph.totals.size_dimension {
        nopeat_core::model::SizeDimension::Stat => "stat",
        nopeat_core::model::SizeDimension::Parsed => "parsed",
        nopeat_core::model::SizeDimension::Gzip => "gzip",
        nopeat_core::model::SizeDimension::Attributed => "attributed",
    }
}

pub fn dimension_label(s: Sizes) -> &'static str {
    match s {
        Sizes::Stat => "stat",
        Sizes::Parsed => "parsed",
        Sizes::Gzip | Sizes::Brotli | Sizes::Zstd => "gzip",
        Sizes::Attributed => "attributed",
    }
}

pub fn emit(cli: &Cli, level: Level, message: &str) {
    if cli.log_level.shows(level) {
        match level {
            Level::Error => eprintln!("{message}"),
            Level::Warn | Level::Info | Level::Debug => println!("{message}"),
        }
    }
}

#[allow(clippy::cast_precision_loss)]
pub fn human_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    if n < 1024 {
        return format!("{n} B");
    }
    let mb = n as f64 / (1024.0 * 1024.0);
    if mb >= 1.0 { format!("{mb:.1} MB") } else { format!("{:.0} KB", n as f64 / KB) }
}

pub fn apply_excludes(graph: &mut UnifiedBundleGraph, patterns: &[String]) -> Result<()> {
    let mut compiled = Vec::new();
    for p in patterns {
        let re = regex::Regex::new(p).with_context(|| format!("invalid --exclude regex `{p}`"))?;
        compiled.push(re);
    }
    if compiled.is_empty() {
        return Ok(());
    }
    graph.assets.retain(|a| !compiled.iter().any(|m| m.is_match(&a.name)));
    graph.chunks.retain(|c| {
        !compiled.iter().any(|m| {
            c.names.iter().any(|n| m.is_match(n)) || c.assets.iter().any(|a| m.is_match(a))
        })
    });
    let kept: Vec<u32> = graph.chunks.iter().map(|c| c.id).collect();
    graph.modules.retain(|_, m| m.chunks.iter().any(|c| kept.contains(c)) || m.chunks.is_empty());
    Ok(())
}

pub fn apply_includes(graph: &mut UnifiedBundleGraph, patterns: &[String]) -> Result<()> {
    let mut compiled = Vec::new();
    for p in patterns {
        let re = regex::Regex::new(p).with_context(|| format!("invalid --include regex `{p}`"))?;
        compiled.push(re);
    }
    if compiled.is_empty() {
        return Ok(());
    }
    let keep = |name: &str| compiled.iter().any(|m| m.is_match(name));
    graph.assets.retain(|a| keep(&a.name));
    graph.chunks.retain(|c| c.names.iter().any(|n| keep(n)) || c.assets.iter().any(|a| keep(a)));
    let kept: Vec<u32> = graph.chunks.iter().map(|c| c.id).collect();
    graph.modules.retain(|_, m| m.chunks.iter().any(|c| kept.contains(c)) || m.chunks.is_empty());
    Ok(())
}

pub fn apply_min_size(graph: &mut UnifiedBundleGraph, min_size: u64) -> u64 {
    if min_size == 0 {
        return 0;
    }
    let mut dropped_bytes = 0u64;
    let mut dropped_names: Vec<String> = Vec::new();
    let mut dropped_count = 0u64;
    graph.modules.retain(|_, m| {
        let size = m.sizes.effective();
        if size < min_size {
            dropped_count += 1;
            dropped_bytes += size;
            if dropped_names.len() < 5 {
                dropped_names.push(m.name.clone());
            }
            false
        } else {
            true
        }
    });
    if dropped_count > 0 {
        // `dropped_names` is capped at five, so this is only ever about deciding whether
        // to add an ellipsis. Comparing the counts in `u64` avoids the cast that a
        // 32-bit target would truncate.
        let preview = if (dropped_names.len() as u64) < dropped_count {
            format!("{}, …", dropped_names.join(", "))
        } else {
            dropped_names.join(", ")
        };
        graph.diagnostics.push(nopeat_core::model::Diagnostic {
            severity: nopeat_core::model::Severity::Info,
            code: "NPT0060".into(),
            message: format!(
                "filtered {dropped_count} module(s) below {min_size} bytes ({}) : {preview}",
                human_bytes(dropped_bytes)
            ),
            subject: None,
            data: serde_json::json!({ "modules": dropped_count, "bytes": dropped_bytes, "min_size": min_size }),
        });
    }
    dropped_count
}

pub fn compressed_summary(
    assets: &[nopeat_core::model::Asset],
    algo: crate::cli_args::Compression,
) -> String {
    let measured = assets.iter().filter(|a| a.sizes.gzip > 0).count();
    match (measured, assets.len()) {
        (0, _) => String::new(),
        (m, t) if m == t => {
            let total: u64 = assets.iter().map(|a| a.sizes.gzip).sum();
            format!("  ·  {} {}", algo.as_str(), human_bytes(total))
        }
        (m, t) => format!("  ·  {} (partial {m}/{t})", algo.as_str()),
    }
}

#[cfg(test)]
mod exclude_tests {
    use super::{apply_excludes, apply_includes, apply_min_size, compressed_summary};
    use crate::cli_args::Compression;
    use nopeat_core::model::{Asset, Chunk, SizeSet, UnifiedBundleGraph};

    fn graph() -> UnifiedBundleGraph {
        let mut g = UnifiedBundleGraph::new();
        g.assets.push(Asset {
            name: "dist/main.js".into(),
            size: 10,
            chunks: vec![0],
            sizes: SizeSet::default(),
        });
        g.assets.push(Asset {
            name: "dist/x.min.js".into(),
            size: 10,
            chunks: vec![1],
            sizes: SizeSet::default(),
        });
        g.chunks.push(Chunk {
            id: 0,
            names: vec!["main".into()],
            initial: true,
            assets: vec!["dist/main.js".into()],
            size: SizeSet::default(),
        });
        g.chunks.push(Chunk {
            id: 1,
            names: vec!["vendor".into()],
            initial: false,
            assets: vec!["dist/x.min.js".into()],
            size: SizeSet::default(),
        });
        g
    }

    #[test]
    fn regex_exclude_removes_matching_assets_and_their_chunks() {
        let mut g = graph();
        apply_excludes(&mut g, &["\\.min\\.js$".to_string()]).unwrap();
        assert_eq!(g.assets.len(), 1);
        assert_eq!(g.assets[0].name, "dist/main.js");
        assert_eq!(g.chunks.len(), 1);
        assert_eq!(g.chunks[0].id, 0);
    }

    #[test]
    fn regex_is_case_sensitive_and_supports_classes() {
        let mut g = graph();
        apply_excludes(&mut g, &["^dist/[a-z]+\\.js$".to_string()]).unwrap();
        assert!(g.assets.iter().all(|a| a.name != "dist/main.js"));
        assert!(g.assets.iter().any(|a| a.name == "dist/x.min.js"));
    }

    #[test]
    fn an_invalid_pattern_is_an_error_not_a_silent_pass() {
        let mut g = graph();
        let err = apply_excludes(&mut g, &["([bad".to_string()]).unwrap_err();
        assert!(err.to_string().contains("invalid --exclude regex"), "{err}");
        assert_eq!(g.assets.len(), 2);
    }

    #[test]
    fn include_keeps_only_matching_assets() {
        let mut g = graph();
        apply_includes(&mut g, &["\\.min\\.js$".to_string()]).unwrap();
        assert_eq!(g.assets.len(), 1);
        assert_eq!(g.assets[0].name, "dist/x.min.js");
        assert_eq!(g.chunks.len(), 1);
        assert_eq!(g.chunks[0].id, 1);
    }

    #[test]
    fn include_and_exclude_compose() {
        let mut g = graph();
        apply_includes(&mut g, &["^dist/".to_string()]).unwrap();
        apply_excludes(&mut g, &["\\.min\\.js$".to_string()]).unwrap();
        assert_eq!(g.assets.len(), 1);
        assert_eq!(g.assets[0].name, "dist/main.js");
    }

    #[test]
    fn an_invalid_include_pattern_is_an_error() {
        let mut g = graph();
        let err = apply_includes(&mut g, &["([bad".to_string()]).unwrap_err();
        assert!(err.to_string().contains("invalid --include regex"), "{err}");
        assert_eq!(g.assets.len(), 2);
    }

    #[test]
    fn min_size_drops_only_small_modules_and_reports_it() {
        let mut g = graph_with_modules(vec![("big.js", 5_000), ("tiny.js", 10)]);
        let dropped = apply_min_size(&mut g, 1_000);
        assert_eq!(dropped, 1);
        assert!(!g.modules.contains_key("tiny.js"));
        assert!(g.modules.contains_key("big.js"));
        assert!(g.diagnostics.iter().any(|d| d.code == "NPT0060"));
    }

    #[test]
    fn min_size_zero_is_a_no_op() {
        let mut g = graph_with_modules(vec![("a.js", 1)]);
        assert_eq!(apply_min_size(&mut g, 0), 0);
        assert_eq!(g.modules.len(), 1);
        assert!(g.diagnostics.is_empty());
    }

    #[test]
    fn compressed_summary_names_the_algorithm_and_has_three_states() {
        use nopeat_core::model::{Asset, SizeSet};
        let mk = |gzip: u64| Asset {
            name: "a.js".into(),
            size: 10,
            chunks: vec![],
            sizes: SizeSet { gzip, ..SizeSet::default() },
        };
        assert_eq!(compressed_summary(&[], Compression::Gzip), "");
        assert_eq!(compressed_summary(&[mk(0)], Compression::Gzip), "");
        assert_eq!(
            compressed_summary(&[mk(100), mk(0)], Compression::Gzip),
            "  ·  gzip (partial 1/2)"
        );
        assert_eq!(compressed_summary(&[mk(100), mk(24)], Compression::Gzip), "  ·  gzip 124 B");
        assert_eq!(
            compressed_summary(&[mk(100), mk(24)], Compression::Brotli),
            "  ·  brotli 124 B"
        );
        assert_eq!(compressed_summary(&[mk(100), mk(24)], Compression::Zstd), "  ·  zstd 124 B");
    }

    fn graph_with_modules(modules: Vec<(&str, u64)>) -> UnifiedBundleGraph {
        use nopeat_core::model::{Module, SizeSet};
        let mut g = UnifiedBundleGraph::new();
        for (name, size) in modules {
            g.modules.insert(
                name.to_string(),
                Module {
                    id: name.to_string(),
                    name: name.to_string(),
                    issuer: None,
                    reasons: vec![],
                    package: None,
                    chunks: vec![],
                    sizes: SizeSet { stat: size, ..SizeSet::default() },
                    attribution_delta: 0,
                    sources: vec![],
                },
            );
        }
        g
    }
}
