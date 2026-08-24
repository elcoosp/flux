//! Criterion benchmark for `flux-parser` against the AGENTS.md §3.6 budget:
//! parsing a 500-line file must take under 5 ms.

use criterion::{Criterion, criterion_group, criterion_main};
use flux_parser::parse;

/// Components emitted per repetition of the synthetic source below.
const LINES_PER_COMPONENT: usize = 10;

/// Builds a source file of roughly `lines` lines exercising the constructs a
/// real Flux file uses: state, interpolation, handlers, conditionals, lists.
fn source_of(lines: usize) -> String {
    let repeats = lines / LINES_PER_COMPONENT;
    let mut source = String::with_capacity(lines * 40);
    for index in 0..repeats {
        source.push_str(&format!(
            "component Screen{index} {{\n  \
             state count: Int = {index}\n  \
             state label: String = \"screen {index}\"\n  \
             Column(gap: 12) {{\n    \
             Text(\"{{label}}: {{count}}\")\n    \
             Button(text: \"inc\", onClick: {{ count = count + 1 }})\n    \
             when count > 3 {{\n      Text(\"many\")\n    }}\n  }}\n}}\n"
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
        group.bench_function(format!("{lines}_lines"), |bencher| {
            bencher.iter(|| parse(&source, 0, "bench.flux").expect("benchmark source parses"));
        });
    }
    group.finish();
}

criterion_group!(benches, parse_benchmark);
criterion_main!(benches);
