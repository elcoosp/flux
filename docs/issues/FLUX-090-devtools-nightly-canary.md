---
id: FLUX-090
status: todo
lane: LANE-DEVTOOLS
phase: "Phase 3"
blocked_by: []
labels:
  - devtools
  - ci
  - rust
source: FLUX_PRODUCTION_READINESS_PLAN.md §2.6 (DevTools gpui nightly-toolchain CI-stability risk; add a scheduled canary against the next nightly).
related_adrs: []
---

# FLUX-090: DevTools nightly-toolchain canary job

- **Lane:** LANE-DEVTOOLS (Phase 3 — DX/stability)
- **Owner:** DevTools / `flux-devtools-ui`
- **Source:** plan §2.6
- **Disjoint from:** every other issue.

## Problem Statement

`flux-devtools-ui` (gpui) requires a **nightly** toolchain (it uses
`std::hint::cold_path`, per AGENTS.md). A nightly-only crate is a DX and CI-stability
risk: when the pinned nightly breaks, contributors are blocked with no warning. The
plan (§2.6) calls for (1) confirming the exact nightly is pinned in
`rust-toolchain.toml` (it should be) and (2) a scheduled job testing against the
*next* nightly so a break is caught early.

Also §2.6 notes `state.rs` (693 lines) and the time-travel buffer/log modules are
prime candidates for the FLUX-088 size-limit cleanup — tracked separately there.

## Solution

- Confirm `rust-toolchain.toml` pins an exact nightly (not `nightly` channel) for the
  `flux-devtools-ui` build; if it's a floating channel, pin it.
- Add a scheduled (e.g. nightly cron) CI job that builds/tests `flux-devtools-ui`
  against the **next** nightly (`+nightly` override), non-blocking but alerting. A
  break is caught before it hits the pinned toolchain bump window.

## Implementation Decisions

- The canary is best-effort/non-blocking first (nightly can be flaky for unrelated
  reasons); promote to blocking only if it proves stable.
- Does not change the pinned toolchain — only adds a forward-looking check.

## Testing Decisions

- The canary workflow parses and runs `cargo check`/`cargo nextest` against
  `+nightly` for `flux-devtools-ui`.

## Out of Scope

- Splitting `state.rs` (FLUX-088).
- `flux doctor` local checks (FLUX-091).
