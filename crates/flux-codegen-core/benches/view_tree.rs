//! Criterion benchmark for the shared release codegen core (FLUX-047).
//!
//! The shared emitter (`flux-codegen-core`) owns the language-neutral structural
//! walk that both backends consume. This bench lowers a synthetic `n`-leaf app
//! once, then builds the ADR-0027 [`Bridge`] and walks the structural
//! [`view_tree`] repeatedly — the part every release compile pays for regardless
//! of the Swift/Kotlin backend chosen.

use criterion::{Criterion, criterion_group, criterion_main};
use flux_codegen_core::{Bridge, view_tree};
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

fn bench_view_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("view_tree");
    for n in [100u32, 1_000] {
        let source = synthetic_app(n);
        let ast = parse(&source, 0, "bench.flux").expect("fixture must parse");
        let typed = type_check(&ast).expect("fixture must type-check");
        let lowered = lower(&ast, &typed).expect("fixture must lower");
        let bridge = Bridge::build(&ast);
        group.bench_with_input(format!("walk/{n}"), &n, |b, _| {
            b.iter(|| {
                let roots = view_tree(
                    std::hint::black_box(&lowered),
                    std::hint::black_box(&bridge),
                );
                assert!(!roots.is_empty(), "view tree must have a root component");
                std::hint::black_box(roots)
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_view_tree);
criterion_main!(benches);
