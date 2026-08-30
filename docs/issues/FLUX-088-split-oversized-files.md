---
id: FLUX-088
status: todo
lane: LANE-SPLIT
phase: "Phase 2"
blocked_by:
  - FLUX-087
labels:
  - rust
  - android
  - ios
  - refactor
source: FLUX_PRODUCTION_READINESS_PLAN.md §1.4 + §2.2 + §2.3 (split every file >300 lines into single-responsibility modules, starting with the two VMs and the wire codec — highest bug density).
related_adrs: []
---

# FLUX-088: Split oversized files (>300 lines) into single-responsibility modules

- **Lane:** LANE-SPLIT (Phase 2 — structural, ongoing file-by-file)
- **Owner:** Rust / Android / iOS (per-file owners)
- **Source:** plan §1.4 (top structural item) + §2.2 (split `FluxBytecodeVM.swift`/
  `checker.rs` per opcode-family) + §2.3 (wire codec).
- **Disjoint from:** every other issue except it depends on FLUX-087's allowlist
  being seeded first (so CI stays green while splits land).

## Problem Statement

11 files in the dump exceed the 300-line cap and concentrate the highest bug
density:

| Lines | File | Priority |
|---|---|---|
| 2233 | `crates/flux-parser/src/parser.rs` | medium |
| 1957 | `crates/flux-ir/src/lower/bytecode.rs` | medium |
| 1774 | `crates/flux-types/src/checker.rs` | high (VM-adjacent) |
| 1584 | `runtimes/ios/.../FluxBytecodeVM.swift` | high (VM) |
| 1391 | `crates/flux-ir-serde/src/telemetry.rs` | medium |
| 1319 | `crates/flux-devserver/src/pipeline.rs` | medium |
| 1306 | `crates/flux-ir-serde/src/frame.rs` | high (wire codec) |
| 1179 | `runtimes/android/.../shadow/ShadowTree.kt` | medium |
| 1090 | `crates/flux-ir/src/arena.rs` | medium |
| 1072 | `crates/flux-ir-serde/src/wire.rs` | high (wire codec) |
| 985 | `runtimes/ios/.../ShadowTreeReconciler.swift` | medium |

A monolith interpreter/codec is where the next parity bug hides (§2.2). Splitting is
the structural fix behind §1.1–1.3 being hard to spot.

## Solution

Split each file into single-responsibility modules (no `mod.rs` — AGENTS.md §2.1),
removing one entry from FLUX-087's allowlist per landed split. Start with the two
VMs and the wire codec (highest bug density):

- `FluxBytecodeVM.swift` → per-opcode-family modules (arithmetic, comparison,
  control-flow, capability/await, memory).
- `checker.rs` → per-inference-rule modules.
- `frame.rs` + `wire.rs` → per-frame-type modules (hello/init/delta/heartbeat/
  dispatch/intern) + a shared encode/decode core.
- Parser/bytecode/telemetry/pipeline/ShadowTree/arena → natural sub-responsibilities
  (each file's own section comments usually already hint at the split points).

## Implementation Decisions

- One file per PR, each removing itself from the FLUX-087 allowlist. Keep behavior
  byte-identical (this is a refactor, not a behavior change) — verify with the
  existing snapshot/parity tests.
- Prefer extracting existing `mod` blocks to sibling files over rewriting logic.
- Functions must also stay ≤40 lines as a side effect (flag any that don't and split
  them too).

## Testing Decisions

- Each split is gated by the crate's existing test suite + `cargo nextest` /
  `swift test` / `./gradlew test` + FLUX-087's gate flipping from allowlisted to
  enforced for that file.

## Out of Scope

- The CI gate itself (FLUX-087).
- Behavior changes / bug fixes inside these files (those are FLUX-079–086).
