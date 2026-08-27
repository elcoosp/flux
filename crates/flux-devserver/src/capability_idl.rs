//! The dev server's view of the Flux capability surface (spec §24, ADR-0045).
//!
//! The canonical capability definitions — [`CapabilityIdl`], [`MethodIdl`],
//! [`CAPABILITY_IDL`], and [`is_satisfied`] — live in `flux_types::capabilities`
//! so the compiler (`flux-ir` `CALL_CAP` emission) and the dev server share one
//! table with no opportunity to drift. This module re-exports that table under
//! the `crate::capability_idl` path the rest of the dev server already uses, and
//! adds the dev-server-only codegen helpers plus the parity guards. Every
//! capability's id, method ids and names are declared exactly once (in
//! `flux_types::capabilities`); the native runtimes derive their tables from it
//! through the `codegen_*` functions.
//!
//! The `capabilities.flux` stdlib declarations must mirror [`CAPABILITY_IDL`]'s
//! names; `tests/capability_codegen_parity` fails if a native registry drifts
//! from what this module generates.

pub(crate) use flux_types::capabilities::{CapabilityIdl, is_satisfied};

// Test-only codegen helpers and the parity guards reference the table by name;
// keep it available under `#[cfg(test)]` so the lib build (no tests) does not
// warn about an unused import. `MethodIdl` is referenced only inside
// `flux_types`, not from this crate, so it is not re-exported here.
#[cfg(test)]
pub(crate) use flux_types::capabilities::CAPABILITY_IDL;

/// The capability list a host advertises in its `Hello` handshake, derived from
/// [`CAPABILITY_IDL`] (spec §D.12.1 / §24.4). Every runtime builds its
/// advertisement from this so the wire names/ids stay identical to the dev
/// server's validation.
#[must_use]
#[cfg(test)]
pub(crate) fn hello_capabilities() -> Vec<(String, u32, Vec<String>)> {
    CAPABILITY_IDL
        .iter()
        .map(|cap| {
            (
                cap.name.to_owned(),
                cap.id,
                cap.methods.iter().map(|m| m.name.to_owned()).collect(),
            )
        })
        .collect()
}

/// Generates the capability metadata table for the iOS `Registry.swift`.
///
/// The emitted block is wrapped in `GENERATED-BEGIN`/`GENERATED-END` markers
/// and pasted into the native registry; `tests/capability_codegen_parity`
/// asserts the checked-in block equals this output so the two cannot drift.
#[must_use]
#[cfg(test)]
pub(crate) fn swift_idl_table() -> String {
    let mut out = String::from(
        "// ===== GENERATED-BEGIN (derived from flux-devserver capability_idl; do not edit) =====\n",
    );
    out.push_str(
        "private static let idlCapabilities: [(String, UInt32, [(String, UInt16)])] = [\n",
    );
    for cap in CAPABILITY_IDL {
        out.push_str(&format!("    (\"{}\", {}, [\n", cap.name, cap.id));
        for m in cap.methods {
            out.push_str(&format!("        (\"{}\", {}),\n", m.name, m.id));
        }
        out.push_str("    ]),\n");
    }
    out.push_str("]\n");
    out.push_str("// ===== GENERATED-END =====\n");
    out
}

