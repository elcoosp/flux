# ADR-0049: Cross-host naming convergence (Android ↔ iOS runtime)

- **Status:** Accepted
- **Date:** 2026-08-28

## Context

`runtimes/android/host` (FLUX-007) and `runtimes/ios/FluxHost` (FLUX-006) are
both behavioral mirrors of the Rust reference VM `flux-vm-ref` (FLUX-005). Over
time the two hosts drifted in the *names* of identical types — `OpCode` vs
`Opcode`, `VMValue` vs `FluxValue`, `FluxFrame` vs `Frame`, `StringResolvable`
vs `StringResolver`, `FluxRuntime` vs `FluxExecutor`, etc. — while the wire
bytes, opcodes and observable behavior stayed identical. The drift is purely
cosmetic but it makes parity edits (e.g. the router fix) require constant mental
translation and invites genuine divergence. See the naming-drift report that
prompted this ADR.

## Decision

For every concept implemented on both hosts, adopt a **single canonical name**.
Where the Rust oracle already names the concept, the host name follows the
oracle. Otherwise the name already used by the larger/older surface wins.
Casing follows each language's idiomatic convention (Rust `PascalCase` enum
cases; Swift `lowerCamel` member names but `PascalCase` types; Kotlin
`SCREAMING_SNAKE` enum entries), but the **identifier spelling** is unified.

### Canonical names (the contract)

For the **type identifier** (struct/enum/class/protocol), both hosts use PascalCase
and the name is unified. For **enum cases / members**, each language keeps its
own idiomatic casing (Rust/Kotlin `SCREAMING_SNAKE` or `PascalCase`; Swift
`lowerCamelCase`). So `CellState.ready` / `RunResult.halt` on iOS and
`CellState.Ready` / `RunResult.Halt` on Android are both correct and are NOT
renamed — only the enclosing type name is unified.

| Concept | Canonical type identifier | Notes |
|---|---|---|
| Decoded value (wire + VM) | `FluxValue` | Android host already `FluxValue`; iOS `VMValue` → `FluxValue`. The adapter-kit value type is also `FluxValue` on both kits — the host and kit types are distinct modules so no collision. |
| Opcode enum | `Opcode` | Android host already `Opcode`; iOS `OpCode` → `Opcode`. Member `opcode` (iOS `opCode` → `opcode`). Cases stay lowerCamel on iOS (`halt`) / SCREAMING_SNAKE on Android (`HALT`). |
| VM fault | `VmError` | Android already `VmError`; iOS `VMError` → `VmError`. Kind enum `VmErrorKind` (Android) / `VmError` enum (iOS) — both renamed `VmError`, see below. |
| Decoded frame | `FluxFrame` | iOS already `FluxFrame`; Android wire `Frame` → `FluxFrame`. |
| String resolver protocol | `StringResolver` | Android already `StringResolver`; iOS `StringResolvable` → `StringResolver`. |
| Host executor/coordinator | `FluxExecutor` | Android host already `FluxExecutor`; iOS type `FluxRuntime` → `FluxExecutor`. (The iOS file stays `FluxExecutor.swift`.) |
| CellState cases | `Ready` / `Pending` / `Error` (Android, PascalCase) vs `ready` / `pending` / `error` (iOS, lowerCamel) | **Not renamed** — each language's idiomatic member casing is preserved. |
| RunResult cases | `Halt` / `Suspended` (Android, PascalCase) vs `halt` / `suspended` (iOS, lowerCamel) | **Not renamed** — idiomatic member casing preserved. |
| SuspendState resume addressing | byte offset `resumeOffset` | The oracle uses byte offset `resume_ip`. Android `resumeIndex` (instruction index) is a divergent *representation*; this ADR unifies the **name** but the byte-vs-index representation is tracked separately (see Open Questions). |

### Not renamed (intentional)

- Wire-only intermediates that exist on only one host (`WireNode`, `WireValue`,
  `WireChild` on Android) — they have no iOS counterpart, so there is nothing to
  converge against. iOS decodes straight into `ShadowNode`.
- `Patch` representation (tagged enum on iOS vs data class + tag byte on Android)
  and `ShadowNode`+`BuiltNode` split (iOS) vs merged `ShadowNode` (Android) —
  these are genuine structural differences, out of scope for a naming-only ADR.
- `ReactiveDispatcher` (Android, an injectable type) vs `@MainActor` (iOS) — the
  abstraction differs by language; the concept is named in docs as the R-graph.

## Consequences

- Parity edits no longer need name translation between the two hosts for these
  types.
- The iOS host now also carries a separate `VmError` *enum* (the kind) and
  `VmError` *struct* (offset + span) to match Android's two-type split, instead
  of a single offset-carrying enum. The wire behavior is unchanged.
- Builds on both platforms must stay green; no behavioral change.

## Open Questions

1. `SuspendState.resumeOffset` (byte offset, iOS/oracle) vs `resumeIndex`
   (instruction index, Android) — the **name** is now `resumeOffset` on both?
   No: this ADR unifies identifiers only where the representation is already the
   same. The index/offset gap is a real semantic difference and is left to a
   follow-up (LANE-A async resolver already reads `resumeOffset` on iOS; Android
   uses `resumeIndex`). Keep the existing names until that gap is closed.
