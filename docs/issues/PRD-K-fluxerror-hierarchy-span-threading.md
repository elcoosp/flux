---
id: PRD-K
status: open
lane: LANE-K
phase: "Phase 0.3"
blocked_by: []
labels:
  - epic
  - prd
  - blocking
  - dx
  - errors
  - span
  - rust
  - kotlin
  - swift
source: docs/roadmaps/flux-roadmap-to-1.0.md §0.3,§1.2,§3,§12,§13
related_adrs:
  - LANE-I
---

# PRD-K: Finish and Harden the FluxError Hierarchy + Span-Threading Through the Wire

- **Lane:** LANE-K (Phase 0.3, blocking, parallel to J)
- **Depends on:** none
- **Source:** `docs/roadmaps/flux-roadmap-to-1.0.md` §0.3, §1.2, §3, §12, §13
- **Related ADRs:** LANE-I (`FluxError` hierarchy + permission gate just landed), AGENTS.md §2.1
  (every Rust error carries a `Span`), §3.11 (error message quality bar)

## Problem Statement

The unified `FluxError` hierarchy + permission gate just landed (LANE-I) and the umbrella was
"half-disabled" until a follow-up fix — i.e. it is days old, not hardened. There is no on-device
error overlay and no source-mapped stack trace from `.flux` spans through the VM/wire back to the
editor. A VM-level runtime error on-device today cannot be traced to a `.flux` source location,
which blocks the 1.0 "10x DX" promise (time-to-first-error / time-to-fix must beat RN/Flutter,
measured). The roadmap's error taxonomy is also incomplete across the three language surfaces.

## Solution

Complete and harden one shared `FluxError` taxonomy across Rust (`flux-types`, `flux-devserver`),
Kotlin, and Swift: `Parse`, `Type`, `Permission`/`Capability`, `Wire`, `Vm`, `Codegen`, `Runtime`.
Extend the span guarantee (already the Rust convention) through the wire protocol so a VM-level
runtime error on-device can be traced back to a `.flux` source location, and add property tests
asserting no error path panics plus a lint/clippy rule banning new `unwrap`/`expect`/`!!`/`try!`
outside tests.

## User Stories

1. As a Fluff app developer, I want a VM-level runtime error in dev mode to show the `.flux` source
   span that caused it, so that I can find the bug without guessing.
2. As a Fluff app developer, I want a consistent error taxonomy across Rust, Kotlin, and Swift, so
   that error handling code is portable in my mental model across layers.
3. As a Fluff app developer, I want every error to read with what / where / why / how, so that the
   message tells me how to fix it (AGENTS.md §3.11).
4. As a Flux core engineer, I want a property test that asserts no error path panics, so that error
   handling cannot regress into a crash.
5. As a Flux core engineer, I want CI to reject any new `unwrap`/`expect`/`!!`/`try!` outside tests,
   so that the zero-panic bar (AGENTS.md §2.1) is enforced automatically.
6. As a Fluff app developer, I want a denied capability grant to surface as a typed `FluxError`,
   never a crash, so that permission failures are ordinary control flow.
7. As a Flux core engineer, I want the wire protocol to carry the originating `Span` for VM/`Wire`
   errors, so that on-device errors map back to source for PRD-P DevTools and the on-device overlay.
8. As a release manager, I want the `FluxError` taxonomy to be complete before the on-device overlay
   (PRD-O) and DevTools (PRD-P) are built against it, so that those features assume a stable shape.

## Implementation Decisions

- **Taxonomy is the contract:** the eight-category taxonomy is fixed up front and shared by all three
  host languages; each enum variant carries a `Span` (Rust) / `Span`-equivalent (Kotlin/Swift).
- **Span through the wire:** a `Wire` and `Vm` error transmitted to the host must include the
  originating `Span` (file:line:col) so the on-device overlay (PRD-O) and DevTools component tree
  (PRD-P) can highlight source. This is an *additive* wire field — requires an ADR + protocol version
  bump per AGENTS.md §3.3; do not deviate from Appendix D.
- **No recovery of the half-disabled umbrella until span-threading lands:** the recent LANE-I
  "half-disabled" umbrella must be fully enabled as part of this PRD, with the follow-up fix verified.
- **Lint enforcement:** add a `cargo clippy` lint config (and Kotlin/Swift equivalents) that bans
  `unwrap`/`expect`/`!!`/`try!` outside `#[cfg(test)]` / test sources, making the §13 "exactly 0"
  count CI-enforced rather than asserted in prose.
- **Property tests over unit tests:** `proptest` generators for each error category asserting
  `Debug`/`Display`/`LocalizedError` never panic and always include a span, since snapshot tests
  alone will not catch every error-shape regression.

## Testing Decisions

- **Good test:** tests that exercise error construction from malformed input and assert the resulting
  `FluxError` carries a non-empty span and a what/where/why/how message; tests that the wire round-trip
  preserves the span. Not tests of internal error-internal plumbing.
- **Modules to test:** `flux-types` error constructors, the wire serialization of the new span-bearing
  error field, the Kotlin/Swift error mappings, and the `proptest` panic-freedom generators.
- **Prior art:** AGENTS.md §2.1 already requires `thiserror` in library crates and `Span` on every
  Rust error; build directly on that. LANE-I's permission gate is the seed to harden.

## Out of Scope

- The on-device error overlay UI itself (PRD-O).
- DevTools signal-graph / component-tree source jump (PRD-P).
- Capability *expansion* (new capabilities) — only the error contract for existing ones (PRD-Q).
- iOS/Android render-tier decision (PRD-J).

## Further Notes

PRD-O (overlay) and PRD-P (DevTools) both depend on the span-threading delivered here. This PRD is
marked blocking in the roadmap and must land alongside PRD-J before Phase 1+ features build on top
of it.
