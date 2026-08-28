//! Criterion benchmark harness for `flux-types`.
//!
//! Performance budget (AGENTS.md §3.6): type-checking a 500-line file must
//! complete in under 3 ms. This bench parses a ~500-line fixture once, then
//! type-checks it repeatedly so the timed region is dominated by the checker
//! rather than the parser.

use criterion::{Criterion, criterion_group, criterion_main};
use flux_parser::parse;
use flux_types::type_check;
use std::hint::black_box;
use std::time::Instant;

/// Asserts `work` completes in under `budget_us` on average over a warm-up plus
/// fixed measurement window, so a regression against the §3.6 budgets fails CI
/// loudly instead of silently drifting.
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

/// A ~500-line well-typed program. Shared machinery (a trait, an ADT + match
/// fn, and one generic component) is defined once; then ~15 uniquely-named
/// leaf components are repeated to reach the budget's line count. Each leaf is
/// self-contained and type-checks, so the fixture as a whole is valid.
fn fixture_source() -> String {
    let mut src = String::from(
        "trait Numeric[T] {\n  fn zero() -> T\n  fn one() -> T\n}\n\n\
         type Shape = Circle(Float) | Rectangle(Float, Float)\n\n\
         fn area(shape: Shape) -> Float {\n  match shape {\n    Circle(r) => r * 3.14\n    Rectangle(w, h) => w * h\n  }\n}\n\n",
    );
    // Each leaf component is ~10 lines; 52 repeats ≈ 520 lines + the shared
    // machinery ≈ 545 lines total. Components use the indent-based surface
    // syntax (ADR-0029); the shared `trait`/`type`/`fn` keep their brace syntax.
    for i in 0..52 {
        src.push_str(&format!(
            "compo Leaf{i}[T: Numeric]\n  state count: T = Numeric.zero()\n  state label: String = \"n{i}\"\n  Button(text: \"Tap: {{count}}\")\n  Text(\"label: {{label}}\")\n  let total = area(Circle(5.0))\n  Text(\"total: {{total}}\")\n  Text(label)\n  Text(\"count\")\n\n",
        ));
    }
    src
}

fn typecheck_benchmark(criterion: &mut Criterion) {
    let source = fixture_source();
    let ast = parse(&source, 1, "bench.flux").expect("fixture must parse");
    assert!(
        source.lines().count() >= 500,
        "fixture must be at least 500 lines (budget target), got {}",
        source.lines().count()
    );
    // §3.6 budget: type-checking a 500-line file must complete in under 3 ms.
    assert_within_budget_us("type_check", 3_000, || {
        type_check(black_box(&ast)).expect("fixture must type-check")
    });

    let mut group = criterion.benchmark_group("type_check");
    group.sample_size(100);
    group.measurement_time(std::time::Duration::from_secs(10));
    group.bench_function("500_line_file", |bencher| {
        bencher.iter(|| {
            let result = type_check(std::hint::black_box(&ast)).expect("fixture must type-check");
            std::hint::black_box(result);
        })
    });
    group.finish();
}

criterion_group!(benches, typecheck_benchmark);
criterion_main!(benches);