/// Generates the capability metadata table for the Android
/// `CapabilityRegistry.kt` (see [`swift_idl_table`] for the parity contract).
#[must_use]
#[cfg(test)]
pub(crate) fn kotlin_idl_table() -> String {
    let mut out = String::from(
        "// ===== GENERATED-BEGIN (derived from flux-devserver capability_idl; do not edit) =====\n",
    );
    out.push_str("private val idlCapabilities: List<Triple<String, UInt, List<Pair<String, UInt>>>> = listOf(\n");
    for cap in CAPABILITY_IDL {
        out.push_str(&format!(
            "    Triple(\"{}\", {}u, listOf(\n",
            cap.name, cap.id
        ));
        for m in cap.methods {
            out.push_str(&format!("        \"{}\" to {}u,\n", m.name, m.id));
        }
        out.push_str("    )),\n");
    }
    out.push_str(")\n");
    out.push_str("// ===== GENERATED-END =====\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idl_ids_match_native_registries() {
        // The ids here must equal the native registry tables and
        // stdlib/capabilities.flux.
        assert_eq!(CapabilityIdl::names_for(1, 1), Some(("Camera", "take")));
        assert_eq!(CapabilityIdl::names_for(2, 2), Some(("Storage", "get")));
        assert_eq!(CapabilityIdl::names_for(3, 1), Some(("Router", "navigate")));
        assert_eq!(CapabilityIdl::names_for(9, 9), None);
        assert_eq!(CAPABILITY_IDL.len(), 3);
    }

    #[test]
    fn satisfaction_checks_name_and_feature() {
        let advertised = hello_capabilities();
        assert!(is_satisfied(&advertised, "Camera", "take"));
        assert!(is_satisfied(&advertised, "Storage", "get"));
        assert!(is_satisfied(&advertised, "Router", "navigate"));
        // stopPreview IS advertised, so this should be satisfied.
        assert!(is_satisfied(&advertised, "Camera", "stopPreview"));
        // An empty advertised set satisfies nothing.
        let empty: Vec<(String, u32, Vec<String>)> = Vec::new();
        assert!(!is_satisfied(&empty, "Camera", "take"));
    }

    #[test]
    fn generated_tables_round_trip_markers() {
        let swift = swift_idl_table();
        assert!(swift.contains("GENERATED-BEGIN"));
        assert!(swift.contains("(\"Camera\", 1, ["));
        assert!(swift.contains("(\"navigate\", 1)"));
        let kotlin = kotlin_idl_table();
        assert!(kotlin.contains("GENERATED-BEGIN"));
        assert!(kotlin.contains("Triple(\"Storage\", 2u"));
    }
}

/// Parity guard: the capability metadata tables checked into the native
/// runtimes must equal what the IDL generates. If a developer edits a table by
/// hand (or edits the IDL without regenerating), this fails in `cargo nextest`
/// — the single-source-of-truth contract (ADR-0045). The generated text is
/// embedded verbatim between `GENERATED-BEGIN`/`GENERATED-END` markers in each
/// native file.
#[cfg(test)]
mod parity {
    use super::*;

    fn repo_root() -> std::path::PathBuf {
        // crates/flux-devserver -> repo root is two levels up.
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    fn extract_generated(src: &str) -> Option<String> {
        let start = src.find("// ===== GENERATED-BEGIN")?;
        let end = src.find("// ===== GENERATED-END")?;
        let end_line = src[end..].find('\n').unwrap_or(0);
        Some(src[start..end + end_line].to_owned())
    }

    /// Normalises a generated block for comparison: trims leading/trailing
    /// whitespace on every line and drops blank lines. This tolerates a
    /// formatter's indentation choices (e.g. Swift auto-indenting inside an
    /// `extension`) while still catching any drift in the capability *content*
    /// — a renamed capability, changed id, or added method fails the guard.
    fn normalize(block: &str) -> String {
        block
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn swift_registry_matches_idl() {
        let path = repo_root().join("runtimes/ios/FluxHost/Sources/FluxHost/HelloFrame.swift");
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let generated = extract_generated(&src).expect("Swift GENERATED block present");
        assert_eq!(
            normalize(&generated),
            normalize(&swift_idl_table()),
            "iOS HelloFrame.swift capability table drifted from capability_idl"
        );
    }

    #[test]
    fn kotlin_registry_matches_idl() {
        let path = repo_root()
            .join("runtimes/android/host/src/main/kotlin/dev/flux/host/wire/HelloFrame.kt");
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let generated = extract_generated(&src).expect("Kotlin GENERATED block present");
        assert_eq!(
            normalize(&generated),
            normalize(&kotlin_idl_table()),
            "Android HelloFrame.kt capability table drifted from capability_idl"
        );
    }

    #[test]
    fn stdlib_capabilities_mirror_idl_names() {
        // stdlib/capabilities.flux must declare exactly the IDL's capability
        // names (the dev server resolves CALL_CAP ids against this manifest,
        // and the host advertises the same names).
        let path = repo_root().join("stdlib/capabilities.flux");
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for cap in CAPABILITY_IDL {
            assert!(
                src.contains(&format!("capability {}", cap.name)),
                "stdlib/capabilities.flux is missing `capability {}`",
                cap.name
            );
            for m in cap.methods {
                assert!(
                    src.contains(&format!("fn {}", m.name)),
                    "stdlib/capabilities.flux is missing `fn {}` on `{}`",
                    m.name,
                    cap.name
                );
            }
        }
    }
}
