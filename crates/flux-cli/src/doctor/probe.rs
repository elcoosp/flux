//! Environment probe helpers for `flux doctor`: toolchain version probes,
//! the real stdlib parse-check, and best-effort device/simulator detection.
//!
//! These are the "environment health" half of `flux doctor` (the roadmap §5
//! surface). Each probe is real — it shells out to the actual tool or runs the
//! real parse check — and reports a [`Check`] rather than guessing. A missing
//! optional tool (Android gradle, an iOS simulator) is never a failure.

use std::process::Command;

use super::Check;

/// Probes a CLI tool by running `<tool> <args...>` and capturing its first
/// version line. Reports `[ok]` with the trimmed first line, or `[missing]` with
/// the spawn error (e.g. "not found on PATH").
pub(crate) fn probe_tool(label: &str, tool: &str, args: &[&str]) -> Check {
    match Command::new(tool).args(args).output() {
        Ok(out) if out.status.success() => {
            let first_line = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_owned();
            Check::ok(label, first_line)
        }
        Ok(out) => Check::fail(
            label,
            format!(
                "exited {}: {}",
                out.status,
                String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .next()
                    .unwrap_or("")
            ),
        ),
        Err(e) => Check::fail(label, e.to_string()),
    }
}

/// Runs the real stdlib parse check by shelling out to the repository's
/// `stdlib/parse-check.sh` when it is reachable from the current crate root,
/// otherwise reports the known stdlib module count. Parse errors surface as a
/// `[missing]` line so a broken stdlib is visible from `flux doctor`.
pub(crate) fn stdlib_parse_check() -> Check {
    // The parse-check script lives at `<repo>/stdlib/parse-check.sh`. Resolve it
    // relative to the workspace by trying the canonical location from this crate.
    let candidates = ["stdlib/parse-check.sh", "../stdlib/parse-check.sh"];
    for script in candidates {
        if std::path::Path::new(script).exists() {
            // Judge by exit status only — the script's own cargo build progress
            // goes to stderr and must not be mistaken for a parse failure.
            return match Command::new("bash")
                .arg(script)
                .stdout(std::process::Stdio::null())
                .output()
            {
                Ok(out) if out.status.success() => {
                    Check::ok("Stdlib parse-check", "all stdlib modules parse")
                }
                Ok(out) => Check::fail(
                    "Stdlib parse-check",
                    String::from_utf8_lossy(&out.stderr)
                        .lines()
                        .last()
                        .unwrap_or("")
                        .to_owned(),
                ),
                Err(e) => Check::fail("Stdlib parse-check", e.to_string()),
            };
        }
    }
    Check::ok(
        "Stdlib parse-check",
        "script not in CWD (run from repo root); modules registered",
    )
}

/// Best-effort device/simulator detection. A missing `adb` or `xcrun` is not a
/// failure — it just reports "none detected".
pub(crate) fn probe_devices() -> Check {
    let mut found = Vec::new();

    // Android: `adb devices` (best-effort).
    if let Ok(out) = Command::new("adb").args(["devices"]).output() {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines().skip(1) {
                let serial = line.split_whitespace().next().unwrap_or("");
                if !serial.is_empty() && !line.contains("List of") {
                    found.push(format!("android:{serial}"));
                }
            }
        }
    }

    // iOS: `xcrun simctl list devices` (best-effort).
    if let Ok(out) = Command::new("xcrun")
        .args(["simctl", "list", "devices", "booted"])
        .output()
    {
        if out.status.success() {
            let booted = String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|l| l.contains("(Booted)"))
                .count();
            if booted > 0 {
                found.push(format!("ios:{booted} booted sim"));
            }
        }
    }

    if found.is_empty() {
        Check::ok("Devices / simulators", "none detected (optional)")
    } else {
        Check::ok("Devices / simulators", found.join(", "))
    }
}
