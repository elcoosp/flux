---
id: FLUX-073
status: todo
lane: LANE-H
phase: "Phase 0/1"
blocked_by: []
labels:
  - perf
  - benchmarking
  - devserver
  - dx
source: Roast #2 (2026-08-29), section 3 — "Your perf budgets live entirely in the fun part of the pipeline." The §3.10 budgets are per-stage Rust micro-budgets (parse, type-check, diff, serialize, VM eval); there is no end-to-end save→photon number, which is the only budget a developer actually feels.
related_adrs:
  - ADR-0041
---

# FLUX-073: Publish an end-to-end save→photon p50/p99 budget on a physical device

## Problem Statement

`AGENTS.md` §3.10 enumerates per-stage performance budgets (parse 500 lines < 5 ms,
type-check < 3 ms, diff 50-node < 1 ms, serialize < 1 ms, VM eval 50-instr < 2 ms).
Every one of these is a *server-side Rust* micro-budget. The number that determines
whether hot reload "feels instant" — **save → pixels on the device** — is not
measured or budgeted anywhere. The dominant costs live outside the Rust pipeline:

- File-watcher debounce/latency (macOS FSEvents coalescing; `DEFAULT_DEBOUNCE`
  is 50 ms in `crates/flux-devserver/src/config.rs`).
- Server pipeline: parse → type-check → lower → diff → serialize.
- WiFi RTT (1–10 ms on LAN; a dropped frame incurs TCP's ~200 ms retransmit).
- On-device decode, signal re-eval, view mutation, layout pass, raster.
- Diff/coalesce windows (`DEFAULT_COALESCE` = 16 ms).

With no save→photon target, regressions in the dominant costs are invisible: a 300 ms
stall from a watcher or WiFi hiccup is indistinguishable from green §3.10 numbers.

## Solution

1. Add a `LANE-H` benchmarking harness that measures **save→photon** end-to-end:
   - Drive the real dev server (`DevServer::start`) + a headless host client over the
     loopback/real WebSocket, editing a fixture `.flux` file and recording the
     wall-clock from `notify` event to the host applying the final patch.
   - Report **p50 / p99** over N edits (N ≥ 50) on at least two tree sizes
     (50-node and 1k-node), matching the §3.10 scale points.
2. Establish a **target budget** (start generous, tighten later): e.g. p99 < 100 ms
   save→photon on LAN for a 50-node tree (the spec's own §3.10 "Save → pixels"
   budget). Document the target in `AGENTS.md` §3.10 as the headline number.
3. Land the harness in CI on a physical-device/simulator runner (or a loopback
   baseline if no device runner exists — clearly labelled as loopback, not LAN).
   Note: criterion-in-CI on shared runners has ±20% noise; prefer a fixed-runner
   job and a wide-enough sample, or gate it as a non-blocking telemetry job rather
   than a flaky hard gate (see Roast #2 note on criterion budgets in CI).

## Implementation Decisions

- Reuse the existing devserver test harness pattern (`crates/flux-devserver/tests/*`)
  for the server end; add a minimal in-crate WebSocket client (or reuse the
  `tokio-tungstenite` client the tests already use) for the host end.
- Keep the per-stage §3.10 budgets — they localize *where* a regression lives; the
  e2e budget catches *that* one exists. They are complementary, not competing.
- Watcher latency is the cheapest, highest-leverage knob: measure the debounce
  contribution first (it alone is 50 ms by default).

## Testing Decisions

- The harness itself is the test: it asserts p99 stays under the agreed target on
  the chosen runner. A regression that pushes p99 over budget fails the job.
- Keep existing per-stage nextest/criterion benches green (they must not regress
  when the e2e harness lands).

## Out of Scope

- Auth/reconnect (see FLUX-075 follow-up candidates) and the WS auth token work
  (landed separately). This issue is purely about *measuring* the e2e budget.
- Replacing MessagePack with a hand-rolled flat codec (Roast #2 §7) — separate
  decision; measure first, optimize only if the codec shows up in the trace.

## Acceptance

- A `LANE-H` e2e bench exists and prints p50/p99 save→photon for ≥ 2 tree sizes.
- `AGENTS.md` §3.10 names save→photon as the headline budget with a stated target.
- CI runs it (blocking or telemetry) on a fixed runner; first measured numbers are
  recorded in the issue as the baseline to tighten against.
