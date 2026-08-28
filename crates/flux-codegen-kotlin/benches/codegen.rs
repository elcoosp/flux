//! Criterion benchmark for the release `codegen` path (FLUX-021, Kotlin/Compose).
//!
//! See the sibling `flux-codegen-swift/benches/codegen.rs` — this is the
//! Kotlin/Compose counterpart so both release backends are covered symmetrically.

use criterion::{Criterion, criterion_group, criterion_main};
use flux_ir::lower;
use flux_parser::parse;
use flux_types::type_check;

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

fn bench_codegen_kotlin(c: &mut Criterion) {
    let mut group = c.benchmark_group("codegen_kotlin");
    for n in [100u32, 1_000] {
        let source = synthetic_app(n);
        let ast = parse(&source, 0, "bench.flux").expect("fixture must parse");
        let typed = type_check(&ast).expect("fixture must type-check");
        let lowered = lower(&ast, &typed).expect("fixture must lower");
        assert!(
            lowered.arena.all_ids().count() > n as usize,
            "lowered arena must carry the generated nodes"
        );
        group.bench_with_input(format!("emit/{n}"), &n, |b, _| {
            b.iter(|| {
                let code = flux_codegen_kotlin::codegen(
                    std::hint::black_box(&lowered),
                    std::hint::black_box(&ast),
                );
                assert!(!code.is_empty(), "generated kotlin must be non-empty");
                std::hint::black_box(code)
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_codegen_kotlin);
criterion_main!(benches);
