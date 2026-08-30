---
id: FLUX-084
status: todo
lane: LANE-WIRE
phase: "Phase 1"
blocked_by: []
labels:
  - rust
  - android
  - ios
  - wire
source: FLUX_PRODUCTION_READINESS_PLAN.md §2.1 (STRING_ID_CANONICAL_CEILING fallback masking must fail the build if a wire path emits an id >= ceiling).
related_adrs: []
---

# FLUX-084: Build-time guard that no wire path emits an id ≥ `STRING_ID_CANONICAL_CEILING`

- **Lane:** LANE-WIRE (Phase 1 — gap)
- **Owner:** Wire / `flux-ir-serde` + both hosts
- **Source:** plan §2.1
- **Disjoint from:** every other issue.

## Problem Statement

`STRING_ID_CANONICAL_CEILING = 0x8000_0000` (`crates/flux-ir-serde/src/frame.rs:67`)
is the boundary: ids at/above it are the "last-resort local fallback" the host must
never synthesize in production (AGENTS.md §3.8 — canonicality is absolute; the local
fallback is dev-only and non-fatal). The plan flags that today this is enforced only
by code-review discipline — a wire path that accidentally emits an id ≥ the ceiling
slides through.

## Solution

Add a build/lint-time assertion that fails the build if a wire-path emits an id ≥
`STRING_ID_CANONICAL_CEILING`:

- **Rust:** a `const` assertion / unit test in `flux-ir-serde` covering the string
  interning paths (the `InternString`/`StringInterned` round-trip already asserts
  `resp.id < STRING_ID_CANONICAL_CEILING` at `frame.rs:1256`/`1279` — promote this
  from runtime `debug_assert` to a compile-time-or-CI gate that also covers the
  emit side).
- **Android / iOS:** add an equivalent assertion in the host string-interning emit
  paths (or a shared lint rule) so the contract holds on both hosts.

The goal is a hard failure on any path that would produce a ≥-ceiling id, not a
runtime log line.

## Implementation Decisions

- Prefer `const { ... }` compile-time checks in Rust where the value is statically
  known; otherwise a CI-gated unit test that exercises the intern emit path.
- Mirror the rule in host-side tests so all three decoders/encoders agree.

## Testing Decisions

- A deliberately-bad test (id ≥ ceiling) must fail the build/CI.
- The normal intern path must still pass with ids < ceiling.

## Out of Scope

- The version fail-closed fixture (FLUX-083).
- The wire fuzz seed corpus (FLUX-085).
