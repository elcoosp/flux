---
id: PRD-T
status: open
lane: LANE-T
phase: "Phase 5"
blocked_by:
  - PRD-J
  - PRD-M
  - PRD-N
labels:
  - epic
  - prd
  - perf
  - benchmarks
  - ci
  - ios
  - android
source: docs/roadmaps/flux-roadmap-to-1.0.md §5,§1.4,§12,§13
related_adrs:
  - ADR-0047
---

# PRD-T: Performance — From "Should Be Fast" to "Proven Fast"

- **Lane:** LANE-T (Phase 5 — maps the roadmap §5 "Phase 5" item, unmapped by the LANE table)
- **Depends on:** PRD-J (perf harness + published numbers), PRD-M (CI matrix), PRD-N (ScrollView for
  large-list benchmark)
- **Source:** `docs/roadmaps/flux-roadmap-to-1.0.md` §5, §1.4, §12, §13
- **Related ADRs:** ADR-0047 (native codegen advantage), PRD-J (render-perf harness), AGENTS.md §3.10
  (perf budgets)

## Problem Statement

The §3.10 performance budgets (parse < 5ms, type-check < 3ms, diff < 1ms, serialize < 1ms, VM eval < 2ms,
signal propagation < 1ms, native mutation < 3ms, end-to-end < 100ms) are aspirational text in a spec —
unverified on either platform (PRD-J establishes the harness). The roadmap's 1.0 requires published,
reproducible benchmarks against RN/Flutter, and a standing perf-regression CI gate, not a one-time push.
Native codegen with no runtime interpreter is a structural advantage on binary size and large-list
scroll, but it is unproven without numbers.

## Solution

Building on PRD-J's harness, publish a public reproducible benchmark comparing cold start, hot-reload
latency, large-list scroll (post PRD-N `ScrollView`), and release binary size against equivalent RN and
Flutter apps doing the same task. Audit the VM dispatch hot path in `flux-vm-ref`/host executors. Extend
the allocation audit (already good Rust practice per AGENTS.md §2.1) to the Kotlin/Swift host runtimes.
Add a perf-regression bot on PRs that comments before/after numbers from the harness, making performance
a standing CI gate.

## User Stories

1. As a Flux core engineer, I want a public, reproducible benchmark vs RN/Flutter for cold start, hot-
   reload, large-list scroll, and binary size, so that "incredible performance" is evidenced, not asserted.
2. As a Fluff app developer, I want the §3.10 budgets met and tracked in CI, so that a regression fails
   the build, not a tweet.
3. As a Flux core engineer, I want a VM-dispatch hot-path audit in `flux-vm-ref`/host executors, so that
   dev-mode (interpreter) latency — which degrades "10x DX" — is understood and bounded.
4. As a Flux core engineer, I want the allocation audit extended to the Kotlin/Swift host runtimes, so
   that the hosts get the same memory discipline Rust already has.
5. As a Flux core engineer, I want a perf-regression bot commenting before/after numbers on each PR, so
   that performance is a standing gate, not a pre-1.0 sprint.
6. As a release manager, I want the binary-size advantage of native codegen (no interpreter) published,
   so that it is a documented differentiator.

## Implementation Decisions

- **One harness, many consumers:** PRD-T's published benchmark and the PRD-J CI gate and the PRD-P
  DevTools flamegraph all read the *same* metric record from PRD-J's harness — no second profiler is
  built.
- **RN/Flutter baselines are real apps:** the comparison uses equivalent RN/Flutter apps doing the *same
  task* (not micro-benchmarks), so the numbers survive reviewer scrutiny. The large-list benchmark
  requires PRD-N's `ScrollView`/`List` to exist first.
- **Dev VM path matters for DX:** even though release ships codegen, a slow dev VM directly degrades the
  dev loop; the hot-path audit treats dev-mode dispatch latency as a first-class budget, not an afterthought.
- **Binary size is structural:** native codegen (ADR-0047) with no interpreter is the lever; publish it
  as the documented advantage the roadmap §0.6 security pass already hints at.
- **Regression bot is CI, not a dashboard:** the before/after comment is emitted from the PRD-J harness in
  CI so it is unavoidable, matching the roadmap's "standing gate" wording.

## Testing Decisions

- **Good test:** a benchmark test asserting the harness produces p50/p95 for each budget line and that the
  regression bot fails a PR that exceeds a threshold by more than the accepted delta. Not tests of the
  baseline apps' UI.
- **Modules to test:** the harness metric emission (PRD-J), the VM-dispatch hot path, the host-runtime
  allocation paths, and the CI regression predicate.
- **Prior art:** PRD-J's harness and LANE-H large-tree benchmarks are the seed; the §3.10 budget table is
  the acceptance criteria.

## Out of Scope

- Building the perf harness (PRD-J).
- The `ScrollView`/`List` primitive (PRD-N) — a prerequisite for the large-list benchmark.
- The CI matrix job (PRD-M) — reused, not built here.
- DevTools flamegraph UI (PRD-P).

## Further Notes

PRD-T fills the §12 gap for "Phase 5 — Performance." It is the evidence base for every performance claim
in 1.0 marketing and the §13 exit metric "render-perf budget met on both platforms, tracked in CI."
