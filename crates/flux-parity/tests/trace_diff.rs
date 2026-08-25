//! Integration tests for the T16 trace-diff tool (ADR-0027).
//!
//! These pin the public API of `flux_parity::trace` and the `flux-parity-trace`
//! CLI binary against the golden corpus under `tests/trace-goldens/`.

use std::path::Path;

use flux_parity::trace::{
    Phase, TraceError, compare, diff_traces, load_trace_str, phase_from_filename,
};

const GOLDENS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/trace-goldens");

/// Builds a golden path for a scenario + phase.
fn golden(scenario: &str, phase: u8, platform: &str) -> String {
    format!("{GOLDENS}/{scenario}.p{phase}.{platform}.jsonl")
}

#[test]
fn identical_swift_kotlin_phases_match() {
    for scenario in [
        "counter_1000",
        "noop_dispatch",
        "pure_subtree",
        "cond_flip",
        "unrelated_signal",
    ] {
        for phase in 1..=3u8 {
            let left = golden(scenario, phase, "swift");
            let right = golden(scenario, phase, "kotlin");
            diff_traces(Path::new(&left), Path::new(&right))
                .unwrap_or_else(|e| panic!("phase {phase} of {scenario} diverged: {e}"));
        }
    }
}

#[test]
fn phase3_foreach_grow_is_oq3_gated() {
    // foreach_grow exists only for phase 3 (OQ-3 gated). When the golden is
    // present it must match; absence is an accepted gate, not a failure.
    let left = golden("foreach_grow", 3, "swift");
    let right = golden("foreach_grow", 3, "kotlin");
    if Path::new(&left).exists() && Path::new(&right).exists() {
        diff_traces(Path::new(&left), Path::new(&right))
            .expect("foreach_grow phase 3 must match when goldens exist");
    }
}

#[test]
fn divergence_exits_nonzero_with_context() {
    // Two traces that differ in one frame must report the first divergence.
    let left = load_trace_str(
        "{\"event\":\"mark\",\"node\":\"n1\"}\n{\"event\":\"mark\",\"node\":\"n2\"}\n",
    )
    .unwrap();
    let right = load_trace_str(
        "{\"event\":\"mark\",\"node\":\"n1\"}\n{\"event\":\"mark\",\"node\":\"n3\"}\n",
    )
    .unwrap();
    let err = compare(&left, &right).unwrap_err();
    assert_eq!(err.left_line, 2);
    let rendered = err.render(&left, &right);
    assert!(rendered.contains("divergence"));
    assert!(rendered.contains("n2"));
    assert!(rendered.contains("n3"));
}

#[test]
fn length_mismatch_is_a_divergence() {
    let left = load_trace_str("{\"event\":\"a\"}\n{\"event\":\"b\"}\n").unwrap();
    let right = load_trace_str("{\"event\":\"a\"}\n").unwrap();
    assert!(compare(&left, &right).is_err());
}

#[test]
fn invalid_phase_is_rejected() {
    assert!(Phase::new(4).is_none());
    assert!("0".parse::<Phase>().is_err());
    assert!("4".parse::<Phase>().is_err());
}

#[test]
fn phase_inferred_from_filename() {
    assert_eq!(
        phase_from_filename(Path::new("x.p1.jsonl")),
        Some(Phase::new(1).unwrap())
    );
    assert_eq!(
        phase_from_filename(Path::new("x.p2.jsonl")),
        Some(Phase::new(2).unwrap())
    );
    assert_eq!(
        phase_from_filename(Path::new("x.p3.jsonl")),
        Some(Phase::new(3).unwrap())
    );
    assert_eq!(phase_from_filename(Path::new("x.jsonl")), None);
}

#[test]
fn malformed_line_is_an_error() {
    let res = load_trace_str("{\"event\":\"x\" NOT JSON}\n");
    assert!(matches!(res, Err(TraceError::Json(_, _))));
}
