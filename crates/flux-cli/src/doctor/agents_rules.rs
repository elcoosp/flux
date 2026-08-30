//! Local AGENTS.md rule checks (FLUX-091, AGENTS.md §1.2 / §2.1 / §2.2).
//!
//! Surfaces the same heuristics the FLUX-087 CI gate enforces, but runs locally
//! and faster (no CI runner, no tokei dependency). It is deliberately
//! best-effort — a line/brace heuristic, not a full parser — and only flags the
//! four things AGENTS.md calls out:
//!
//! * file longer than 300 lines (§1.2),
//! * function longer than 40 lines (§1.2),
//! * `unwrap` / `expect` / `panic!` in non-test Rust (§2.1),
//! * `try!` / force-unwrap (`as!`, `!!`) in non-test Swift/Kotlin (§2.2).
//!
//! Test code is excluded by path (`tests/`, `benches/`, `Tests/`, `*Test.*`)
//! and, for Rust, by tracking `mod tests` brace depth so in-test `unwrap`s in
//! the same file are not flagged.

use std::path::Path;

/// A single AGENTS.md rule violation found by the scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentsFinding {
    /// Path of the offending file, relative to the scan root.
    pub path: String,
    /// 1-based line number of the finding.
    pub line: u32,
    /// Human-readable description of the violation.
    pub message: String,
}

/// Scans `root` for AGENTS.md violations and returns every finding.
///
/// Hidden/build dirs (`target`, `Generated`, `.git`, `node_modules`) and the
/// `Cargo.lock` files are skipped so only first-party source is judged. The
/// order is deterministic (directory-walk order) so `flux doctor` output and
/// tests are stable.
#[must_use]
pub(crate) fn scan_sources(root: &Path) -> Vec<AgentsFinding> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if is_skipped_dir(name) {
                    continue;
                }
            }
            if path.is_dir() {
                stack.push(path);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if matches!(ext, "rs" | "swift" | "kt" | "kts") {
                    scan_file(&path, root, &mut out);
                }
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
    out
}

/// Runs the check against the repository by walking up to the workspace root.
#[must_use]
pub(crate) fn check_agents_rules() -> Vec<AgentsFinding> {
    let root = workspace_root().unwrap_or_else(|| Path::new(".").to_path_buf());
    scan_sources(&root)
}

/// Directory names that are never source we own.
fn is_skipped_dir(name: &str) -> bool {
    matches!(
        name,
        "target" | "Generated" | ".git" | "node_modules" | "build"
    )
}

/// Reads one file and records any AGENTS.md violations into `out`.
fn scan_file(path: &Path, root: &Path, out: &mut Vec<AgentsFinding>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned();
    let is_test_file = is_test_path(path);
    let lines: Vec<&str> = text.lines().collect();

    // Rule: file > 300 lines.
    if lines.len() > 300 {
        out.push(AgentsFinding {
            path: rel.clone(),
            line: 1,
            message: format!("file is {} lines (exceeds 300)", lines.len()),
        });
    }

    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => scan_rust(&rel, &lines, is_test_file, out),
        Some("swift") | Some("kt") | Some("kts") => {
            scan_swift_kotlin(&rel, &lines, is_test_file, out)
        }
        _ => {}
    }
}

/// Scans a Rust source file: function length + `unwrap`/`expect`/`panic!`.
fn scan_rust(rel: &str, lines: &[&str], is_test_file: bool, out: &mut Vec<AgentsFinding>) {
    let mut depth: usize = 0;
    let mut fn_start: Option<u32> = None;
    let mut in_test_mod: Option<usize> = None; // depth at which a `mod tests` opened

    for (i, raw) in lines.iter().enumerate() {
        let line_no = (i + 1) as u32;
        let line = raw.trim();

        // Track `mod tests` so in-test unwraps are not flagged (§1.2 allows them).
        if !is_test_file && in_test_mod.is_none() && line.starts_with("mod tests") {
            in_test_mod = Some(depth);
        }

        if is_fn_start(line) && depth == 0 {
            fn_start = Some(line_no);
        }

        for c in raw.chars() {
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth = depth.saturating_sub(1);
                if let Some(open) = in_test_mod {
                    if open == depth {
                        in_test_mod = None;
                    }
                }
                if fn_start.is_some() && depth == 0 {
                    let len = line_no - fn_start.unwrap() + 1;
                    if len > 40 {
                        out.push(AgentsFinding {
                            path: rel.to_owned(),
                            line: fn_start.unwrap(),
                            message: format!("function is {len} lines (exceeds 40)"),
                        });
                    }
                    fn_start = None;
                }
            }
        }

        if !is_test_file && in_test_mod.is_none() && contains_rust_panic_api(line) {
            out.push(AgentsFinding {
                path: rel.to_owned(),
                line: line_no,
                message: "unwrap/expect/panic! in non-test code".to_owned(),
            });
        }
    }
}

