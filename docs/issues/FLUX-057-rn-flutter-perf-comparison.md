---
id: FLUX-057
status: partial
lane: LANE-H
phase: "Phase 5"
blocked_by: []
labels:
  - perf
  - benchmark
  - release
  - research
source: CHANGELOG.md §PRD-T (deferred: "the RN/Flutter published comparison needs equivalent external apps (out of scope for a repo-internal change)")
related_adrs: []
---

# FLUX-057: Published RN/Flutter perf comparison (web research, no in-repo apps)

- **Lane:** LANE-H (Phase 5)
- **Scope (revised 2026-08-29):** web-research only. **No external RN/Flutter apps are
  built or committed inside this repository.** The user has decided the comparison relies on
  published third-party benchmarks, not on Flux-authored equivalent apps.
- **Related ADRs:** —

## Problem Statement

PRD-T deferred "the RN/Flutter published comparison [which] needs equivalent external apps (out
of scope for a repo-internal change)." The roadmap (§7) wants a public, reproducible benchmark vs
RN/Flutter across four metrics: **cold start**, **hot-reload latency**, **large-list scroll**, and
**release binary size**.

Flux's structural claim is that **release mode codegens idiomatic Swift/SwiftUI + Kotlin/Jetpack
Compose with no VM, no runtime interpreter, no reconciler** (per AGENTS.md §0.2). The published
numbers below are the competitive baseline Flux must meet or beat; they are *evidence*, not a gate.

## Revised Scope Decision (2026-08-29)

- **In-repo external apps: explicitly out of scope.** Do NOT add RN/Flutter reference apps under
  `apps/` or anywhere in this workspace.
- The comparison is produced from **published, citable third-party benchmarks** gathered via web
  research and recorded below. Numbers are directional (different devices, app shapes, and
  methodology), so they are reported as ranges with sources, not as a single authoritative figure.
- The Flux side of the comparison is **not** produced here; it belongs to `flux-perf-harness`
  (PRD-J) once the `runtimes/` host adapters wire their `MeasureFn` closures (parallel-owned work).
  This issue only establishes the external baseline.
- `blocked_by` cleared: FLUX-056 / PRD-T gate the *Flux* measurement, which is not part of this
  research-only issue.

## Findings — published RN/Flutter baseline (web research)

All figures are from independent 2025–2026 comparisons. Treat as ranges; methodology varies.

| Metric (§7) | Flutter | React Native | Source |
|---|---|---|---|
| **Cold start** (first frame) | ~250 ms mid-range Android; <50 ms first-frame in same-app tests | ~350 ms mid-range Android; <50 ms first-frame in same-app tests | tech-insider 2026 spec table; synergyboat 2025 (first frame <50 ms all) |
| **Hot reload latency** | 0.4–0.8 s (sub-second) | 1.2–1.8 s (Fast Refresh) | tech-insider 2026 spec table; agilesoftlabs 2026 |
| **Large-list scroll** (10k items) | 60 FPS, smooth | 50–58 FPS, noticeable jank under stress | tech-insider 2026 (list-scroll update); sudolabs / betterium scroll tests |
| **Release binary size** (APK) | 38–42 MB | 28–32 MB | tech-insider 2026 spec table; synergyboat 2025 (Native smallest, Flutter mid, RN/Expo largest) |

### Notes / caveats

- Flutter leads on raw rendering (Impeller, GPU-shaded, AOT to ARM) and hot reload; RN leads on
  binary size and ecosystem breadth (npm). Both are far from "native" cold start in absolute terms.
- The synergyboat 2025 benchmark built **the same Flashcard app** in Flutter, RN, and Native for an
  apples-to-apples read: Flutter quickest first frame, RN most consistent, Native smallest memory
  and binary. That is the closest thing to a controlled comparison available publicly.
- React Native's numbers assume the **New Architecture** (Fabric + Hermes/JSI); older bridge
  architecture is materially slower and not representative of current RN.

### Where Flux sits vs this baseline (structural claim, not yet measured)

Flux release mode emits native SwiftUI/Compose — i.e. it targets the **Native** column's
characteristics (small binary, no interpreter, native cold start), not the RN/Flutter VM columns.
The §3.10 budgets in AGENTS.md (node mutation <3 ms, save→pixels <100 ms) are the internal bar; the
external baseline above is the market context. Closing the loop requires the Flux-side measurement
from PRD-J, which is parallel-owned.

## Sources

- Tech Insider — "Flutter vs React Native 2026" (spec table: cold start, hot reload, FPS, memory,
  app size). https://tech-insider.org/flutter-vs-react-native-2026/
- Synergyboat — "Flutter vs React Native vs Native: 2025 Performance Benchmark" (same-app Flashcard
  benchmark; first-frame <50 ms; binary-size ordering).
  https://www.synergyboat.com/blog/flutter-vs-react-native-vs-native-performance-benchmark-2025
- Agilesoftlabs — "Flutter vs React Native 2026: Performance Cost DX" (hot reload 1–3 s RN vs
  sub-second Flutter). https://www.agilesoftlabs.com/blog/2026/02/flutter-vs-react-native-2026-cost-dx_17
- Sudolabs — "Flutter and React Native performance overview" (list-scroll FPS under load).
  https://sudolabs.com/insights/flutter-and-react-native-performance-overview

## Testing Decisions

- No tests: this is a research/doc issue. The only artifact is this record + the source links.
- The downstream *measurement* (Flux side) is covered by PRD-J (`flux-perf-harness`) and its CI
  gate; this issue does not add code.

## Out of Scope

- Building RN/Flutter reference apps in this repo (explicitly excluded per user decision).
- The Flux-side measurement (PRD-J / `flux-perf-harness`).
- The in-repo regression gate (PRD-T / FLUX-056) — internal, separate.
