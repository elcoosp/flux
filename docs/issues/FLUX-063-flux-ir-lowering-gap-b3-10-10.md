---
id: FLUX-063
status: done
lane: LANE-F
phase: "Phase 0/1"
blocked_by: []
labels:
  - lowering
  - parity
  - bug
source: CHANGELOG.md §FLUX-011 (PARTIAL: 6/10 B.3 examples fail at flux-ir lowering: "unsupported handler operand/expression")
related_adrs:
  - ADR-0047
---

# FLUX-063: Close the flux-ir handler-lowering gap (B.3 parity 10/10)

- **Lane:** LANE-F (Phase 0/1 — blocking for parity)
- **Depends on:** none
- **Source:** `CHANGELOG.md` §FLUX-011 (PARTIAL — blocker noted "out of scope" for
  codegen; the gap lives in `crates/flux-ir/src/lower/bytecode.rs`)
- **Related ADRs:** ADR-0047

## Problem Statement

FLUX-011 fixed the codegen bridge but 6/10 B.3 examples (b32,b33,b35,b36,b38,b310)
still fail the parity gate **at the `flux-ir` lowering step**:
`unsupported handler operand: Call {...}` for `Numeric.one()`/`Rectangle(...)`;
`unsupported handler expression` for `router.navigate(...)`, `refetch()`,
`Auth.login(...)`, `...` spreads. These live in `crates/flux-ir/src/lower/bytecode.rs`.
The gate cannot reach 10/10 from the codegen side; the flux-ir handler-lower gap
must be landed first.

## Solution

Extend `flux-ir/src/lower/bytecode.rs` to lower: method/constructor calls as handler
operands (`Numeric.one()`, `Rectangle(...)`), capability calls as handler
expressions (`router.navigate`, `Auth.login`), and `...` spreads. Pin each of the 6
failing B.3 examples with a `flux-parity` trace test until 10/10 is green.

## Implementation Decisions

- This is a **lowering** fix, not a codegen fix — do NOT re-touch the codegen bridge
  (FLUX-011 already closed that).
- Reuse the reference VM (`flux-vm-ref`) as the oracle for the lowered bytecode.

## Testing Decisions

- `flux-parity` B3.1–B3.10 all green; each of the 6 named examples has a trace test
  asserting dev==Swift==Kotlin.

## Out of Scope

- The codegen bridge (FLUX-011, done). The async host halves (FLUX-064).
