//! FLUX-F39 — pins the current fact that the async bridge's `resume` half
//! has no production caller. When a capability-completion driver lands
//! (see the issue), this test should be replaced with an end-to-end test
//! that verifies the frame flow through the driver.
//!
//! Why a source-text scan rather than a compile-time check: Rust has no
//! language-level way to assert "this function is never called outside
//! tests". A `#[doc(hidden)]` marker or a `#[deprecated]` would not catch
//! the pattern (both are already public API). A source scan is the only
//! honest way to pin the current state until the driver exists.

#[test]
#[ignore = "FLUX-F39: AsyncBridge::resume has no production caller; parked handlers deadlock"]
fn async_bridge_resume_has_no_production_caller() {
    // Verify the current state: nothing under crates/flux-devserver/src calls
    // `AsyncBridge::resume` (other than tests). If a driver lands, this
    // assertion flips and the test can be re-written.
    let src_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders: Vec<String> = Vec::new();
    walk_rs(&src_root, &mut offenders);
    assert!(
        offenders.is_empty(),
        "AsyncBridge::resume is now called from production code: {offenders:?}. \
         Replace this test with an end-to-end capability-completion test."
    );
}

fn walk_rs(dir: &std::path::Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("read src dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rs(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(text) = std::fs::read_to_string(&path) {
                // Ignore hits inside `#[cfg(test)] mod tests { … }` by a
                // simple "past the first #[cfg(test)]" heuristic.
                let test_start = text.find("#[cfg(test)]").unwrap_or(text.len());
                let production = &text[..test_start];
                if production.contains(".resume(") {
                    out.push(path.display().to_string());
                }
            }
        }
    }
}
