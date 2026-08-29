---
id: FLUX-066
status: done
lane: LANE-J
phase: "Phase 0"
blocked_by:
  - PRD-J
labels:
  - perf
  - ios
  - android
  - benchmark
source: CHANGELOG.md AGENTS.md §0.2 (no render-perf test on either platform) + roadmap §0.2
related_adrs:
  - ADR-0048
---

# FLUX-066: On-device render-perf harness (both platforms, CI-gated)

- **Lane:** LANE-J (Phase 0 — blocking)
- **Depends on:** PRD-J (`MetricRecord` schema exists in Rust)
- **Source:** `AGENTS.md` §0.2 + roadmap §0.2 ("Build the large-tree benchmark suite
  into a repeatable, CI-gated perf test on both Android and iOS hosts")
- **Related ADRs:** ADR-0048

## Problem Statement

The §3.10 "native view mutation < 3 ms" budget is unverified on **both** platforms
because there is no on-device render-perf test. PRD-J built the Rust harness
(`flux-perf-harness`) but it does not run on the actual iOS/Android hosts.
Roadmap §0.2 requires a repeatable, CI-gated on-device perf test before Phase 1+.

## Solution

Port the `MetricRecord` instrumentation into both hosts: node-mutation latency
(observable-props write → next composed frame, per AGENTS.md §3.10 note), dirty-
subset reconciliation size vs full-tree, cold start (dev-session attach → first
frame), release cold start. Publish numbers; gate CI. Feeds FLUX-065's convergence
decision and FLUX-059's DevTools timeline.

## Implementation Decisions

- Reuses PRD-J's `MetricRecord` schema so Rust harness + on-device harness + DevTools
  share one shape.
- Measures the unified-tier definition (observable props write → next frame) on both
  platforms.

## Testing Decisions

- The on-device bench runs in CI (sim for iOS, emulator for Android) and fails if the
  §3.10 budget regresses.

## Out of Scope

- The RN/Flutter comparison (FLUX-057).
