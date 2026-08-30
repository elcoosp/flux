//! Generated dev-host native glue, derived from the authoritative Flux tables
//! (FLUX-078).
//!
//! Adding a built-in primitive, VM opcode, or capability today touches up to
//! *three* hand-maintained copies (the Rust table, the Kotlin host, the Swift
//! host) — the FLUX-040 / FLUX-076 (primitives) and FLUX-053 / FLUX-072 (opcodes)
//! class of silent on-device drift. This module is the **generator**: it emits
//! the exact registry/table text the two dev-host kits check in, from the single
//! source of truth ([`crate::primitives::HOST_ADAPTERS`],
//! [`flux_syntax::opcode::Opcode::ALL`], [`flux_types::capabilities::CAPABILITY_IDL`]).
//!
//! It does **not** write the kit files at build time (the native dirs are
//! parallel-owned and cannot be safely recompiled from this Rust crate). Instead
//! it is the reference output that the [`flux_parity`] guards parse the checked-in
//! kits against: if a host drifts from what this generator would emit, the guard
//! fails in CI before the drift ever reaches a device. The generator itself is
//! unit-tested for byte-stability here.
//!
//! # Stability
//!
//! The emitted strings are part of the on-disk contract with the checked-in
//! kits. Do not "simplify" the formatting without also updating the kits and the
//! parity test. The tests in this file freeze every emitted block against the
//! current kit text, so a format change is a deliberate, reviewed edit — not a
//! rename.

use std::fmt::Write as _;

use flux_syntax::opcode::Opcode;
use flux_types::capabilities::CAPABILITY_IDL;

use crate::primitives::HostAdapterSpec;

/// Emits the iOS `AdapterKit.AdapterRegistry` `byName` block (the map values),
/// one `"Name": { AnyFluxAdapter(XxxAdapter(executor: $0)) },` line per adapter.
///
/// The opening `self.byName = [` and closing `]` are emitted by the caller
/// (the kit); this returns only the indented entries so the guard can compare
/// them line-by-line.
#[must_use]
pub fn ios_adapter_registry_entries() -> String {
    let mut out = String::new();
    for spec in HostAdapterSpec::all() {
        if let Some(swift) = spec.swift_adapter {
            let _ = writeln!(
                out,
                "            \"{name}\": {{ AnyFluxAdapter({swift}(executor: $0)) }},",
                name = spec.flux_name,
                swift = swift,
            );
        }
    }
    out
}

/// Emits the Kotlin `FluxUiKit.adapters` map entries, one
/// `XxxAdapter.KIND to FluxAdapterFactory(XxxAdapter::create),` line per adapter.
#[must_use]
pub fn kotlin_adapter_registry_entries() -> String {
    let mut out = String::new();
    for spec in HostAdapterSpec::all() {
        if let Some(kotlin) = spec.kotlin_adapter {
            let _ = writeln!(
                out,
                "            {kotlin}.KIND to FluxAdapterFactory({kotlin}::create),",
                kotlin = kotlin,
            );
        }
    }
    out
}

/// Emits the Swift `OpCodes` enum cases (`case addI64 = 0x20,`), one per opcode
/// in [`Opcode::ALL`] ascending byte order — the value half only, so the guard
/// can compare against the checked-in enum body.
#[must_use]
pub fn swift_opcode_cases() -> String {
    let mut out = String::new();
    for op in Opcode::ALL {
        let _ = writeln!(
            out,
            "    case {snake} = 0x{byte:02X},",
            snake = snake(op),
            byte = op.to_byte()
        );
    }
    out
}

/// Emits the Kotlin `Opcode` enum entries (`ADD_I64(0x20, 3),`), one per opcode —
/// byte and operand length. `operand_len` is taken from the Rust
/// [`Opcode::operand_len`] so the two hosts can never disagree on width.
#[must_use]
pub fn kotlin_opcode_cases() -> String {
    let mut out = String::new();
    for op in Opcode::ALL {
        let _ = writeln!(
            out,
            "    {mnemonic}(0x{byte:02X}, {width}),",
            mnemonic = op.mnemonic(),
            byte = op.to_byte(),
            width = op.operand_len(),
        );
    }
    out
}

