use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use rayon::prelude::*;

use crate::Result;
use crate::model::{SizeSet, UnifiedBundleGraph};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Gzip,
    Brotli,
    Zstd,
}

impl Compression {
    pub fn as_str(self) -> &'static str {
        match self {
            Compression::Gzip => "gzip",
            Compression::Brotli => "brotli",
            Compression::Zstd => "zstd",
        }
    }
}

pub fn attribute_from_disk(
    graph: &mut UnifiedBundleGraph,
    dir: &Path,
    compression: Compression,
) -> Result<()> {
    let targets: Vec<(usize, String, std::path::PathBuf)> = graph
        .assets
        .iter()
        .enumerate()
        .map(|(i, a)| (i, a.name.clone(), dir.join(&a.name)))
        .collect();

    let measured: Vec<(usize, u64, u64)> = targets
        .par_iter()
        .map(|(idx, _name, path)| match std::fs::read(path) {
            Ok(bytes) => (*idx, bytes.len() as u64, compressed_size(&bytes, compression)),

            Err(_) => (*idx, 0, 0),
        })
        .collect();

    let mut missing = Vec::new();
    for (idx, parsed, gzip) in measured {
        let asset = &mut graph.assets[idx];
        if parsed == 0 {
            missing.push(asset.name.clone());
            continue;
        }
        asset.sizes.parsed = parsed;
        asset.sizes.gzip = gzip;
    }

    for chunk in &mut graph.chunks {
        let mut stat = 0u64;
        let mut parsed = 0u64;
        let mut gzip = 0u64;
        for name in &chunk.assets {
            if let Some(a) = graph.assets.iter().find(|a| &a.name == name) {
                stat += a.sizes.stat;
                parsed += a.sizes.parsed;
                gzip += a.sizes.gzip;
            }
        }
        chunk.size = SizeSet { stat, parsed, gzip, attributed: None };
    }

    if !missing.is_empty() {
        missing.truncate(5);
        graph.diagnostics.push(crate::model::Diagnostic {
            severity: crate::model::Severity::Info,
            code: "NPT0002".into(),
            message: format!(
                "{} asset(s) not found next to the metadata: {}{}",
                missing.len(),
                missing.join(", "),
                if missing.len() == 5 { ", …" } else { "" }
            ),
            subject: None,
            data: serde_json::Value::Null,
        });
    }

    propagate_asset_sizes_to_modules(graph);
    Ok(())
}

fn propagate_asset_sizes_to_modules(graph: &mut UnifiedBundleGraph) {
    let scale: BTreeMap<u32, f64> = graph
        .chunks
        .iter()
        .map(|chunk| {
            let stat: u64 = chunk
                .assets
                .iter()
                .filter_map(|n| graph.assets.iter().find(|a| &a.name == n))
                .map(|a| a.sizes.stat)
                .sum();
            let parsed: u64 = chunk
                .assets
                .iter()
                .filter_map(|n| graph.assets.iter().find(|a| &a.name == n))
                .map(|a| a.sizes.parsed)
                .sum();
            let factor = if stat > 0 && parsed > 0 { parsed as f64 / stat as f64 } else { 1.0 };
            (chunk.id, factor)
        })
        .collect();

    for module in graph.modules.values_mut() {
        let mut sum = 0.0_f64;
        let mut measured_chunks = 0usize;
        for chunk_id in &module.chunks {
            if let Some(factor) = scale.get(chunk_id) {
                sum += *factor;
                measured_chunks += 1;
            }
        }
        let factor = if measured_chunks == 0 { 1.0 } else { sum / measured_chunks as f64 };

        #[allow(clippy::float_cmp, clippy::if_not_else)]
        if factor != 1.0 {
            let scaled = (module.sizes.stat as f64 * factor).round();
            module.sizes.parsed = if scaled.is_finite() && scaled > 0.0 {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                {
                    scaled.min(u64::MAX as f64) as u64
                }
            } else {
                module.sizes.stat
            };
        } else {
            module.sizes.parsed = module.sizes.stat;
        }
    }
}

pub fn gzip_size(bytes: &[u8]) -> u64 {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(6));

    let _ = enc.write_all(bytes);
    enc.finish().map_or(0, |v| v.len() as u64)
}

pub fn compressed_size(bytes: &[u8], algo: Compression) -> u64 {
    match algo {
        Compression::Gzip => gzip_size(bytes),
        Compression::Brotli => brotli_size(bytes),
        Compression::Zstd => zstd_size(bytes),
    }
}

fn brotli_size(bytes: &[u8]) -> u64 {
    let mut out = Vec::new();
    let mut enc = brotli::CompressorWriter::new(&mut out, 4096, 11, 22);
    if enc.write_all(bytes).is_err() || enc.flush().is_err() {
        return 0;
    }
    drop(enc);
    out.len() as u64
}

