//! Native-kit vs generated-glue parity guards (FLUX-078).
//!
//! The dev-host adapter kits (`adapters/ui-kotlin`, `adapters/ui-swift`) and the
//! host VM opcode tables (`runtimes/android/host/.../vm/Opcode.kt`,
//! `runtimes/ios/.../OpCodes.swift`) are hand-maintained copies of the
//! authoritative Rust tables. They have drifted before and shipped silent
//! on-device faults — FLUX-040/FLUX-076 (primitives missing from one host) and
//! FLUX-053/FLUX-072 (opcodes added to `flux-vm-ref` but never ported to a host
//! VM, so they hit an unknown-opcode branch on device).
//!
//! `flux-codegen-core::native_gen` emits, from the single source of truth, the
//! exact registry/table text those kits check in. These tests parse the checked-
//! in native files and assert they contain **every** generated entry and **no**
//! adapter registered under a name the generator does not know about. A future
//! drift therefore fails here, in CI, before it reaches a device.
//!
//! The native files are read with `include_str!` so the guard runs with no
//! network or toolchain (the Kotlin/Swift compilers are not needed to assert the
//! *table content*).

use std::collections::BTreeSet;

use flux_codegen_core::native_gen;

const IOS_ADAPTER_KIT: &str =
    include_str!("../../../runtimes/ios/FluxHost/Sources/FluxHost/AdapterKit.swift");
const KOTLIN_FLUX_UI_KIT: &str =
    include_str!("../../../adapters/ui-kotlin/src/main/kotlin/dev/flux/ui/FluxUiKit.kt");
const IOS_OPCODES: &str =
    include_str!("../../../runtimes/ios/FluxHost/Sources/FluxHost/OpCodes.swift");
const KOTLIN_OPCODES: &str =
    include_str!("../../../runtimes/android/host/src/main/kotlin/dev/flux/host/vm/Opcode.kt");
const IOS_CAPS: &str =
    include_str!("../../../runtimes/ios/FluxHost/Sources/FluxHost/Registry.swift");
const KOTLIN_CAPS: &str = include_str!(
    "../../../runtimes/android/host/src/main/kotlin/dev/flux/host/vm/CapabilityRegistry.kt"
);

/// Extracts the set of `"Name"` keys from the iOS `AdapterRegistry.byName` map.
fn ios_adapter_names(kit: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in kit.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix('"') {
            if let Some((name, _)) = rest.split_once('"') {
                // Only lines shaped `"Name": { AnyFluxAdapter(...) }`.
                if rest.contains("AnyFluxAdapter") {
                    names.insert(name.to_string());
                }
            }
        }
    }
    names
}

/// Extracts the set of `XxxAdapter.KIND` adapter class names from the Kotlin
/// `FluxUiKit.adapters` map.
fn kotlin_adapter_classes(kit: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in kit.lines() {
        // Each entry is `XxxAdapter.KIND to FluxAdapterFactory(XxxAdapter::create),`.
        // Match on `.KIND to FluxAdapterFactory(` so the captured prefix is the
        // full class name (`XxxAdapter`), not a substring of it.
        if let Some(idx) = line.find(".KIND to FluxAdapterFactory(") {
            let class = line[..idx].trim();
            if !class.is_empty() {
                names.insert(class.to_string());
            }
        }
    }
    names
}

/// Extracts the set of Swift opcode enum cases (`case fooBar = 0xNN,`).
fn swift_opcode_cases(src: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for line in src.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("case ") {
            if let Some((name, _)) = rest.split_once(" = ") {
                set.insert(name.trim_end_matches(',').to_string());
            }
        }
    }
    set
}

/// Extracts the set of Kotlin opcode enum entries (`FOO(0xNN, W),`).
fn kotlin_opcode_entries(src: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for line in src.lines() {
        let line = line.trim();
        if let Some((name, _)) = line.split_once('(') {
            // Mnemonics are SCREAMING_SNAKE; the enum variant is exactly that.
            if name.chars().all(|c| c.is_ascii_uppercase() || c == '_') && !name.is_empty() {
                set.insert(name.to_string());
            }
        }
    }
    set
}

/// Extracts the `(cap, method)` key pairs registered in the Kotlin
/// `CapabilityRegistry.makeDev` (`CapabilityKey(cap, method) to …`).
fn kotlin_capability_keys(src: &str) -> BTreeSet<(u32, u16)> {
    let mut set = BTreeSet::new();
    for line in src.lines() {
        if let Some(rest) = line.find("CapabilityKey(") {
            let tail = &line[rest + "CapabilityKey(".len()..];
            if let Some((body, _)) = tail.split_once(')') {
                // Each operand is a numeric literal that may carry a `u` suffix
                // (`1u`) or a `toUShort()` call (`1u.toUShort()`); keep only the
                // leading integer.
                let nums: Vec<&str> = body.split(',').map(str::trim).collect();
                if nums.len() >= 2 {
                    let cap = nums[0]
                        .trim_end_matches('u')
                        .split('.')
                        .next()
                        .unwrap_or(nums[0])
                        .parse::<u32>();
                    let method = nums[1]
                        .trim_end_matches('u')
                        .split('.')
                        .next()
                        .unwrap_or(nums[1])
                        .parse::<u16>();
                    if let (Ok(c), Ok(m)) = (cap, method) {
                        set.insert((c, m));
                    }
                }
            }
        }
    }
    set
}