/// Scans a Swift/Kotlin source file: function length + `try!`/`as!`/`!!`.
fn scan_swift_kotlin(rel: &str, lines: &[&str], is_test_file: bool, out: &mut Vec<AgentsFinding>) {
    let mut depth: usize = 0;
    let mut fn_start: Option<u32> = None;
    for (i, raw) in lines.iter().enumerate() {
        let line_no = (i + 1) as u32;
        let line = raw.trim();
        if is_swift_kotlin_fn_start(line) && depth == 0 {
            fn_start = Some(line_no);
        }
        for c in raw.chars() {
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth = depth.saturating_sub(1);
                if fn_start.is_some() && depth == 0 {
                    let len = line_no - fn_start.unwrap() + 1;
                    if len > 40 {
                        out.push(AgentsFinding {
                            path: rel.to_owned(),
                            line: fn_start.unwrap(),
                            message: format!("function is {len} lines (exceeds 40)"),
                        });
                    }
                    fn_start = None;
                }
            }
        }
        if !is_test_file && contains_force_unwrap(line) {
            out.push(AgentsFinding {
                path: rel.to_owned(),
                line: line_no,
                message: "try!/force-unwrap (as!/!!) in non-test code".to_owned(),
            });
        }
    }
}

/// `true` for a top-level Rust function declaration line.
fn is_fn_start(line: &str) -> bool {
    let stripped = strip_visibility(line);
    stripped.starts_with("fn ")
        || stripped.starts_with("async fn ")
        || stripped.starts_with("const fn ")
        || stripped.starts_with("unsafe fn ")
}

/// Strips a leading `pub(...)` / `pub` / visibility qualifier for matching.
fn strip_visibility(line: &str) -> &str {
    let l = line.trim_start();
    let l = l
        .strip_prefix("pub(crate)")
        .or_else(|| l.strip_prefix("pub(super)"))
        .or_else(|| l.strip_prefix("pub"))
        .unwrap_or(l);
    l.trim_start()
}

/// `true` for a Swift `func` / Kotlin `fun` declaration line.
fn is_swift_kotlin_fn_start(line: &str) -> bool {
    line.starts_with("func ") || line.starts_with("fun ") || line.starts_with("override func ")
}

/// `true` when a Rust line contains a forbidden panic API in non-test code.
fn contains_rust_panic_api(line: &str) -> bool {
    line.contains(".unwrap()") || line.contains(".expect(") || line.contains("panic!(")
}

/// `true` when a Swift/Kotlin line contains `try!`, `as!`, or `!!`.
fn contains_force_unwrap(line: &str) -> bool {
    line.contains("try!") || line.contains("as!") || line.contains("!!")
}

/// `true` when the path is clearly a test target.
fn is_test_path(path: &Path) -> bool {
    let s = path.to_string_lossy();
    if s.contains("/tests/")
        || s.contains("/benches/")
        || s.contains("/Tests/")
        || s.contains("/test/")
    {
        return true;
    }
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name.ends_with("Tests.swift")
            || name.ends_with("Test.swift")
            || name.ends_with("Test.kt")
            || name.ends_with("Tests.kt")
            || name.ends_with("_test.rs")
            || name == "tests.rs"
            || name.starts_with("test_")
        {
            return true;
        }
    }
    false
}

/// Walks up from the current dir to find the workspace root (the dir holding
/// `AGENTS.md`). Returns `None` when not inside the repo.
fn workspace_root() -> Option<std::path::PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join("AGENTS.md").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
