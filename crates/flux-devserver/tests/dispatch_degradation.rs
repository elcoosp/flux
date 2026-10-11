//! FLUX-F40 / F41 — pins the current behavior of the dispatch-report path.
//!
//! F41: `set_signal_deps` is only called from tests; the internal
//! pass-through inside `RunningServer::set_signal_deps` forwards to
//! `Pipeline::set_signal_deps`, but nothing in production calls the
//! wrapper either. This test scans `src/` for an *external* call — a
//! `.set_signal_deps(` whose receiver is not `self.shared.pipeline` (the
//! internal forwarder) — and asserts there is none today.

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
                // Skip the body of `#[cfg(test)] mod tests`.
                let test_start = text.find("#[cfg(test)]").unwrap_or(text.len());
                let production = &text[..test_start];
                for (lineno, line) in production.lines().enumerate() {
                    // An external call is `.set_signal_deps(` on a receiver
                    // that is NOT the internal `self.shared.pipeline.lock()`
                    // forwarder inside the `RunningServer::set_signal_deps`
                    // wrapper itself.
                    let trimmed = line.trim();
                    if trimmed.starts_with("//") {
                        continue;
                    }
                    if !line.contains(".set_signal_deps(") {
                        continue;
                    }
                    // The pass-through line is:
                    //   self.shared.pipeline.lock().set_signal_deps(deps);
                    if line.contains("self.shared.pipeline") {
                        continue;
                    }
                    // The function definition itself:
                    //   pub fn set_signal_deps(&self, deps: Option<...>)
                    if line.contains("fn set_signal_deps") {
                        continue;
                    }
                    out.push(format!("{}:{}: {}", path.display(), lineno + 1, trimmed));
                }
            }
        }
    }
}
