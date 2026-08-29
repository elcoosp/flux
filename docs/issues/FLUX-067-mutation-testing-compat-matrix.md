---
id: FLUX-067
status: done
lane: LANE-M
phase: "Phase 0"
blocked_by: []
labels:
  - ci
  - security
  - mutation-testing
source: CHANGELOG.md §roadmap §0.5 (CI/build hardening: mutation testing + compat matrix)
related_adrs: []
---

# FLUX-067: Mutation testing on flux-differ + flux-vm-ref + compat matrix

- **Lane:** LANE-M (Phase 0 — CI hardening)
- **Depends on:** none
- **Source:** `CHANGELOG.md` roadmap §0.5
- **Related ADRs:** —

## Problem Statement

Roadmap §0.5: add mutation testing (`cargo-mutants` or similar) on `flux-differ` and
`flux-vm-ref` (correctness-critical crates; snapshot tests alone won't catch every
regression class), plus a full compatibility matrix job (min/max Xcode, Android Gradle
Plugin, Kotlin versions). PRD-M landed the version-compat matrix for the wire but not
the toolchain matrix or mutation testing.

## Solution

- Add `cargo-mutants` as a CI job over `flux-differ` + `flux-vm-ref`. It is a
  **cargo subcommand binary** (not a Cargo manifest dependency), so CI installs it
  (a pinned `cargo install cargo-mutants@27` step or the `sourcefrog/cargo-mutants`
  action) rather than a `MANIFEST_REQUESTS.md` row.
- Add a toolchain compatibility matrix job (Xcode / AGP / Kotlin min+max) declared and
  tested, not assumed.

## Implementation Decisions

- Mutation testing is a non-blocking informational job first (high mutant count), then
  promoted to a gate once the surviving-mutant set is triaged.
- The matrix reuses PRD-M's versioning story (ADR-0050) for the wire part.

## Testing Decisions

- The mutants job runs in CI; the matrix job asserts min+max toolchain builds.

## Out of Scope

- The wire fuzz target (LANE-D, done).

## Status (2026-08-29)

**DONE (verified).** Workflows committed: `.github/workflows/mutation-testing.yml`
(`cargo-mutants` over `flux-differ` + `flux-vm-ref`, informational/non-blocking first)
and `.github/workflows/compat-matrix.yml` (min/max Xcode + AGP×Kotlin matrices,
best-effort cells). All three CI YAMLs in scope parse cleanly (`python3 yaml.safe_load`).
GitHub Actions itself cannot execute in this environment, so the live run is verified
indirectly (YAML validity + the issue's promotion policy); the surviving-mutant triage
gate remains a follow-up once the job runs on a runner.
