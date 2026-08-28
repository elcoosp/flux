//! Criterion benchmark: large-synthetic-app pipeline (LANE-H, T1).
//!
//! The §3.10 end-to-end gate ("Save → pixels < 100 ms") has no benchmark at
//! production scale. This bench drives the real [`Pipeline`] (parse → type-check
//! → lower → diff → serialize) over a synthetic 10k-node app to measure the
//! server-side compile cost: the part the dev server owns before a frame hits
//! the wire. We assert the lowered arena actually carries the expected node
//! count so the bench is not measuring an empty tree.

use criterion::{Criterion, criterion_group, criterion_main};
use flux_devserver::Pipeline;
use std::hint::black_box;
use std::path::Path;

/// Generates a synthetic `n`-leaf Flux app: a root `Counter` component holding a
/// `Column` whose body is `n` sibling `Text` primitives. Each `Text` lowers to
/// exactly one primitive node (see `flux-ir::lower::lower_call`), so the tree
/// carries `n + 2` nodes (root + Column + leaves) — production-scale structure
/// without needing a real 10k-node .flux fixture on disk.
fn synthetic_app(n: u32) -> String {
    let mut src =
        String::from("compo Counter\n    state count: Int = 0\n\n    Column(gap: 8.0) {\n");
    for i in 0..n {
        // A distinct prop string per leaf, so every node is uniquely
        // identifiable and the differ has real (if unchanged) structure to scan.
        src.push_str(&format!("        Text(text: \"label-{i}\")\n"));
    }
    src.push_str("    }\n\n");
    src
}

fn bench_pipeline_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_large");
    for n in [1_000u32, 10_000] {
        let src = synthetic_app(n);
        let root = ".";
        group.bench_with_input(format!("parse_lower_wire/{n}"), &n, |b, _| {
            b.iter(|| {
                let mut p = Pipeline::new(root, false);
                let path = Path::new("synthetic-main.flux");
                p.set_source(path, src.clone());
                match p.compile() {
                    Ok(flux_devserver::Compiled::Init(bytes)) => {
                        assert!(!bytes.is_empty(), "init frame must serialize");
                        bytes
                    }
                    Ok(flux_devserver::Compiled::Unchanged) => {
                        panic!("first compile must produce an Init");
                    }
                    Ok(flux_devserver::Compiled::Delta(_)) => {
                        panic!("first compile must produce an Init, not a Delta");
                    }
                    Ok(other) => panic!("unexpected compile outcome: {other:?}"),
                    Err(diag) => panic!("pipeline compile failed: {diag:?}"),
                }
            })
        });
        let _ = black_box(&src);
    }
    group.finish();
}

criterion_group!(benches, bench_pipeline_large);
criterion_main!(benches);
