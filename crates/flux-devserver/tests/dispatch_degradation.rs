//! FLUX-F40 / F41 — pins the current behavior of the dispatch-report path.
//!
//! * F40: an active index whose dirty node id is not in the arena currently
//!   returns `None` (no Error frame); the doc claims callers degrade, but
//!   nothing tells the host why the dispatch had no effect.
//! * F41: in production, `set_signal_deps` is never called, so the index is
//!   always inactive and every report degrades silently. This test documents
//!   the "no production caller" fact via a source scan.

#[test]
#[ignore = "FLUX-F41: set_signal_deps only called from tests; production reports are no-ops"]
fn set_signal_deps_has_no_production_caller() {
    let src_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders: Vec<String> = Vec::new();
    walk_rs(&src_root, &mut offenders);
    assert!(
        offenders.is_empty(),
        "set_signal_deps is now called from production code: {offenders:?}. \
         Replace this test with an end-to-end test that drives a real dispatch \
         report through a live index."
    );
}

fn walk_rs(dir: &std::path::Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("read src dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rs(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(text) = std::fs::read_to_string(&path) {
                let test_start = text.find("#[cfg(test)]").unwrap_or(text.len());
                let production = &text[..test_start];
                // Match a CALL, not the public wrapper's declaration.
                if production.contains(".set_signal_deps(") {
                    out.push(path.display().to_string());
                }
            }
        }
    }
}
