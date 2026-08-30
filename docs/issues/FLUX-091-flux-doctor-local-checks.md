---
id: FLUX-091
status: done
lane: LANE-CLI
phase: "Phase 3"
blocked_by: []
labels:
  - cli
  - dx
source: FLUX_PRODUCTION_READINESS_PLAN.md §2.8 (extend flux doctor to check approved-dependency drift + flag AGENTS.md violations locally).
related_adrs: []
---

# FLUX-091: `flux doctor` — local pre-commit checks (dep drift + AGENTS.md violations)

- **Lane:** LANE-CLI (Phase 3 — DX)
- **Owner:** CLI / `flux-cli`
- **Source:** plan §2.8
- **Disjoint from:** every other issue.

## Problem Statement

`flux build`'s emit-only fallback is a good DX call (plan §2.8, first bullet — verify
the emitted manual-build command substitutes real paths, not a template; if it
doesn't, fold that into this issue). The second bullet is the actionable gap:
`flux doctor` (`crates/flux-cli/src/.../doctor.rs`) should be extended to:

1. Check for **approved-dependency version drift** — AGENTS.md §1.3 mandates "latest
   stable, pinned to `^MAJOR`" against an explicit approved list. `doctor` should
   flag any dependency that has drifted off the approved set/version ceiling.
2. Flag **AGENTS.md violations** locally (pre-commit style) so contributors get the
   feedback before CI: file >300 lines, function >40 lines, `unwrap`/`expect`/`panic!`
   in non-test code, `try!`/force-unwrap in non-test Swift/Kotlin. This overlaps with
   FLUX-087's CI gate but runs locally and faster.

## Solution

- Extend `doctor.rs` with two check modules: `check_dependency_drift` (reads the
  approved list / `MANIFEST_REQUESTS.md` and the lockfiles) and `check_agents_rules`
  (the same heuristics as FLUX-087's gate, run locally).
- Verify the `flux build` emit-only fallback message already substitutes real
  `xcodebuild` / `./gradlew` paths; if it emits a template, fix it here.

## Implementation Decisions

- `doctor` is non-fatal by default (advisory) but supports `--strict` for
  pre-commit gating.
- Reuse the FLUX-087 heuristic script where possible (single source of truth for the
  line-count/`unwrap` rules) so the local and CI checks can't drift apart.

## Testing Decisions

- Unit tests on `doctor`: a fixture with a drifted dep + an oversized file + an
  `unwrap` in lib reports exactly those findings; `--strict` exits non-zero.

## Out of Scope

- The CI gate itself (FLUX-087).
- Adding new dependencies (that goes to `MANIFEST_REQUESTS.md` per §1.3).
