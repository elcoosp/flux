//! Criterion benchmark: end-to-end parity check over the Appendix B.3 examples.
//!
//! This is both a correctness gate and a performance gate — the dev→release parity
//! pass (compile + reduce + structural compare) must stay within the §3.6 budgets.

use criterion::{Criterion, criterion_group, criterion_main};

use flux_parity::{all_examples, check_parity};

fn bench_end_to_end(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end");
    for (name, src) in all_examples() {
        let name: &str = name;
        let file_id: u32 = match name {
            "b31_simple" => 31,
            "b32_generic" => 32,
            "b33_adt" => 33,
            "b34_lifecycle" => 34,
            "b35_navigation" => 35,
            "b36_async" => 36,
            "b37_pure" => 37,
            "b38_platform" => 38,
            "b39_capability" => 39,
            "b310_refs" => 310,
            _ => 0,
        };
        group.bench_with_input(name, &file_id, |b, &fid: &u32| {
            b.iter(|| {
                let report = check_parity(src, fid).expect("parity check succeeds");
                assert!(report.is_equivalent(), "parity must hold");
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_end_to_end);
criterion_main!(benches);
