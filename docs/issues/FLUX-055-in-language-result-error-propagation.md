---
id: FLUX-055
status: todo
lane: LANE-L
phase: "Phase 1"
blocked_by:
  - PRD-Q
  - PRD-L
labels:
  - language
  - types
  - adr
source: CHANGELOG.md §PRD-S (deferred: "an in-language Result/error-propagation story for fallible capability calls")
related_adrs:
  - ADR-0044
  - ADR-0045
---

# FLUX-055: In-language Result / error-propagation for fallible capabilities (ADR-gated)

- **Lane:** LANE-L (Phase 1)
- **Depends on:** PRD-Q (typed error envelope), PRD-L
- **Source:** `CHANGELOG.md` §PRD-S deferred + roadmap §3
- **Related ADRs:** ADR-0044/0045

## Problem Statement

Capabilities can fail (denied grant → typed error, never a crash). The *language*
ergonomics around handling that error should match the runtime contract
(roadmap §3). PRD-S deferred it as ADR-gated.

## Solution

ADR for an in-language `Result`/error-propagation story (e.g. `match` on the
result cell, `try`-like propagation), then extend `flux-types` + lower (reusing
ADR-0044's `Ready`/`Pending`/`Error` cell semantics), then parity tests.

## Implementation Decisions

- The runtime already settles `Ready`/`Error` cells — this is the language surface
  to read them without a crash.
- Targets the frozen grammar.

## Testing Decisions

- A handler that `AWAIT`s a denied capability and matches the `Error` cell
  type-checks + lowers; parity trace pins dev/release.

## Out of Scope

- The capability permission gate (FLUX-049) — that is the security side.
