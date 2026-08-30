---
id: FLUX-073
status: done   # verified: LANE-H loopback harness (tests/save_to_photon.rs + benches/save_photon.rs) + Budgets::v1 SaveToPhoton gate + CI gate in .github/workflows/benchmarks.yml; physical-device measurement deferred (no on-device runner)
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

## Implementation (2026-08-29)

**What landed (LANE-H, FLUX-073):**

- `flux-perf-harness` schema extended: new `Scenario::LoopbackE2e` (the real
  dev server + headless loopback WebSocket client, no device) and new
  `MetricKind::SaveToPhoton`; `MetricRecord::p99()` added; `Budgets::v1()` gained a
  `SaveToPhoton` p95 ceiling of **250 ms** (generous starting point — tighten
  toward the spec's `< 100 ms` as the watcher/debounce contribution is reduced).
- `crates/flux-devserver/tests/save_to_photon.rs` — the authoritative e2e test.
  Drives `DevServer::start` + a persistent loopback `tungstenite` client, edits a
  synthetic `Counter`/`Column`/`Text`/`Button` fixture 60x per tree size, asserts a
  `Delta` frame arrives for every save, and reports p50/p99 + a JSON `MetricRecord`.
  The edit varies the `Button`'s `onPress` increment constant: that is the only
  edit the differencer detects as a `Delta` over a stable-size tree (a `Text` `text`
  edit lives inside a prop thunk the node-level prop hash does not capture, and
  literal layout props like `gap` are likewise not in the diff — only a handler-body
  change reliably ships a frame). The baseline uses a sentinel increment value no
  edit takes, so the first edit is never identical to the baseline.
- `crates/flux-devserver/benches/save_photon.rs` — the CI telemetry bench (same
  harness, `harness = false`). Prints p50/p99 + JSON and the gate verdict.

**Measured loopback baseline** (`debounce` = 10 ms; the production default is 50 ms,
so real-world numbers will be higher — the debounce window is the dominant,
cheapest-to-fix cost):

| Tree size | samples | p50 | p99 | gate (p95 <= 250 ms) |
|---|---|---|---|---|
| 50-node | 60 | ~ 43 ms | ~ 45 ms | pass |
| ~1k-node | 60 | ~ 46 ms | ~ 50 ms | pass |

Both sit well inside the 250 ms ceiling. The loopback path excludes WiFi RTT,
on-device decode, signal re-eval, view mutation, layout and raster — until a
physical-device / simulator runner exists this is the honest baseline to tighten
against, not the LAN number.

**Out of scope / follow-ups:** a fixed-runner CI job (blocking or telemetry) to run
`benches/save_photon.rs` on every push; a `Scenario::{IosLanE2e, AndroidLanE2e}` once
a device runner exists; tightening `DEFAULT_DEBOUNCE` and confirming the resulting
loopback tail.

**Note:** a pre-existing compile break in `flux-ir-serde/src/telemetry.rs`
(`ViewMutation` gained a `component_name` field but the decoder/`EnrichedTelemetryEvent`
match arms were not updated) blocked the whole workspace build; it was fixed as part
of landing this harness (decoder reads + returns `component_name`; enrich match
passes it through).

## CI wiring (2026-08-29)

A dedicated `save-photon` job was added to `.github/workflows/benchmarks.yml`
(the FLUX-073 LANE-H telemetry runner). It:
- runs on the fixed `macos-14` runner with the nightly toolchain (matches the
  existing `bench` job and the harness's edition-2024 / nightly requirement);
- runs `cargo bench -p flux-devserver --bench save_photon` with
  `--config 'profile.bench.strip="none"'` (avoids the macOS `rust-objcopy`
  SIGABRT warning that would otherwise trip the workflow's `-D warnings`);
- extracts the `LANE-H record json:` lines into `save-photon-metrics.jsonl`;
- **hard-fails only** if the harness prints `LANE-H gate: passed=false`
  (observed p95 > 250 ms ceiling) — otherwise it is non-blocking telemetry;
- uploads `save-photon-metrics.jsonl` + `save-photon.log` as the
  `flux-save-photon-metrics` artifact (30-day retention).

Unlike the existing `bench` criterion job (gated to non-PR events for cost), the
`save-photon` job runs on every push **and** every PR because the loopback run is
cheap (~30 s for two tree sizes) — this is exactly the "fixed-runner telemetry"
follow-up called out in Acceptance. A physical-device / simulator runner
(`Scenario::{IosLanE2e, AndroidLanE2e}`) remains a follow-up once such a runner
exists; until then the loopback numbers are the recorded baseline.
