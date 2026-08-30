//! Approved-dependency version-drift check (FLUX-091, AGENTS.md §1.3).
//!
//! AGENTS.md §1.3 mandates "latest stable, pinned to `^MAJOR`" against an
//! explicit approved list. This module flags any dependency that has drifted
//! off that ceiling — i.e. a resolved version whose major is higher than the
//! `^MAJOR` floor declared in the workspace manifest. A major-version jump is
//! exactly the kind of change that should go through an ADR + manifest steward,
//! not silently creep in via a lockfile update.
//!
//! The pure comparison logic ([`find_drift`]) takes the *declared* requirements
//! and the *resolved* versions explicitly so it can be unit-tested without
//! shelling out to `cargo` or touching the network. The live gather path
//! ([`DependencyReport::gather`]) reads both from `cargo metadata`, which is
//! fast and offline (it reuses `Cargo.lock`).

use serde::Deserialize;

/// A single dependency that has drifted past its approved `^MAJOR` ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DependencyDrift {
    /// Crate name as it appears in the manifest (e.g. `"tokio"`).
    pub name: String,
    /// The approved ceiling pulled from the workspace manifest (e.g. `"^1"`).
    pub approved: String,
    /// The version actually resolved in the lockfile (e.g. `"2.0.1"`).
    pub resolved: String,
    /// Human-readable explanation shown under `flux doctor`.
    pub warning: String,
}

/// A dependency as declared in a manifest, before resolution.
#[derive(Debug, Clone)]
pub(crate) struct DeclaredDep {
    /// Crate name.
    pub name: String,
    /// The semver requirement string (e.g. `"^1"`, `">=1.2"`, `"1"`).
    pub req: String,
}

/// The result of running the drift check: the findings plus whether the live
/// gather succeeded (so callers can distinguish "clean" from "couldn't check").
#[derive(Debug, Default)]
pub(crate) struct DependencyReport {
    /// Every dependency that drifted past its `^MAJOR` ceiling.
    pub drifts: Vec<DependencyDrift>,
    /// `Some(msg)` when the live gather could not run (e.g. not in a workspace);
    /// `None` when the data was read successfully.
    pub note: Option<String>,
}

impl DependencyReport {
    /// Runs the check against the real workspace via `cargo metadata`.
    ///
    /// Network-free: `cargo metadata` resolves from `Cargo.lock` when present.
    /// On any failure (not a workspace, `cargo` absent) it returns an empty
    /// report with a [`DependencyReport::note`] so `flux doctor` stays advisory.
    #[must_use]
    pub(crate) fn gather() -> DependencyReport {
        let meta = match std::process::Command::new("cargo")
            .args(["metadata", "--format-version=1"])
            .output()
        {
            Ok(out) if out.status.success() => out,
            Ok(out) => {
                return DependencyReport {
                    note: Some(format!(
                        "cargo metadata exited {} (run from a workspace)",
                        out.status
                    )),
                    ..Default::default()
                };
            }
            Err(e) => {
                return DependencyReport {
                    note: Some(format!("cargo metadata unavailable: {e}")),
                    ..Default::default()
                };
            }
        };
        let Ok(text) = String::from_utf8(meta.stdout) else {
            return DependencyReport {
                note: Some("cargo metadata produced non-UTF8 output".to_owned()),
                ..Default::default()
            };
        };
        let parsed: CargoMetadata = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                return DependencyReport {
                    note: Some(format!("cargo metadata parse error: {e}")),
                    ..Default::default()
                };
            }
        };

        // Build a name -> resolved version map from the resolve graph.
        let mut resolved: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for pkg in &parsed.packages {
            resolved.insert(pkg.name.clone(), pkg.version.clone());
        }

        // Collect declared deps from workspace-member packages only.
        let members: std::collections::HashSet<&str> = parsed
            .workspace_members
            .iter()
            .map(String::as_str)
            .collect();
        let mut declared: Vec<DeclaredDep> = Vec::new();
        for pkg in &parsed.packages {
            if !members.contains(pkg.id.as_str()) {
                continue;
            }
            for dep in &pkg.dependencies {
                // Skip path/workspace-internal deps (they have no `^MAJOR`
                // ceiling; they are first-party crates, not external approvals).
                if dep.source.as_deref().is_some_and(|s| s.contains("path+")) {
                    continue;
                }
                declared.push(DeclaredDep {
                    name: dep.name.clone(),
                    req: dep.req.clone(),
                });
            }
        }

        DependencyReport {
            drifts: find_drift(&declared, &resolved),
            note: None,
        }
    }
}

