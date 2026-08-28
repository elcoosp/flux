---
id: PRD-M
status: open
lane: LANE-M
phase: "Phase 0.5-0.6"
blocked_by: []
labels:
  - epic
  - prd
  - blocking
  - ci
  - security
  - build
  - ios
  - android
source: docs/roadmaps/flux-roadmap-to-1.0.md §0.5,§0.6,§12,§13
related_adrs: []
---

# PRD-M: CI / Build / Security Hardening

- **Lane:** LANE-M (Phase 0.5–0.6, blocking, parallel)
- **Depends on:** none
- **Source:** `docs/roadmaps/flux-roadmap-to-1.0.md` §0.5, §0.6, §12, §13
- **Related ADRs:** AGENTS.md §0.3 (manifest freeze), §3.3 (wire protocol), §1.3 (dependency policy)

## Problem Statement

`flux build` detects `xcodebuild`/`gradle` but does **not** invoke them in the general case — it
logs a manual command when absent. That is a CI/DX gap for anyone without both toolchains. There is
no compatibility-matrix job (min/max Xcode, AGP, Kotlin), no mutation testing on the
correctness-critical `flux-differ` / `flux-vm-ref` crates, and no wire-protocol version-compatibility
test (old dev server ↔ new host and vice versa) before real devices run old host binaries against a
changed wire format. On security, the `CALL_CAP` capability system has no formal threat model and no
fuzz of the dispatch path, and the release codegen "no interpreter → no JS-bundle OTA attack surface"
advantage is assumed, not verified.

## Solution

Wire `flux build` to actually invoke the native toolchain in CI (keeping the log-fallback for local
envs without toolchains); add a compatibility-matrix job; add mutation testing on `flux-differ` and
`flux-vm-ref`; add a wire-protocol version-compatibility test matrix; and run a `CALL_CAP` threat
model + dispatch-path fuzz, documenting the release-update integrity story as a verified security
advantage.

## User Stories

1. As a Flux core engineer, I want `flux build` to invoke `xcodebuild`/`gradle` in CI, so that release
   builds are actually produced and gate-shipped, not just logged as a manual command.
2. As a Flux core engineer, I want a compatibility-matrix job for min/max Xcode, AGP, Kotlin, so that
   supported versions are tested, not assumed.
3. As a Flux core engineer, I want mutation testing on `flux-differ` and `flux-vm-ref`, so that
   snapshot tests alone do not hide a killed-mutation regression in the correctness-critical crates.
4. As a Flux core engineer, I want a wire-protocol version-compatibility test (old server ↔ new host
   and vice versa), so that a device running an old host binary does not silently break against a new
   wire format.
5. As a Flux core engineer, I want a `CALL_CAP` threat model, so that I know whether a malicious `.flux`
   patch can escalate to an undeclared capability.
6. As a Fluff app developer, I want the release codegen "no interpreter" integrity story documented as
   a verified advantage, so that I can cite it in security reviews.
7. As a Flux core engineer, I want the `CALL_CAP` dispatch path fuzzed like LANE-D fuzzed the wire
   (`flux-ir-serde`), so that malformed capability payloads cannot crash the host.
8. As a Flux core engineer, I want `flux build` to distinguish "your `.flux` is wrong" from "your
   Xcode/Gradle setup is wrong" in its failure diagnostics, so that build failures are actionable.

## Implementation Decisions

- **Build invocation:** `flux build` already detects the toolchains; this PRD wires the invocation into
  the release-gate CI path and preserves the "log manual command" fallback for local envs. No manifest
  edits (AGENTS.md §1.3 freeze) — invocation is CI config, not a Cargo/Gradle manifest change.
- **Mutation testing:** prefer `cargo-mutants` (or equivalent) scoped to `flux-differ` and
  `flux-vm-ref` first; broaden only if the budget allows. These two crates are the diff/execute
  correctness core.
- **Wire versioning is Appendix D:** the version-compatibility test is a matrix job, not a protocol
  change. Any *new* compatibility rule still requires an ADR + version bump per AGENTS.md §3.3.
- **Threat model output is an ADR candidate:** the `CALL_CAP` threat model + the release-integrity
  conclusion should land as a new ADR (next free ≥ 0049) since it is a security decision, not just code.
- **Fuzz reuses LANE-D harness:** the capability dispatch fuzz mirrors the `flux-ir-serde` wire fuzz
  shape already established by LANE-D.

## Testing Decisions

- **Good test:** the matrix job fails when an unsupported version combo breaks; the version-compat test
  fails on a real mismatch; the fuzz fails on a host panic. Not tests of CI YAML plumbing itself.
- **Modules to test:** `flux build` invocation + fallback, `flux-differ`/`flux-vm-ref` under mutation,
  the wire version-negotiation path, and the `CALL_CAP` dispatch under malformed input.
- **Prior art:** LANE-D's `flux-ir-serde` fuzz and the existing `flux-differ`/`flux-vm-ref` snapshot
  suites are the seed.

## Out of Scope

- New capabilities (PRD-Q).
- The capability *error* contract hardening (PRD-K) — only the security/fuzz of the dispatch path.
- DevTools / LSP (PRD-O, PRD-P).
- iOS/Android render-tier (PRD-J).

## Further Notes

PRD-M is a Phase 0 exit criterion (wire protocol has a versioning test) and runs parallel to PRD-J /
PRD-K. The compatibility matrix is the foundation PRD-T's public benchmark job will reuse.
