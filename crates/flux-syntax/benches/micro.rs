//! Criterion micro-benchmarks for the two hottest primitive operations in the
//! pipeline hot path, neither of which was previously benched:
//!
//! * [`compute_node_id`] — called once per node in every lowered tree (a 10k-node
//!   app calls it 10k times per compile), so its per-call cost multiplies
//!   directly into the §3.10 parse/lower budgets.
//! * [`StringTable::intern`] — called per distinct string during lowering and
//!   during wire `Init` frame assembly; a regression here silently slows every
//!   tree with many strings.

use criterion::{Criterion, criterion_group, criterion_main};
use flux_syntax::{ExprTag, Span, StringTable, compute_node_id};

/// A representative span: file 1, a short token range.
fn span(index: u32) -> Span {
    Span::new(1, index * 4, index * 4 + 3)
}

fn bench_compute_node_id(c: &mut Criterion) {
    c.bench_function("compute_node_id", |b| {
        b.iter(|| {
            let id = compute_node_id(0, ExprTag(7), span(42), None);
            std::hint::black_box(id)
        });
    });
}

fn bench_string_table_intern(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_table_intern");
    for count in [50usize, 1_000] {
        let strings: Vec<String> = (0..count).map(|i| format!("label-{i}")).collect();
        group.bench_with_input(format!("intern/{count}"), &count, |b, _| {
            b.iter(|| {
                let mut table = StringTable::new();
                for s in std::hint::black_box(&strings) {
                    table.intern(s);
                }
                assert_eq!(table.len(), count, "every distinct string is interned once");
                std::hint::black_box(table)
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_compute_node_id, bench_string_table_intern);
criterion_main!(benches);
