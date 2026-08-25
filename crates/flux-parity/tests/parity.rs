//! Parity acceptance tests over all ten Appendix B.3 examples.
//!
//! Each test compiles the example through the full pipeline, reduces the dev (AST)
//! and both release (Swift / Kotlin codegen) paths to the structural [`ViewNode`]
//! model, asserts equivalence, and snapshots the relation with `insta`.
//!
//! Examples the MLP lowerer does not yet support are reported as
//! `LowererUnsupported` rather than failing — the harness proves parity for every
//! example the pipeline can fully compile and degrades gracefully otherwise.

use flux_parity::{ParityReport, ParityStatus, all_examples, check_parity};

/// Asserts the documented parity contract for one example and snapshots the
/// report via `insta`.
///
/// Panics (via `insta::assert_snapshot!`) if the snapshot diverges from the
/// committed baseline; that is the intended failure mode for a parity regression.
/// A lowerer-unsupported example is asserted to be reported gracefully (not to
/// panic), since the release backends could not be exercised for it.
fn assert_parity(name: &str, source: &str, file_id: u32) {
    let report: ParityReport =
        check_parity(source, file_id).expect("example parses and type-checks");
    if std::env::var("PARITY_DEBUG").is_ok() {
        eprintln!(
            "=== MISMATCH {name} ===\nDEV={:#?}\nSW={:#?}\nKT={:#?}",
            report.dev, report.swift, report.kotlin
        );
    }
    match report.status {
        ParityStatus::Supported => assert!(
            report.is_equivalent(),
            "parity divergence for {name}: dev vs swift vs kotlin trees differ"
        ),
        ParityStatus::LowererUnsupported => {
            // The dev (AST) tree is still available; only the release backends
            // could not run. This is a documented MLP lowerer capability boundary.
            assert!(!report.dev.is_empty(), "dev AST tree must be present");
        }
    }
    let serialized = format!(
        "verdict: {}\nstatus: {:?}\n\ndev    == {:#?}\nswift  == {:#?}\nkotlin == {:#?}\n",
        report.verdict(),
        report.status,
        report.dev,
        report.swift,
        report.kotlin
    );
    insta::assert_snapshot!(format!("parity_{name}"), serialized);
}

#[test]
fn b31_simple_parity() {
    let (name, src) = all_examples()[0];
    assert_parity(name, src, 31);
}

#[test]
fn b32_generic_parity() {
    let (name, src) = all_examples()[1];
    assert_parity(name, src, 32);
}

#[test]
fn b33_adt_parity() {
    let (name, src) = all_examples()[2];
    assert_parity(name, src, 33);
}

#[test]
fn b34_lifecycle_parity() {
    let (name, src) = all_examples()[3];
    assert_parity(name, src, 34);
}

#[test]
fn b35_navigation_parity() {
    let (name, src) = all_examples()[4];
    assert_parity(name, src, 35);
}

#[test]
fn b36_async_parity() {
    let (name, src) = all_examples()[5];
    assert_parity(name, src, 36);
}

#[test]
fn b37_pure_parity() {
    let (name, src) = all_examples()[6];
    assert_parity(name, src, 37);
}

#[test]
fn b38_platform_parity() {
    let (name, src) = all_examples()[7];
    assert_parity(name, src, 38);
}

#[test]
fn b39_capability_parity() {
    let (name, src) = all_examples()[8];
    assert_parity(name, src, 39);
}

#[test]
fn b310_refs_parity() {
    let (name, src) = all_examples()[9];
    assert_parity(name, src, 310);
}
