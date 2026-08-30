# ADR-0058 — Codegen-driven native glue + parity guards (FLUX-078)

- **Status:** Accepted
- **Date:** 2026-08-30
- **Scope:** dev-host adapter kits, host VM opcode tables, capability registries
- **Supersedes:** — (complements ADR-0047 codegen core, ADR-0045 capability bridge)

## Context

Every built-in primitive, VM opcode, and capability today touches up to **three**
hand-maintained copies:

1. the Rust table of truth (`flux-codegen-core::primitives::PRIMITIVES`,
   `flux-syntax::opcode::Opcode::ALL`, `flux-types::capabilities::CAPABILITY_IDL`),
2. the Kotlin dev-host kit (`adapters/ui-kotlin`, `runtimes/android/host/vm`),
3. the Swift dev-host kit (`adapters/ui-swift`, `runtimes/ios/FluxHost`).

The release codegen path (ADR-0047) is already single-source — `PRIMITIVES`
drives both backends. But the **dev-host glue** (adapter registries, opcode
tables, capability registries) is still hand-ported per platform. That hand
porting has shipped silent on-device faults:

- **FLUX-040 / FLUX-076** — a primitive added to `PRIMITIVES` but never given a
  Swift adapter, or vice-versa.
- **FLUX-053** — `IS_NULL` added to `flux-vm-ref` but never ported to a host VM,
  so `?.` short-circuiting hit an unknown-opcode branch on device.
- **FLUX-072** — the `LIST_INSERT`/`LIST_REMOVE`/`LIST_CLEAR`/`LIST_REMOVE_ITEM`
  opcodes implemented in `flux-vm-ref` and ported to the hosts, but
  `flux-syntax::opcode::Opcode::ALL` (the canonical opcode contract) was never
  extended to include them — so the authoritative list drifted from the hosts.

The `time-to-feature` cost of this is the dominant hot path: adding a primitive
is one table row in Rust plus ~1,900 lines of mechanical Kotlin/Swift adapter
duplication (FLUX-040 = three commits, 126 + 1,123 + 773 LOC).

## Decision

Introduce a **generator + parity-guard pair** so the dev-host glue is derived
from the authoritative Rust tables and divergence fails CI *before* it reaches a
device:

### 1. `flux_codegen_core::native_gen` (generator, pure Rust)

Emits the exact registry/table text the two dev-host kits check in:

- `host_adapter_registry_entries` — the `AdapterKit.byName` / `FluxUiKit.adapters`
  entries, driven by a new `HostAdapterSpec` table (per-platform `Option<&str>`
  adapter class names, because e.g. `Container` is Kotlin-only).
- `kotlin_opcode_cases` / `swift_opcode_cases` — the `Opcode.kt` / `OpCodes.swift`
  enum entries, driven by `Opcode::ALL`.
- `capability_keys` — the `(cap, method, name)` triples, driven by
  `CAPABILITY_IDL`.

The generator does **not** write the kit files at build time: the native dirs
are parallel-owned and cannot be safely recompiled from this crate. It is the
reference output that the parity guard compares against.

### 2. `flux-parity/tests/native_kit_parity.rs` (parity guard)

Parses the checked-in native files with `include_str!` (no toolchain needed) and
asserts:

- **adapters** — `kit == generated` per platform (every registered adapter name
  matches `HostAdapterSpec`; no unknown/extra adapters).
- **opcodes** — `kit == generated` against `Opcode::ALL` (equality both
  directions, so a missing *or* extra opcode fails).
- **capabilities** — `kit ⊆ idl` (every capability a host registers must exist
  in `CAPABILITY_IDL`; a host key absent from the IDL is a real on-device error
  and fails). Subset (not equality) is used because adding a capability to the IDL
  is a deliberate ADR-gated change that the host kits catch up to later; the
  guard still catches the dangerous direction (unknown key on device).

### 3. Authoritative-table fixes surfaced by the guards (this change)

The guards were RED on first run and revealed three genuine drift bugs, now
fixed at the source:

- `flux-syntax::opcode::Opcode::ALL` extended to include the 4 `LIST_*` opcodes
  (it was stale — `57 → 61`); `flux-vm-ref` already handled them.
- `runtimes/ios/.../OpCodes.swift` gained the `isNull` (`0xD1`) case (mnemonic
  + width) — the FLUX-053 iOS half that was never ported.
- `flux-types::capabilities::CAPABILITY_IDL` gained the deterministic
  `(2, 99)` `Storage.devReferenceAsync` method — the iOS host had been
  hand-assigning this id, violating the ADR-0045 deterministic-id rule.

## Consequences

- Adding a primitive/opcode/capability now requires **one** edit to the
  authoritative Rust table; the parity guard proves the hosts match. The
  mechanical Kotlin/Swift adapter *bodies* are still hand-written (they are
  genuinely platform-specific), but the registry/table *wiring* is derived and
  checked.
- Future drift (the FLUX-053/072/040 class) fails in `cargo nextest` instead of
  silently on device.
- Build-time code emission into the native dirs remains out of scope (parallel
  ownership + unverifiable native compilation); the generator is the contract and
  the guard is the enforcement.

## Alternatives considered

- **Emit the kit files directly from `build.rs`.** Rejected: the native dirs are
  the highest-collision zone (parallel agents), cannot be recompiled here, and
  writing them risks stomping in-flight work. The generator-as-contract +
  parity-guard model delivers the same drift protection without the collision
  risk.
- **Subset-only opcode guard.** Rejected: subset would not catch a host *missing*
  an opcode (the FLUX-053/072 direction). Equality is achievable because the
  generator owns the full `Opcode::ALL`.
