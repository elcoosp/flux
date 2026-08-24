//! Criterion benchmark harness for `flux-types`.
//!
//! Performance budget (AGENTS.md §3.6): type-checking a 500-line file must
//! complete in under 3 ms. This bench parses a ~500-line fixture once, then
//! type-checks it repeatedly so the timed region is dominated by the checker
//! rather than the parser.

use criterion::{Criterion, criterion_group, criterion_main};
use flux_parser::parse;
use flux_types::type_check;

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
    // Each leaf component is ~11 lines; 48 repeats ≈ 528 lines + the shared
    // machinery ≈ 545 lines total.
    for i in 0..48 {
        src.push_str(&format!(
            "component Leaf{i}[T: Numeric](initial: T) {{\n  state count: T = initial\n  state label: String = \"n{i}\"\n  Button(text: \"Tap: {{count}}\")\n  Text(\"label: {{label}}\")\n  let total = area(Circle(5.0))\n  Text(\"total: {{total}}\")\n  Text(label)\n  Text(\"count\")\n}}\n\n",
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
