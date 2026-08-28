//! Criterion benchmark for `flux-parser` against the AGENTS.md §3.6 budget:
//! parsing a 500-line file must take under 5 ms.

use criterion::{Criterion, criterion_group, criterion_main};
use flux_parser::parse;
use std::hint::black_box;
use std::time::Instant;

/// Asserts `work` completes in under `budget_us` on average over a warm-up plus
/// fixed measurement window. Runs once per bench invocation (independent of
/// criterion's own sampling) so a regression fails CI loudly rather than
/// silently drifting against the §3.6 / §3.10 budgets.
const WARMUP: u32 = 10;
const MEASURE: u32 = 200;

fn assert_within_budget_us<F, T>(name: &str, budget_us: u128, work: F)
where
    F: Fn() -> T,
{
    for _ in 0..WARMUP {
        black_box(work());
    }
    let started = Instant::now();
    for _ in 0..MEASURE {
        black_box(work());
    }
    let avg_us = started.elapsed().as_micros() / u128::from(MEASURE);
    assert!(
        avg_us < budget_us,
        "{name} averaged {avg_us} µs, over the §3.6 budget of {budget_us} µs"
    );
}

/// Components emitted per repetition of the synthetic source below.
const LINES_PER_COMPONENT: usize = 10;

/// Builds a source file of roughly `lines` lines exercising the constructs a
/// real Flux file uses: state, interpolation, handlers, conditionals, lists.
/// Components use the indent-based surface syntax (ADR-0029); containers such
/// as `Column`/`when`/handler bodies keep their brace syntax.
fn source_of(lines: usize) -> String {
    let repeats = lines / LINES_PER_COMPONENT;
    let mut source = String::with_capacity(lines * 40);
    for index in 0..repeats {
        source.push_str(&format!(
            "compo Screen{index}\n  \
             state count: Int = {index}\n  \
             state label: String = \"screen {index}\"\n  \
             Column(gap: 12) {{\n    \
             Text(\"{{label}}: {{count}}\")\n    \
             Button(text: \"inc\", onClick: || {{ count = count + 1 }})\n    \
             when count > 3 {{\n      Text(\"many\")\n    }}\n  }}\n\n"
        ));
    }
    source
}

fn parse_benchmark(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("parse");
    for lines in [100usize, 500] {
        let source = source_of(lines);
        assert!(
            parse(&source, 0, "bench.flux").is_ok(),
            "the benchmark source must parse"
        );
        // §3.6 budget: parsing a 500-line file must complete in under 5 ms. The
        // 100-line source is a strict subset and must also clear the cap.
        assert_within_budget_us("parse", 5_000, || parse(&source, 0, "bench.flux"));
        group.bench_function(format!("{lines}_lines"), |bencher| {
            bencher.iter(|| parse(&source, 0, "bench.flux").expect("benchmark source parses"));
        });
    }
    group.finish();
}

criterion_group!(benches, parse_benchmark);
criterion_main!(benches);