/// Runs the dependency-drift check against the real workspace.
///
/// Convenience wrapper around [`DependencyReport::gather`] so callers (and the
/// `flux doctor` orchestrator) get a flat `DependencyReport` without touching
/// the struct's inherent API.
#[must_use]
pub(crate) fn check_dependency_drift() -> DependencyReport {
    DependencyReport::gather()
}

/// Compares declared `^MAJOR` ceilings against resolved versions and returns
/// every drift.
///
/// A drift is recorded when a dependency's resolved major version is strictly
/// greater than the major floor of its declared requirement. Pre-release
/// suffixes are ignored for the comparison (e.g. `2.0.0-rc.1` is major 2).
#[must_use]
pub(crate) fn find_drift(
    declared: &[DeclaredDep],
    resolved: &std::collections::HashMap<String, String>,
) -> Vec<DependencyDrift> {
    let mut out = Vec::new();
    for d in declared {
        let Some(ver) = resolved.get(&d.name) else {
            // Declared but absent from the resolve graph — cannot verify; skip
            // rather than false-positive (a path dep may resolve under another
            // id).
            continue;
        };
        let Some(ceiling_major) = req_major(&d.req) else {
            continue;
        };
        let Some(resolved_major) = version_major(ver) else {
            continue;
        };
        if resolved_major > ceiling_major {
            out.push(DependencyDrift {
                name: d.name.clone(),
                approved: d.req.clone(),
                resolved: ver.clone(),
                warning: format!(
                    "resolved major {} exceeds approved ^MAJOR {}",
                    resolved_major, ceiling_major
                ),
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Extracts the major version floor from a semver requirement string.
///
/// Handles the common forms used in this workspace: `^1`, `>=1.2`, `=1.2.3`,
/// `~1.0`, and a bare `1`. Returns `None` when no leading numeric major is
/// present (e.g. a git/`*` requirement, which has no `^MAJOR` ceiling to
/// enforce here).
fn req_major(req: &str) -> Option<u64> {
    let digits: String = req
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Extracts the major version from a concrete version string, ignoring any
/// pre-release / build suffix after the first `-` or `+`.
fn version_major(version: &str) -> Option<u64> {
    let core = version.split(['-', '+']).next().unwrap_or(version);
    core.split('.').next().and_then(|m| m.parse::<u64>().ok())
}

/// `cargo metadata --format-version=1` — only the fields we read.
#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
    id: String,
    #[serde(default)]
    dependencies: Vec<CargoDep>,
}

#[derive(Debug, Deserialize)]
struct CargoDep {
    name: String,
    req: String,
    #[serde(default)]
    source: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
        pairs
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn drift_flags_major_bump() {
        let declared = vec![DeclaredDep {
            name: "tokio".to_owned(),
            req: "^1".to_owned(),
        }];
        let map = resolved(&[("tokio", "2.0.1")]);
        let drifts = find_drift(&declared, &map);
        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].name, "tokio");
        assert_eq!(drifts[0].resolved, "2.0.1");
        assert_eq!(drifts[0].approved, "^1");
    }

    #[test]
    fn no_drift_within_major() {
        let declared = vec![DeclaredDep {
            name: "serde".to_owned(),
            req: "^1".to_owned(),
        }];
        let map = resolved(&[("serde", "1.0.219")]);
        assert!(find_drift(&declared, &map).is_empty());
    }

    #[test]
    fn prerelease_does_not_mask_drift() {
        let declared = vec![DeclaredDep {
            name: "tokio".to_owned(),
            req: "^1".to_owned(),
        }];
        let map = resolved(&[("tokio", "2.0.0-rc.1")]);
        assert_eq!(find_drift(&declared, &map).len(), 1);
    }

    #[test]
    fn req_major_parses_common_forms() {
        assert_eq!(req_major("^1"), Some(1));
        assert_eq!(req_major(">=1.2"), Some(1));
        assert_eq!(req_major("=1.2.3"), Some(1));
        assert_eq!(req_major("~1.0"), Some(1));
        assert_eq!(req_major("1"), Some(1));
        assert_eq!(req_major("*"), None);
    }

    #[test]
    fn version_major_strips_suffix() {
        assert_eq!(version_major("1.2.3"), Some(1));
        assert_eq!(version_major("2.0.0-rc.1"), Some(2));
        assert_eq!(version_major("3.1.0+build.5"), Some(3));
    }
}
