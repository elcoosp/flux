---
id: PRD-J
status: open
lane: LANE-J
phase: "Phase 0.1-0.2"
blocked_by: []
labels:
  - epic
  - prd
  - blocking
  - parity
  - perf
  - ios
  - android
  - readiness
source: docs/roadmaps/flux-roadmap-to-1.0.md §0.1,§0.2,§1.4,§12,§13
related_adrs:
  - ADR-0048
  - ADR-0027
---

# PRD-J: Close the iOS/Android Render-Tier Question and Stand Up the Render-Perf Harness

- **Lane:** LANE-J (Phase 0.1–0.2, blocking)
- **Depends on:** none — must run first
- **Source:** `docs/roadmaps/flux-roadmap-to-1.0.md` §0.1, §0.2, §1.4, §12, §13
- **Related ADRs:** ADR-0048 (iOS dev-tier convergence), ADR-0027 (R-graph / node-ID bridge), AGENTS.md §0.2 / §3.10

## Problem Statement

Flux's 1.0 definition requires iOS/Android architectural parity, but the two platforms
currently render through different tiers: Android is declarative (`ShadowTreeRenderer` +
`DirtyReconciler.reconcileDirty` touching only `dependents[S]`), while iOS is still an
imperative UIKit reconciler (`adapters/ui-swift/FluxUIKit`, `ShadowTreeReconciler` owning a
parallel tree of live view objects). ADR-0048 gates the port on a measurement that has not
happened, and there is **no render-perf test on either platform** — so the §3.10
"native view mutation < 3ms" budget is unverified everywhere. Every downstream DevTools and
perf feature in the roadmap assumes one consistent rendering model to instrument, so this
question cannot stay open past Phase 0.

## Solution

From the user's perspective: make the ADR-0048 call with data, write the conclusion, and stand
up a repeatable, CI-gated render-perf harness that instruments both hosts and both execution
paths (dev VM + release codegen) so every future performance claim is backed by numbers.

1. Build the render-perf harness (LANE-H already landed parts of the large-tree benchmark —
   promote it to a CI-gated, both-platforms suite). Instrument: node mutation latency,
   dirty-subset reconciliation size vs full-tree size, WebSocket patch round-trip, VM dispatch
   latency, dev-session cold start (attach → first frame), release cold start.
2. Run the harness against the current imperative `FluxUIKit` reconciler on iOS.
3. Prototype a minimal declarative SwiftUI dev-tier for one primitive (`Text`) behind a feature
   flag and measure it against the same harness.
4. Make the ADR-0048 call (port to match Android, or ratify UIKit as permanent with rationale),
   publish the numbers, and close the question.

## User Stories

1. As a Flux core engineer, I want the render-perf harness to run on both iOS and Android in CI,
   so that the §3.10 budgets are measured, not asserted.
2. As a Flux core engineer, I want the harness to report node-mutation latency and dirty-subset
   reconciliation size separately from full-tree reconciliation, so that I can see whether
   `reconcileDirty` is doing its job.
3. As a Flux core engineer, I want the harness to measure both the dev VM path and the release
   codegen path independently, so that regressions in either backend are caught early.
4. As a Flux core engineer, I want to run the harness against the current imperative iOS
   reconciler and capture a baseline, so that any port is compared against a fixed number.
5. As a Flux core engineer, I want a feature-flagged declarative SwiftUI `Text` prototype measured
   by the same harness, so that the ADR-0048 decision is grounded in data.
6. As a Flux core engineer, I want ADR-0048 updated with the conclusion and the published numbers,
   so that no future work reopens the question by assumption.
7. As a Fluff app developer, I want one consistent rendering model across iOS and Android, so that
   DevTools and codegen behave identically on both platforms.
8. As a release manager, I want the perf harness to fail CI when the §3.10 mutation budget is
   exceeded, so that 1.0 ships with a verified budget.

## Implementation Decisions

- **Harness shape:** a shared benchmark driver plus platform host adapters. Android side runs on
  the JVM host (`runtimes/android/host`, no emulator needed for the reactive core timings) and the
  iOS side runs against the `FluxUIKit` reconciler on a simulator; both feed the same metric schema.
- **One source of truth:** DevTools timeline/flamegraph (PRD-P) will consume the same instrumented
  metrics, so the harness must emit a stable, parseable metric record (cold start, patch RTT,
  dirty-reconcile size, dispatch latency) — not just stdout.
- **ADR-0048 outcome is data-driven:** whichever direction is chosen, the decision record states the
  measured numbers and the rationale. If UIKit is ratified as permanent, the doc comments in
  `FluxUIKit` already describe the imperative tier accurately and must stay that way (AGENTS.md §0.2
  forbids rewriting them to claim a SwiftUI tier that does not exist).
- **Feature flag:** the declarative SwiftUI prototype is gated so it cannot ship to release builds
  until the decision lands; the flag default keeps today's imperative tier active.
- **No new opcodes / wire fields** as part of this PRD — this is measurement + a decision, not a
  protocol change.

## Testing Decisions

- **Good test:** a test that asserts the harness produces a complete metric record within the
  harness's own budget and that the record parses into the shared schema; not a test of platform
  UI internals.
- **Modules to test:** the harness driver (deterministic metric emission on a fixed fixture tree),
  the metric-schema (de)serialization, and the CI gate predicate (pass/fail on threshold).
- **Prior art:** `flux-parity`'s dev/release trace-diff harness and LANE-H large-tree benchmarks
  are the closest existing instrumentation; mirror their fixture-tree construction and CI wiring.
- The harness itself must not be flaky: use a fixed warm tree and report p50/p95, not a single
  sample.

## Out of Scope

- Porting `FluxUIKit` wholesale to a declarative tier (that is a follow-on only if ADR-0048 so
  concludes).
- Building DevTools flamegraphs (PRD-P).
- Adding new stdlib primitives (PRD-N).
- The async-resolver / capability error contract (PRD-K).

## Further Notes

This is the single most blocking PRD in the 1.0 sequence: PRD-N (stdlib) explicitly depends on the
rendering model being settled, and PRD-P (DevTools) depends on both the perf instrumentation and
the span-threading from PRD-K. Track the published numbers as a dashboard, not prose (roadmap §13).
