//! Unit tests for the `flux doctor` orchestrator (`doctor/mod.rs`).

use super::*;

#[test]
fn should_bail_only_with_strict_and_findings() {
    // No findings → never bail, even under --strict.
    assert!(!should_bail(true, &[]));
    // Findings but not strict → stay advisory (non-fatal).
    assert!(!should_bail(false, &["a violation".to_owned()]));
    // Findings + strict → gate the pre-commit hook.
    assert!(should_bail(true, &["a violation".to_owned()]));
}

#[test]
fn advisory_findings_formats_drift_and_rules() {
    let drifts = vec![DependencyDrift {
        name: "tokio".to_owned(),
        approved: "^1".to_owned(),
        resolved: "2.0.1".to_owned(),
        warning: "resolved major 2 exceeds approved ^MAJOR 1".to_owned(),
    }];
    let findings = vec![AgentsFinding {
        path: "lib.rs".to_owned(),
        line: 10,
        message: "unwrap in non-test code".to_owned(),
    }];
    let advisory = advisory_findings(&drifts, &findings);
    assert_eq!(advisory.len(), 2);
    assert!(advisory[0].contains("tokio"));
    assert!(advisory[0].contains("2.0.1"));
    assert!(advisory[1].contains("lib.rs:10"));
    assert!(advisory[1].contains("unwrap"));
}
