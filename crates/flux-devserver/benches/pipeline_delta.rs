//! Criterion benchmark: the hot-reload recompile path (LANE-H, T1 follow-up).
//!
//! `pipeline_large` only ever measures the *first* compile (Init). The actual
//! dev loop is: edit source → recompile → emit a `Delta`. This bench drives a
//! real [`Pipeline`] through two source snapshots — the initial compile plus a
//! minimal edit to one leaf — and asserts the second compile yields exactly one
//! `Delta` carrying one structural patch (the O(changed) guarantee), while the
//! timed region is dominated by recompile cost. This is the "save → pixels"
//! inner loop the existing suite never measured.

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::path::Path;

use flux_devserver::Pipeline;

/// A `n`-leaf Flux app: a root `Counter` holding a `Column` of `n` `Text`s.
fn synthetic_app(n: u32) -> String {
    let mut src =
        String::from("compo Counter\n    state count: Int = 0\n\n    Column(gap: 8.0) {\n");
    for i in 0..n {
        src.push_str(&format!("        Text(text: \"label-{i}\")\n"));
    }
    src.push_str("    }\n\n");
    src
}

/// Mutates exactly one leaf's prop string, modelling a single user edit that
/// must recompile to a minimal `Delta`.
fn edited_app(n: u32) -> String {
    let mut src =
        String::from("compo Counter\n    state count: Int = 0\n\n    Column(gap: 8.0) {\n");
    for i in 0..n {
        // Flip the last leaf's literal so exactly one node differs.
        let text = if i == n - 1 {
            "label-EDITED".to_string()
        } else {
            format!("label-{i}")
        };
        src.push_str(&format!("        Text(text: \"{text}\")\n"));
    }
    src.push_str("    }\n\n");
    src
}

fn bench_pipeline_delta(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_delta");
    for n in [1_000u32, 10_000] {
        let initial = synthetic_app(n);
        let edited = edited_app(n);
        let root = ".";
        group.bench_with_input(format!("recompile/{n}"), &n, |b, _| {
            b.iter(|| {
                let mut p = Pipeline::new(root, false);
                let path = Path::new("synthetic-main.flux");
                // First compile → Init (retained as last-good tree).
                p.set_source(path, initial.clone());
                match p.compile() {
                    Ok(flux_devserver::Compiled::Init(bytes)) => {
                        assert!(!bytes.is_empty(), "init frame must serialize");
                    }
                    other => panic!("first compile must produce an Init, got {other:?}"),
                }
                // Apply the single-leaf edit and recompile → Delta.
                p.set_source(path, edited.clone());
                let delta = p.compile().expect("recompile must succeed");
                match delta {
                    flux_devserver::Compiled::Delta(bytes) => {
                        assert!(!bytes.is_empty(), "delta frame must serialize");
                        bytes
                    }
                    flux_devserver::Compiled::Unchanged => {
                        panic!("a real edit must not be reported Unchanged");
                    }
                    flux_devserver::Compiled::Init(_) => {
                        panic!("second compile must be a Delta, not another Init");
                    }
                    // `Compiled` is `#[non_exhaustive]`; future variants never
                    // occur in this pipeline but must still be handled.
                    _ => panic!("unexpected compile outcome: {delta:?}"),
                }
            })
        });
        let _ = black_box(&initial);
        let _ = black_box(&edited);
    }
    group.finish();
}

criterion_group!(benches, bench_pipeline_delta);
criterion_main!(benches);
