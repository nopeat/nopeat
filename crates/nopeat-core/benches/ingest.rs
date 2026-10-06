use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use nopeat_core::stats;

fn generate(modules: usize, assets: usize, source_bytes: usize) -> String {
    use std::fmt::Write as _;

    let mut out = String::with_capacity(modules * 64 + assets * 128);
    out.push_str(r#"{"version":"5.90.0","assets":["#);
    for i in 0..assets {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            r#"{{"type":"asset","name":"chunk.{i}.js","size":{},"chunks":[{}],"emitted":true}}"#,
            20_000 + i * 37,
            i % 200
        );
    }
    out.push_str(r#"],"chunks":[],"modules":["#);
    let source = "function f(a,b){return a+b*2-1;}".repeat(source_bytes / 31 + 1);
    for i in 0..modules {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            r#"{{"id":{i},"identifier":"/repo/node_modules/pkg{}/dist/index-{i}.js","name":"./node_modules/pkg{}/dist/index-{i}.js","size":{},"chunks":[{}],"reasons":[],"source":"{}"}}"#,
            i % 400,
            i % 400,
            1700 + (i % 900),
            i % 200,
            source
        );
    }
    out.push_str("]}");
    out
}

fn ingest_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("ingest");

    for modules in [8_000usize, 64_000] {
        let json = generate(modules, 1_500, 1_700);
        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_with_input(BenchmarkId::new("stats", modules), &json, |b, json| {
            b.iter(|| {
                let graph =
                    stats::ingest_reader(json.as_bytes(), "webpack").expect("valid fixture");
                black_box(graph.modules.len())
            });
        });
    }
    group.finish();
}

fn ingest_metafile(c: &mut Criterion) {
    let json = r#"{"inputs":{"src/app.tsx":{"bytes":120000,"imports":[]}},"outputs":{},"version":"0.19.0"}"#;
    c.bench_function("metafile/small", |b| {
        b.iter(|| {
            let graph = stats::ingest_reader(black_box(json.as_bytes()), "esbuild")
                .expect("valid metafile");
            black_box(graph.modules.len())
        });
    });
}

criterion_group!(benches, ingest_stats, ingest_metafile);
criterion_main!(benches);
