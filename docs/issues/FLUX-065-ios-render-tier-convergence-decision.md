---
id: FLUX-065
status: todo
lane: LANE-J
phase: "Phase 0"
blocked_by: []
labels:
  - ios
  - architecture
  - perf
source: CHANGELOG.md AGENTS.md §0.2 (Axis 2: iOS has not converged to the declarative tier) + roadmap §0.1
related_adrs:
  - ADR-0048
---

# FLUX-065: Close the iOS render-tier convergence question (ADR-0048 Phase 0/1)

- **Lane:** LANE-J (Phase 0 — blocking for downstream DX/perf)
- **Depends on:** none (do first)
- **Source:** `AGENTS.md` §0.2 Axis 2 + roadmap §0.1
- **Related ADRs:** ADR-0048 (gated on measurement, not settled)

## Problem Statement

iOS `adapters/ui-swift/FluxUIKit` is still an imperative UIKit reconciler
(`ShadowTreeReconciler` keeps a parallel tree of live view objects) while Android
is declarative Compose. ADR-0048 gates the port on measurement that hasn't happened.
There is **currently no render-perf test on either platform**, so the §3.10
"native mutation < 3 ms" budget is unverified everywhere and the UIKit-vs-SwiftUI
tradeoff is unmeasured. Roadmap §0.1 (blocking) requires this question closed before
Phase 2.

## Solution

Run ADR-0048 Phase 0/1: build the render-perf harness (FLUX-066) against the current
imperative `FluxUIKit`, prototype a minimal declarative SwiftUI dev-tier for one
primitive behind a feature flag, measure both, make the call, and either port or
formally ratify the imperative tier as permanent with a documented rationale. Do
not enter Phase 2 with this open.

## Implementation Decisions

- This is a decision + measurement issue, not a rewrite. The measurement gates the
  rewrite.
- Keep `FluxUIKit` working until the call is made (don't break the iOS build).

## Testing Decisions

- ADR-0048 Phase 0/1 produces numbered measurements (both tiers) — the evidence the
  roadmap requires. The conclusion is written into ADR-0048 (updated) + AGENTS.md
  §0.2.

## Out of Scope

- The actual port (if chosen) — that is a follow-up once measured. The harness
  itself is FLUX-066.
