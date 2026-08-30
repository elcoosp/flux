---
id: FLUX-087
status: todo
lane: LANE-CILINT
phase: "Phase 2"
blocked_by: []
labels:
  - ci
  - rust
  - kotlin
  - swift
source: FLUX_PRODUCTION_READINESS_PLAN.md §1.4 + §2.7 (the project is not compliant with its own ≤300-line-file / ≤40-line-fn rule; add a CI gate so it can't regress).
related_adrs: []
---

# FLUX-087: CI line-count / `unwrap`-in-lib gate (cannot regress §1.4)

- **Lane:** LANE-CILINT (Phase 2 — structural)
- **Owner:** CI / tooling
- **Source:** plan §1.4 + §2.7
- **Disjoint from:** every other issue (adds a CI gate script; does not edit the
  oversized files — that is FLUX-088).

## Problem Statement

AGENTS.md §1.2 mandates "no functions longer than 40 lines; no files longer than
300 lines" and §2.1 "one responsibility per module." The plan measured 11 files
violating this (e.g. `parser.rs` 2233, `bytecode.rs` 1957, `checker.rs` 1774,
`FluxBytecodeVM.swift` 1584, `ShadowTreeReconciler.swift` 985, `arena.rs` 1090).
These aren't cosmetic — they're exactly where §1.1–1.3 hid. Today nothing in
`rust-check.yml` / `android-check.yml` / `ios-check.yml` actually enforces the limit,
so it re-accumulates silently.

## Solution

Add a small custom CI gate (a `tokei`-or-`awk` script is enough) that:
1. Fails if any tracked source file exceeds 300 lines (configurable allowlist for
   generated/committed files that are exempt by decision).
2. Fails if any function exceeds 40 lines (best-effort parse; or gated to Rust via
   `cargo`/`rustc` lint if a crate-level lint exists — otherwise the awk heuristic).
3. Fails if `unwrap`/`expect`/`panic!` appears in non-test Rust code, or `try!`/
   force-unwrap in non-test Swift/Kotlin (the AGENTS.md §2.2 rule).

Wire it into `rust-check.yml` (and the host check ymls) so §1.4 can't regress.
Seed the allowlist with the 11 known-oversized files (they're being split in
FLUX-088) so the gate is green now and tightens as FLUX-088 lands.

## Implementation Decisions

- Keep it a standalone script (`scripts/ci-size-gate.sh` or `scripts/check_agents_rules.py`)
  invoked by the existing check workflows — no new CI runner dependency required beyond
  what's present (tokei is already a project tool per the plan's mention).
- The allowlist is the escape hatch; each FLUX-088 split removes a file from it.

## Testing Decisions

- The gate runs in CI; a synthetic oversized file fails it; removal from allowlist
  then enforces.

## Out of Scope

- The actual file splits (FLUX-088).
- Mutation testing scope (FLUX-067, already done).
