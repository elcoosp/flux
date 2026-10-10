//! Unit tests for the AGENTS.md rule scanner (`doctor/agents_rules.rs`).

use super::*;

use std::io::Write;

fn write_file(dir: &Path, rel: &str, body: &str) -> String {
    let full = dir.join(rel);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let mut f = std::fs::File::create(&full).unwrap();
    f.write_all(body.as_bytes()).unwrap();
    full.to_string_lossy().into_owned()
}

#[test]
fn flags_oversized_file() {
    let tmp = std::env::temp_dir().join(format!("agents-{}-big", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let body: String = "fn a() {}\n".repeat(301);
    write_file(&tmp, "big.rs", &body);
    let findings = scan_sources(&tmp);
    let file_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.path == "big.rs" && f.message.contains("exceeds 300"))
        .collect();
    assert_eq!(file_findings.len(), 1, "oversized file must be flagged");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn flags_long_function_and_unwrap() {
    let tmp = std::env::temp_dir().join(format!("agents-{}-fn", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    // 45-line function, one with an unwrap inside.
    let mut body = String::from("pub fn do_work() {\n");
    for i in 0..44 {
        body.push_str(&format!("    let _x{i} = {i};\n"));
    }
    body.push_str("    let v = compute().unwrap();\n");
    body.push_str("    let _force = guard!!;\n");
    body.push_str("}\n");
    write_file(&tmp, "lib.rs", &body);
    let findings = scan_sources(&tmp);
    assert!(
        findings
            .iter()
            .any(|f| f.path == "lib.rs" && f.message.contains("exceeds 40")),
        "long fn must be flagged: {findings:?}"
    );
    assert!(
        findings
            .iter()
            .any(|f| f.path == "lib.rs" && f.message.contains("unwrap")),
        "unwrap must be flagged: {findings:?}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_unwrap_not_flagged() {
    // An unwrap inside `mod tests` must NOT be reported.
    let tmp = std::env::temp_dir().join(format!("agents-{}-testmod", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    write_file(
        &tmp,
        "lib.rs",
        "fn real() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {\n        let v = something().unwrap();\n    }\n}\n",
    );
    let findings = scan_sources(&tmp);
    assert!(
        !findings.iter().any(|f| f.message.contains("unwrap")),
        "in-test unwrap must be skipped: {findings:?}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn flags_swift_force_unwrap_in_non_test() {
    let tmp = std::env::temp_dir().join(format!("agents-{}-swift", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    write_file(
        &tmp,
        "View.swift",
        "func render() {\n    let v = fetch() as! View\n}\n",
    );
    let findings = scan_sources(&tmp);
    assert!(
        findings
            .iter()
            .any(|f| f.path == "View.swift" && f.message.contains("force-unwrap")),
        "force-unwrap must be flagged: {findings:?}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn swift_test_file_unwrap_skipped() {
    let tmp = std::env::temp_dir().join(format!("agents-{}-swifttest", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    write_file(
        &tmp,
        "ViewTests.swift",
        "func testX() {\n    let v = fetch()!\n}\n",
    );
    let findings = scan_sources(&tmp);
    assert!(
        !findings.iter().any(|f| f.message.contains("force-unwrap")),
        "test-file force-unwrap must be skipped: {findings:?}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

// ── F22 regression tests ────────────────────────────────────────────────

/// A comment mentioning `.expect()` / `.unwrap()` must not be flagged.
#[test]
fn comment_mentioning_expect_is_not_flagged() {
    let line = "    // Audit note: callers still `.expect()` at the frame boundary.";
    assert!(
        !contains_rust_panic_api(line),
        "a comment mentioning .expect() must not be flagged"
    );
    let line = "// The `.unwrap()` here is only prose.";
    assert!(!contains_rust_panic_api(line));
}

/// A string literal containing `.unwrap()` must not be flagged.
#[test]
fn string_literal_containing_unwrap_is_not_flagged() {
    let line = r#"    line.contains(".unwrap()") || line.contains(".expect(")"#;
    assert!(
        !contains_rust_panic_api(line),
        "a string literal mentioning .unwrap() must not be flagged"
    );
}

/// Real panic APIs still flag — exactly the three the CI gate checks
/// (`unwrap` / `expect` / `panic!`), no more.
#[test]
fn real_panic_api_is_flagged() {
    assert!(contains_rust_panic_api("let x = foo().unwrap();"));
    assert!(contains_rust_panic_api("let x = foo().expect(\"msg\");"));
    assert!(contains_rust_panic_api("panic!(\"boom\");"));
}

/// Swift/Kotlin `try!` / `as!` in a comment or string literal is not flagged.
#[test]
fn swift_kotlin_comment_and_string_false_positives_are_filtered() {
    assert!(!contains_force_unwrap("// was `try!` before"));
    assert!(!contains_force_unwrap(r#"let s = "try! is bad""#));
    assert!(contains_force_unwrap("let x = try! foo()"));
    assert!(contains_force_unwrap("let x = y as! Int"));
    assert!(contains_force_unwrap("val v = x!!"));
}
