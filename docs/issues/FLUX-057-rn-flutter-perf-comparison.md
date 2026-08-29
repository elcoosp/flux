---
id: FLUX-057
status: todo
lane: LANE-H
phase: "Phase 5"
blocked_by:
  - FLUX-056
  - PRD-T
labels:
  - perf
  - benchmark
  - release
source: CHANGELOG.md §PRD-T (deferred: "the RN/Flutter published comparison needs equivalent external apps (out of scope for a repo-internal change)")
related_adrs: []
---

# FLUX-057: Published RN/Flutter perf comparison (external apps)

- **Lane:** LANE-H (Phase 5)
- **Depends on:** FLUX-056 (in-repo benches), PRD-T (regression gate)
- **Source:** `CHANGELOG.md` §PRD-T deferred
- **Related ADRs:** —

## Problem Statement

PRD-T deferred "the RN/Flutter published comparison [which] needs equivalent
external apps (out of scope for a repo-internal change)." The roadmap (§7) wants a
public, reproducible benchmark vs RN/Flutter: cold start, hot-reload latency,
large-list scroll, release binary size.

## Solution

Build 2–3 equivalent reference apps (RN + Flutter + Flux) doing the same task
(reuse the FLUX-036 showcase apps where possible) and publish a reproducible
benchmark harness comparing the four §7 metrics. Native codegen with no runtime
interpreter is the structural advantage to prove with numbers.

## Implementation Decisions

- External apps live outside the Flux workspace (or in a separate benchmark repo) —
  they are not Flux source.
- The Flux side reuses `flux-perf-harness` (PRD-J/PRD-T).

## Testing Decisions

- The harness emits a comparison JSON; CI publishes it (no assertion on beating RN
  — it is evidence, not a gate).

## Out of Scope

- The in-repo regression gate (PRD-T/FLUX-056) — those are internal.
