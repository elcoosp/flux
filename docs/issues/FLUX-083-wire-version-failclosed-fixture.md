---
id: FLUX-083
status: todo
lane: LANE-WIRE
phase: "Phase 1"
blocked_by: []
labels:
  - rust
  - android
  - ios
  - wire
source: FLUX_PRODUCTION_READINESS_PLAN.md §2.1 (PROTOCOL_VERSION fail-closed across Kotlin/Swift decoders + cross-language fixture).
related_adrs:
  - ADR-0056
---

# FLUX-083: Cross-decoder `PROTOCOL_VERSION` fail-closed fixture (v1 → v2)

- **Lane:** LANE-WIRE (Phase 1 — gap)
- **Owner:** Wire / `flux-ir-serde` + both host decoders
- **Source:** plan §2.1
- **Disjoint from:** every other issue (adds fixtures + host-side rejection, does not
  touch the storage/differ/VM files).

## Problem Statement

`PROTOCOL_VERSION` is frozen at `2` (`crates/flux-ir-serde/src/frame.rs:41`) and the
Rust decoder already fails closed on mismatch (`frame.rs:224` -> typed `WireError`).
But the plan flags a gap: the **host** decoders — `FrameDeserializer.kt`
(`runtimes/android/.../wire/FrameDeserializer.kt`, has `PROTOCOL_VERSION` +i
`SUPPORTED_VERSIONS` per `scripts/release-gate/check-contract-freeze.sh:66`) and
`FrameDeserializer.swift` — must reject a version-mismatched frame with a clean,
typed error, not a best-effort partial decode. Today only the Rust round-trip is
exercised.

## Solution

- Confirm (and, if missing, add) the explicit typed rejection in both
  `FrameDeserializer.kt` and `FrameDeserializer.swift`: a frame tagged with a
  version outside the supported set must produce a `WireError`/typed rejection
  **before** any field decode.
- Add a **cross-language fixture**: a v1-tagged (or otherwise unsupported) frame
  fed to all three decoders (Rust via `flux-ir-serde`, Kotlin, Swift), asserting a
  clean typed rejection on each. The Rust side already has the path; add it to the
  shared fixture set so the three stay in lockstep.

## Implementation Decisions

- Do NOT bump `PROTOCOL_VERSION` — this issue only verifies the fail-closed path.
- Reuse the existing frame-serialization helpers; the fixture is one v1 frame
  (or an unsupported-version frame) plus three assertion sites.

## Testing Decisions

- Rust: a `flux-ir-serde` unit test asserting `WireError` on the bad-version frame.
- Android: a JVM unit test on `FrameDeserializer` (no emulator) asserting typed reject.
- iOS: an XCTest on `FrameDeserializer` asserting typed reject.
- A CI job (or `perf-harness.yml`-adjacent) that runs all three and diffs the
  rejection type — the cross-language guarantee FLUX-082's error-frame module also
  relies on.

## Out of Scope

- The `STRING_ID_CANONICAL_CEILING` build-time lint (FLUX-084).
- The wire fuzz seed corpus (FLUX-085).
