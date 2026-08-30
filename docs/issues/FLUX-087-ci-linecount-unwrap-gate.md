---
id: FLUX-087
status: done
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

## Resolution (2026-08-30)

Implemented and wired green.

- `scripts/ci-size-gate.sh` (delta gate by default):
  - File length (>300 lines, §1.2) — BLOCKING in delta; respects
    `scripts/ci-size-gate.allowlist` (escape hatch). `tokei` was NOT a real
    project dependency, so the gate uses portable `awk`/`wc`/`git`; no new CI
    runner dep.
  - Forbidden calls (`unwrap`/`expect`/`panic!` in non-test Rust; `try!`/
    force-unwrap in non-test Swift/Kotlin, §2.1/§2.2/§2.3) — BLOCKING regression
    check on newly-added diff lines only, so pre-existing debt isn't punished.
  - Function length (>40 lines, §1.2) — STRICT in `--all` mode, but NON-BLOCKING
    in delta mode: the tree carries widespread pre-existing function debt with no
    per-function allowlist, so blocking it on day one would red the gate. Promote
    to blocking in delta once FLUX-088/function-split work clears the debt.
  - `--all` (strict whole-tree), `--base/--head`, `--selftest` (own throwaway git
    repo), `-v` supported. Embedded selftest passes.
- `scripts/ci-size-gate.allowlist`: seeded with **60** currently-oversized tracked
  production source files (measured 2026-08-30), not the "11" the problem
  statement assumed — the repo's real oversized-file count is ~5x that. Each
  FLUX-088 split removes its line so the rule re-arms.
- Wired into CI: `.github/workflows/size-gate.yml` (always-on, delta blocking +
  non-blocking `--all` report + selftest job) and a `FLUX-087 structural gate`
  step added to `rust-check.yml`, `android-check.yml`, `ios-check.yml`
  (checkouts switched to `fetch-depth: 0` so `origin/main` merge-base resolves).

Note on the "11 files" assumption: the plan's figure was stale; the live tree has
57+ production source files over 300 lines. The allowlist is generated, not
hand-counted, and is kept complete (verified: `--all` reports 0 file-length
failures).
