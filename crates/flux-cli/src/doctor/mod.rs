//! `flux doctor` — environment health check (roadmap §5 / Phase 3).
//!
//! Mirrors `react-native doctor` / `flutter doctor`: one command that probes
//! the toolchain, the stdlib, the wire-protocol version, and (best-effort)
//! connected devices/simulators, and prints a clear pass/fail summary. Every
//! check is real — it shells out to the actual tool or runs the real parse
//! check — and reports `[ok]` / `[missing]` rather than guessing.
//!
//! Device/simulator detection is best-effort: it looks for the Android `adb`
//! device list and the iOS `xcrun simctl` list, but never fails the command if
//! those tools are absent (a missing simulator is not a broken install).
//!
//! Beyond the environment probes, `flux doctor` also runs two *local*
//! structural checks (FLUX-091):
//!
//! * [`check_dependency_drift`] — flags any dependency that has drifted off the
//!   approved set / `^MAJOR` version ceiling mandated by AGENTS.md §1.3.
//! * [`check_agents_rules`] — flags AGENTS.md §1.2 / §2.1 / §2.2 violations
//!   locally (file > 300 lines, function > 40 lines, `unwrap`/`expect`/`panic!`
//!   in non-test Rust, `try!`/force-unwrap in non-test Swift/Kotlin). This is
//!   the same heuristic the FLUX-087 CI gate runs, surfaced locally and faster.
//!
//! The command is advisory (non-fatal) by default; `--strict` makes any
//! advisory finding exit non-zero so it can gate a pre-commit hook.

mod agents_rules;
mod dependency_drift;
mod probe;

use anyhow::Result;

use probe::{probe_devices, probe_tool, stdlib_parse_check};

pub(crate) use agents_rules::AgentsFinding;
pub(crate) use agents_rules::check_agents_rules;
pub(crate) use dependency_drift::DependencyDrift;
pub(crate) use dependency_drift::check_dependency_drift;

/// One line of `flux doctor` output.
#[derive(Debug)]
pub(crate) struct Check {
    /// Human-readable label, e.g. "Xcode (xcodebuild)".
    label: String,
    /// `true` when the check passed.
    ok: bool,
    /// Detail shown after the status marker.
    detail: String,
}

impl Check {
    /// Builds a passing [`Check`].
    fn ok(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Check {
            label: label.into(),
            ok: true,
            detail: detail.into(),
        }
    }

    /// Builds a failing [`Check`].
    fn fail(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Check {
            label: label.into(),
            ok: false,
            detail: detail.into(),
        }
    }
}

/// Runs `flux doctor`.
///
/// When `strict` is `true`, any advisory finding (dependency drift or an
/// AGENTS.md rule violation) flips the process exit status to non-zero so the
/// command can gate a pre-commit hook. The environment probes are never fatal.
///
/// # Errors
///
/// Propagates a write failure to stdout; individual failed checks are reported
/// as `[missing]` lines, not as errors.
pub(crate) fn run(strict: bool) -> Result<()> {
    let mut checks: Vec<Check> = Vec::new();

    // --- Toolchain presence (each is a real `--version` probe). ---
    checks.push(probe_tool(
        "Rust toolchain (cargo)",
        "cargo",
        &["--version"],
    ));
    checks.push(probe_tool(
        "Swift toolchain (swift)",
        "swift",
        &["--version"],
    ));
    checks.push(probe_tool(
        "Xcode (xcodebuild)",
        "xcodebuild",
        &["-version"],
    ));
    checks.push(probe_tool("Android (gradle)", "gradle", &["--version"]));
    checks.push(probe_tool("Android (kotlinc)", "kotlinc", &["-version"]));
    checks.push(probe_tool("Node (website/tooling)", "node", &["--version"]));

    // --- Wire protocol version (single source of truth). ---
    checks.push(Check::ok(
        "Wire protocol version",
        format!("PROTOCOL_VERSION = {}", flux_devserver::PROTOCOL_VERSION),
    ));

    // --- Stdlib parse-check (runs the real parser over every stdlib module). ---
    checks.push(stdlib_parse_check());

    // --- Best-effort device / simulator detection. ---
    checks.push(probe_devices());

    // --- Local structural checks (FLUX-091). ---
    let report = check_dependency_drift();
    let drifts = report.drifts;
    if let Some(note) = &report.note {
        // The live gather couldn't read the workspace — surface it as an
        // info line so `flux doctor` stays advisory (never a hard failure).
        println!("note: dependency drift check skipped — {note}");
    }
    let agents_findings = check_agents_rules();

    let advisory = advisory_findings(&drifts, &agents_findings);

    // The two structural checks read as their own summary lines so the count is
    // visible at a glance; individual findings are listed below.
    checks.push(Check::ok(
        "Dependency drift (AGENTS.md §1.3)",
        if drifts.is_empty() {
            "all resolved deps within approved set / ^MAJOR".to_owned()
        } else {
            format!("{} drifted (see below)", drifts.len())
        },
    ));
    checks.push(Check::ok(
        "AGENTS.md rules (§1.2/§2.1/§2.2)",
        if agents_findings.is_empty() {
            "no violations found".to_owned()
        } else {
            format!("{} violation(s) (see below)", agents_findings.len())
        },
    ));

    let env_passed = checks.iter().filter(|c| c.ok).count();
    let env_total = checks.len();
    for c in &checks {
        let mark = if c.ok { "ok" } else { "missing" };
        println!("[{}] {} — {}", mark, c.label, c.detail);
    }

    if !drifts.is_empty() || !agents_findings.is_empty() {
        println!();
        if !drifts.is_empty() {
            println!("dependency drift:");
            for d in &drifts {
                println!(
                    "  - {} {} resolved {} — {} (approved {})",
                    d.name, d.approved, d.resolved, d.warning, d.approved
                );
            }
        }
        if !agents_findings.is_empty() {
            println!("AGENTS.md violations:");
            for f in &agents_findings {
                println!("  - {}:{} — {}", f.path, f.line, f.message);
            }
        }
    }

    println!();
    println!(
        "flux doctor: {}/{} environment checks passed",
        env_passed, env_total
    );
    if advisory.is_empty() {
        println!("no dependency drift or AGENTS.md violations detected");
    } else {
        println!(
            "{} advisory finding(s) — run with --strict to gate pre-commit",
            advisory.len()
        );
    }

    if should_bail(strict, &advisory) {
        anyhow::bail!(
            "{} advisory finding(s) under --strict; see above",
            advisory.len()
        );
    }
    Ok(())
}

/// Builds the human-readable advisory list from the drift + rule findings.
fn advisory_findings(drifts: &[DependencyDrift], agents_findings: &[AgentsFinding]) -> Vec<String> {
    let mut advisory: Vec<String> = Vec::new();
    for d in drifts {
        advisory.push(format!(
            "dependency drift: {} {} resolved {} (approved {})",
            d.name, d.resolved, d.warning, d.approved
        ));
    }
    for f in agents_findings {
        advisory.push(format!(
            "AGENTS.md violation: {}:{} — {}",
            f.path, f.line, f.message
        ));
    }
    advisory
}

/// Returns `true` when `flux doctor` must exit non-zero: any advisory finding
/// exists and `--strict` was requested.
fn should_bail(strict: bool, advisory: &[String]) -> bool {
    strict && !advisory.is_empty()
}

#[cfg(test)]
mod mod_tests;
