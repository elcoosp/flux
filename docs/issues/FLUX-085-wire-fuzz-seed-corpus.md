---
id: FLUX-085
status: todo
lane: LANE-WIRE
phase: "Phase 2"
blocked_by: []
labels:
  - rust
  - fuzz
  - wire
source: FLUX_PRODUCTION_READINESS_PLAN.md §2.1 (commit a wire fuzz corpus of real captured frames so wire-fuzz.yml regresses against known-good shapes).
related_adrs: []
---

# FLUX-085: Committed wire fuzz seed corpus (real captured frames)

- **Lane:** LANE-WIRE (Phase 2 — hardening)
- **Owner:** Wire / `flux-ir-serde`
- **Source:** plan §2.1
- **Disjoint from:** every other issue.

## Problem Statement

The wire decoder fuzz target (`fuzz/fuzz_targets/decode_frame.rs`) already has the
right contract ("never panic on attacker bytes") and is wired into `wire-fuzz.yml`.
But the plan notes it fuzzes against **random bytes each run** — there is no
committed **seed corpus of real captured frames**, so the harness can't regress
against known-good shapes (valid frames from a real devserver session, version
handshakes, intern strings, large deltas).

## Solution

- Capture and commit a seed corpus of real frames under `fuzz/corpus/decode_frame/`
  (or the repo's fuzz-corpus convention): valid Hello/Init/Delta/Heartbeat/Dispatch/
  Intern frames, plus boundary cases (max u16 prop indices, content-addressed ids
  from FLUX-074, version-mismatch frames from FLUX-083).
- Ensure `wire-fuzz.yml` feeds the corpus as seeds so every run starts from
  known-good + known-boundary shapes, not just random bytes.

## Implementation Decisions

- Seed files are small raw frame bytes; do not hand-craft them in a way that
  diverges from what the devserver actually emits (capture from a real session where
  possible, or generate via the existing `Frame::to_*_bytes` round-trip of fixtures).
- Keep the random-byte fuzzing too — the corpus is additive, not a replacement.

## Testing Decisions

- `wire-fuzz.yml` runs green against the committed corpus; a genuinely corrupt but
  previously-seen shape still triggers the never-panic contract.

## Out of Scope

- The version fail-closed fixture (FLUX-083).
- The id-ceiling build guard (FLUX-084).