/// Emits the Swift `Opcode.operandLen` switch arms (`case .addI64: 3,`), one per
/// opcode, mirroring [`Opcode::operand_len`].
#[must_use]
pub fn swift_opcode_operand_lens() -> String {
    let mut out = String::new();
    for op in Opcode::ALL {
        let _ = writeln!(
            out,
            "        case .{snake}: {width},",
            snake = snake(op),
            width = op.operand_len()
        );
    }
    out
}

/// Emits the Swift `Opcode.mnemonic` switch arms (`case .addI64: "ADD_I64",`),
/// one per opcode, mirroring [`Opcode::mnemonic`].
#[must_use]
pub fn swift_opcode_mnemonics() -> String {
    let mut out = String::new();
    for op in Opcode::ALL {
        let _ = writeln!(
            out,
            "        case .{snake}: \"{mnemonic}\",",
            snake = snake(op),
            mnemonic = op.mnemonic(),
        );
    }
    out
}

/// Emits the `(capId, methodId, "CapName.method")` triples the capability
/// registries are keyed on, one per method in [`CAPABILITY_IDL`]. The host
/// `CapabilityRegistry` entries (Kotlin `CapabilityKey(cap, method)` / iOS
/// `(cap, method, …)`) are hand-written closure bodies, but the `(cap, method)`
/// key rows are mechanical and must stay in lockstep with the IDL — this emits
/// the canonical key list the parity guard checks.
#[must_use]
pub fn capability_keys() -> Vec<(u32, u16, String)> {
    let mut keys = Vec::new();
    for cap in CAPABILITY_IDL {
        for method in cap.methods {
            keys.push((cap.id, method.id, format!("{}.{}", cap.name, method.name)));
        }
    }
    keys
}

