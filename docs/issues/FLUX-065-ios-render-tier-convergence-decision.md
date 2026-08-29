---
id: FLUX-065
status: blocked
lane: LANE-J
phase: "Phase 0"
blocked_by:
  - FLUX-066
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

## Status (2026-08-29) — BLOCKED

FLUX-065 cannot run ADR-0048 Phase 0/1 because the required measurement
infrastructure does not exist in the tree:

- **No render-perf harness.** ADR-0048 Phase 0 calls for a render benchmark on
  both platforms against the §3.10 budget (observable-props write → next composed
  frame, ~50-node subtree, plus single-leaf-dirty and all-dirty cases). That
  harness is FLUX-066, which is still `todo` and itself blocked on PRD-J
  (`flux-perf-harness`).
- **`flux-perf-harness` crate is absent.** Verified: `ls crates/ | grep -i perf`
  returns nothing; the `MetricRecord` schema it would define is referenced only in
  docs/README, never as a Rust type under `crates/`. This is the same root blocker
  as FLUX-056 (large-list scroll benchmark) and FLUX-059 (timeline/flamegraph) —
  all gated on PRD-J.
- **No render-perf test on either platform today.** As ADR-0048 §"Decision"
  already records, iOS has only VM/wire perf tests (`VMDispatchPerfTests`,
  `DeserializeAllocPerfTests`, `StringTablePerfTests`), none measuring view
  mutation; Android has none at all. The §3.10 "< 3 ms" budget is unverified
  everywhere.

Therefore the convergence question (port iOS to SwiftUI vs. ratify the imperative
tier) is **not decidable** from this repo yet — there are no numbers to decide
with. This is exactly the "gated on measurement, not settled" status ADR-0048
already declares; FLUX-065 formalizes that the decision issue is blocked until
FLUX-066 lands the harness and the two tiers are measured.

No prose was edited into AGENTS.md §0.2 or ADR-0048 to paper over the gap — both
already describe the divergence accurately and intentionally (per ADR-0048, the
iOS doc comments must keep describing the UIKit code that actually ships until
the port is made). When PRD-J + FLUX-066 land, the Phase 0/1 measurements go
into ADR-0048 and this issue is closed with a recorded decision.