fn zstd_size(bytes: &[u8]) -> u64 {
    zstd::stream::encode_all(bytes, 3).map_or(0, |v| v.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::{Compression, attribute_from_disk, compressed_size, gzip_size};
    use crate::model::{Asset, Chunk, Module, SizeSet, UnifiedBundleGraph};
    use std::collections::BTreeMap;

    #[test]
    fn gzip_level_6_is_byte_stable() {
        let small = b"the quick brown fox jumps over the lazy dog";
        assert_eq!(gzip_size(small), gzip_size(small), "must be deterministic");

        let repetitive = vec![b'a'; 64 * 1024];
        let gz = gzip_size(&repetitive);
        assert!(gz > 0 && gz < repetitive.len() as u64 / 100, "64 KB of 'a' must compress hard");
        assert_eq!(gz, gzip_size(&repetitive), "must be deterministic at scale");
    }

    #[test]
    fn every_compression_algorithm_is_deterministic_and_actually_compresses() {
        let repetitive = vec![b'a'; 64 * 1024];
        for algo in [Compression::Gzip, Compression::Brotli, Compression::Zstd] {
            let first = compressed_size(&repetitive, algo);
            assert!(first > 0 && first < repetitive.len() as u64 / 100, "{algo:?}: {first}");
            assert_eq!(first, compressed_size(&repetitive, algo), "{algo:?} must be deterministic");
        }
        let small = b"the quick brown fox jumps over the lazy dog";
        let gzip = compressed_size(small, Compression::Gzip);
        let brotli = compressed_size(small, Compression::Brotli);
        let zstd = compressed_size(small, Compression::Zstd);
        assert!(gzip > 0 && brotli > 0 && zstd > 0);
        assert!(
            brotli <= gzip && zstd <= gzip,
            "brotli ({brotli}) and zstd ({zstd}) should not lose to gzip ({gzip}) on compressible text"
        );
    }

    #[test]
    fn missing_assets_become_a_diagnostic_not_a_failure() {
        let mut g = UnifiedBundleGraph::new();
        g.assets.push(Asset {
            name: "gone.js".into(),
            size: 10,
            chunks: vec![0],
            sizes: SizeSet { stat: 10, ..SizeSet::default() },
        });
        g.chunks.push(Chunk {
            id: 0,
            names: vec!["main".into()],
            initial: true,
            assets: vec!["gone.js".into()],
            size: SizeSet::default(),
        });
        attribute_from_disk(
            &mut g,
            std::path::Path::new("/definitely/not/here"),
            Compression::Gzip,
        )
        .unwrap();
        assert!(g.diagnostics.iter().any(|d| d.code == "NPT0002"));
    }

    #[test]
    fn measured_asset_sizes_propagate_to_chunks_and_modules() {
        let dir = std::env::temp_dir().join("nopeat-sizes-test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.js"), vec![b'a'; 4096]).unwrap();

        let mut g = UnifiedBundleGraph::new();
        g.assets.push(Asset {
            name: "main.js".into(),
            size: 1000,
            chunks: vec![0],
            sizes: SizeSet { stat: 1000, ..SizeSet::default() },
        });
        g.chunks.push(Chunk {
            id: 0,
            names: vec!["main".into()],
            initial: true,
            assets: vec!["main.js".into()],
            size: SizeSet::default(),
        });
        let mut modules = BTreeMap::new();
        for (name, stat) in [("./a.js", 600u64), ("./b.js", 400)] {
            let m = Module {
                id: name.into(),
                name: name.into(),
                issuer: None,
                reasons: vec![],
                package: None,
                chunks: vec![0],
                sizes: SizeSet { stat, ..SizeSet::default() },
                attribution_delta: 0,
                sources: vec![],
            };
            modules.insert(m.id.clone(), m);
        }
        g.modules = modules;

        attribute_from_disk(&mut g, &dir, Compression::Gzip).unwrap();

        assert_eq!(g.assets[0].sizes.parsed, 4096);
        assert!(g.assets[0].sizes.gzip > 0);
        assert_eq!(g.chunks[0].size.parsed, 4096);

        let sum: u64 = g.modules.values().map(|m| m.sizes.parsed).sum();
        assert_eq!(sum, 4096, "modules must reconcile with the measured asset");
        assert!(g.check_size_invariant(1_000).is_ok());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_bundle_dir_is_reported_as_a_diagnostic_not_a_panic() {
        let mut g = UnifiedBundleGraph::new();
        g.assets.push(Asset {
            name: "main.js".into(),
            size: 10,
            chunks: vec![0],
            sizes: SizeSet { stat: 10, ..SizeSet::default() },
        });

        let result = attribute_from_disk(
            &mut g,
            std::path::Path::new("/definitely/not/here"),
            Compression::Gzip,
        );
        assert!(result.is_ok(), "a missing dir is data, not a crash: {result:?}");
        assert!(g.diagnostics.iter().any(|d| d.code == "NPT0002"));
        assert_eq!(g.assets[0].sizes.parsed, 0, "an unmeasured asset stays 0, not a guess");
    }
}