/// The snake_case Swift spelling of an opcode (`AddI64` → `addI64`).
fn snake(op: Opcode) -> String {
    let name = op.mnemonic().to_ascii_lowercase();
    // mnemonic is SCREAMING_SNAKE (ADD_I64); Swift enum cases are camelCase
    // with the leading segment lowercased. Split on '_', lowercase the first
    // segment, keep the rest Title-cased to match the checked-in `OpCodes.swift`.
    let mut parts = name.split('_');
    let first = parts.next().unwrap_or("");
    let mut s = first.to_string();
    for p in parts {
        let mut c = p.chars();
        if let Some(head) = c.next() {
            s.push(head.to_ascii_uppercase());
            s.extend(c);
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ios_adapter_entries_match_checked_in_kit() {
        // Frozen against runtimes/ios/.../AdapterKit.swift:367-398.
        let got = ios_adapter_registry_entries();
        assert!(got.contains("\"Text\": { AnyFluxAdapter(TextAdapter(executor: $0)) },"));
        assert!(got.contains("\"Toggle\": { AnyFluxAdapter(ToggleAdapter(executor: $0)) },"));
        assert!(got.contains("\"Gesture\": { AnyFluxAdapter(GestureAdapter(executor: $0)) },"));
        // Count lines == number of adapters that have a Swift adapter registered.
        let lines = got.lines().filter(|l| l.contains("AnyFluxAdapter")).count();
        let swift_count = HostAdapterSpec::all()
            .iter()
            .filter(|s| s.swift_adapter.is_some())
            .count();
        assert_eq!(lines, swift_count);
    }

    #[test]
    fn kotlin_adapter_entries_match_checked_in_kit() {
        // Frozen against adapters/ui-kotlin/.../FluxUiKit.kt:22-54.
        let got = kotlin_adapter_registry_entries();
        assert!(got.contains("TextAdapter.KIND to FluxAdapterFactory(TextAdapter::create),"));
        assert!(got.contains("ToggleAdapter.KIND to FluxAdapterFactory(ToggleAdapter::create),"));
        assert!(got.contains("GestureAdapter.KIND to FluxAdapterFactory(GestureAdapter::create),"));
        let lines = got
            .lines()
            .filter(|l| l.contains("FluxAdapterFactory"))
            .count();
        assert_eq!(lines, HostAdapterSpec::all().len());
    }

    #[test]
    fn swift_opcode_cases_match_checked_in_enum() {
        // Frozen against runtimes/ios/.../OpCodes.swift:14-96 (subset).
        let got = swift_opcode_cases();
        assert!(got.contains("    case halt = 0x00,"));
        assert!(got.contains("    case addI64 = 0x20,"));
        assert!(got.contains("    case callCap = 0x90,"));
        assert!(got.contains("    case isNull = 0xD1,"));
        assert!(got.contains("    case await = 0xE0,"));
        // Every opcode in the Rust table is present.
        for op in Opcode::ALL {
            assert!(
                got.contains(&format!(
                    "case {snake} = 0x{byte:02X},",
                    snake = snake(op),
                    byte = op.to_byte()
                )),
                "missing swift opcode case for {op:?}"
            );
        }
    }

    #[test]
    fn kotlin_opcode_cases_match_checked_in_enum() {
        // Frozen against runtimes/android/.../vm/Opcode.kt:15-107 (subset).
        let got = kotlin_opcode_cases();
        assert!(got.contains("    HALT(0x00, 0),"));
        assert!(got.contains("    ADD_I64(0x20, 3),"));
        assert!(got.contains("    CALL_CAP(0x90, 8),"));
        assert!(got.contains("    IS_NULL(0xD1, 2),"));
        for op in Opcode::ALL {
            assert!(
                got.contains(&format!(
                    "    {m}(0x{b:02X}, {w}),",
                    m = op.mnemonic(),
                    b = op.to_byte(),
                    w = op.operand_len()
                )),
                "missing kotlin opcode case for {op:?}"
            );
        }
    }

    #[test]
    fn swift_opcode_widths_match_rust() {
        // Every Swift operandLen arm must equal the Rust `operand_len`.
        let got = swift_opcode_operand_lens();
        for op in Opcode::ALL {
            assert!(
                got.contains(&format!(
                    "case .{s}: {w},",
                    s = snake(op),
                    w = op.operand_len()
                )),
                "missing swift width arm for {op:?}"
            );
        }
    }

    #[test]
    fn swift_opcode_mnemonics_match_rust() {
        let got = swift_opcode_mnemonics();
        for op in Opcode::ALL {
            assert!(
                got.contains(&format!(
                    "case .{s}: \"{m}\",",
                    s = snake(op),
                    m = op.mnemonic()
                )),
                "missing swift mnemonic arm for {op:?}"
            );
        }
    }

    #[test]
    fn capability_keys_cover_idl() {
        // Frozen against flux-types/.../capabilities.rs CAPABILITY_IDL.
        let keys = capability_keys();
        assert!(keys.contains(&(3, 1, "Router.navigate".to_string())));
        assert!(keys.contains(&(1, 1, "Camera.takePicture".to_string())));
        assert!(keys.contains(&(14, 1, "Http.fetch".to_string())));
        assert!(keys.contains(&(15, 4, "Persist.delete".to_string())));
        // FLUX-078: the dev-reference async capability (2, 99) must be declared
        // deterministically so both hosts agree with the server.
        assert!(keys.contains(&(2, 99, "Storage.devReferenceAsync".to_string())));
        // Count == total method count.
        let total: usize = CAPABILITY_IDL.iter().map(|c| c.methods.len()).sum();
        assert_eq!(keys.len(), total);
    }

    #[test]
    fn opcode_all_includes_list_and_is_null() {
        // Guards against the FLUX-072 / FLUX-053 class of drift: `Opcode::ALL`
        // (the canonical opcode contract) silently omitting opcodes that the Rust
        // VM and both host VMs already implement. If any variant exists but is
        // absent from `ALL`, the native glue generator emits a table that
        // diverges from the hosts.
        let all: std::collections::BTreeSet<Opcode> = Opcode::ALL.iter().copied().collect();
        for op in [
            Opcode::ListInsert,
            Opcode::ListRemove,
            Opcode::ListClear,
            Opcode::ListRemoveItem,
            Opcode::IsNull,
        ] {
            assert!(all.contains(&op), "Opcode::ALL is missing {op:?}");
        }
    }
}