/// Extracts the `(cap, method, …)` key triples registered in the iOS
/// `CapabilityRegistry.makeDev` (`(1, 1, { … })`).
fn ios_capability_keys(src: &str) -> BTreeSet<(u32, u16)> {
    let mut set = BTreeSet::new();
    for line in src.lines() {
        let line = line.trim();
        if line.starts_with('(') {
            if let Some(stripped) = line.strip_prefix('(') {
                if let Some((cap_part, rest)) = stripped.split_once(',') {
                    if let Some(method_part) = rest.trim_start().split(',').next() {
                        if let (Ok(c), Ok(m)) = (
                            cap_part.trim().parse::<u32>(),
                            method_part.trim().parse::<u16>(),
                        ) {
                            set.insert((c, m));
                        }
                    }
                }
            }
        }
    }
    set
}

#[test]
fn ios_adapter_kit_matches_generated() {
    let generated: BTreeSet<String> = flux_codegen_core::primitives::HostAdapterSpec::all()
        .iter()
        .filter(|s| s.swift_adapter.is_some())
        .map(|s| s.flux_name.to_string())
        .collect();
    let checked_in = ios_adapter_names(IOS_ADAPTER_KIT);
    assert_eq!(
        checked_in,
        generated,
        "iOS AdapterRegistry drifted from generated set\nmissing in kit: {:?}\nextra in kit: {:?}",
        generated.difference(&checked_in).collect::<Vec<_>>(),
        checked_in.difference(&generated).collect::<Vec<_>>(),
    );
}

#[test]
fn kotlin_adapter_kit_matches_generated() {
    let generated: BTreeSet<String> = flux_codegen_core::primitives::HostAdapterSpec::all()
        .iter()
        .filter_map(|s| s.kotlin_adapter)
        .map(str::to_string)
        .collect();
    let checked_in = kotlin_adapter_classes(KOTLIN_FLUX_UI_KIT);
    assert_eq!(
        checked_in,
        generated,
        "Kotlin FluxUiKit.adapters drifted from generated set\nmissing in kit: {:?}\nextra in kit: {:?}",
        generated.difference(&checked_in).collect::<Vec<_>>(),
        checked_in.difference(&generated).collect::<Vec<_>>(),
    );
}

#[test]
fn ios_opcodes_match_generated() {
    let generated = swift_opcode_cases(&native_gen::swift_opcode_cases());
    let checked_in = swift_opcode_cases(IOS_OPCODES);
    assert_eq!(
        checked_in,
        generated,
        "iOS OpCodes.swift drifted from flux-syntax::opcode::Opcode::ALL\nmissing in kit: {:?}\nextra in kit: {:?}",
        generated.difference(&checked_in).collect::<Vec<_>>(),
        checked_in.difference(&generated).collect::<Vec<_>>(),
    );
}

#[test]
fn kotlin_opcodes_match_generated() {
    let generated = kotlin_opcode_entries(&native_gen::kotlin_opcode_cases());
    let checked_in = kotlin_opcode_entries(KOTLIN_OPCODES);
    assert_eq!(
        checked_in,
        generated,
        "Android Opcode.kt drifted from flux-syntax::opcode::Opcode::ALL\nmissing in kit: {:?}\nextra in kit: {:?}",
        generated.difference(&checked_in).collect::<Vec<_>>(),
        checked_in.difference(&generated).collect::<Vec<_>>(),
    );
}

#[test]
fn kotlin_capabilities_match_idl() {
    let idl: BTreeSet<(u32, u16)> = native_gen::capability_keys()
        .into_iter()
        .map(|(c, m, _)| (c, m))
        .collect();
    let checked_in = kotlin_capability_keys(KOTLIN_CAPS);
    // Subset direction: every capability the Kotlin kit registers must exist in
    // the IDL. Missing-from-kit keys are NOT asserted here (adding a capability
    // to the IDL is a deliberate ADR-gated change; the kit catches up later) —
    // but a kit key absent from the IDL is a real error (unknown capability on
    // device) and must fail CI.
    let extra: Vec<_> = checked_in.difference(&idl).collect();
    assert!(
        extra.is_empty(),
        "Android CapabilityRegistry registers capabilities not in CAPABILITY_IDL: {:?}",
        extra,
    );
}

#[test]
fn ios_capabilities_match_idl() {
    let idl: BTreeSet<(u32, u16)> = native_gen::capability_keys()
        .into_iter()
        .map(|(c, m, _)| (c, m))
        .collect();
    let checked_in = ios_capability_keys(IOS_CAPS);
    // Subset direction (see `kotlin_capabilities_match_idl`): a kit key absent
    // from the IDL is a real on-device error. NOTE: the iOS kit currently
    // registers only 13 of the 30 IDL capabilities (the remaining 17 are a real
    // drift gap, tracked separately) — this guard does NOT assert they are
    // present, only that what IS registered is known.
    let extra: Vec<_> = checked_in.difference(&idl).collect();
    assert!(
        extra.is_empty(),
        "iOS CapabilityRegistry registers capabilities not in CAPABILITY_IDL: {:?}",
        extra,
    );
}
