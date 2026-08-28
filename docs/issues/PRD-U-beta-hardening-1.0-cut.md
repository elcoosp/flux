---
id: PRD-U
status: open
lane: LANE-U
phase: "Phase 9"
blocked_by:
  - PRD-J
  - PRD-K
  - PRD-L
  - PRD-M
  - PRD-N
  - PRD-O
  - PRD-P
  - PRD-Q
  - PRD-R
  - PRD-S
  - PRD-T
labels:
  - epic
  - prd
  - release
  - beta
  - readiness
  - ios
  - android
source: docs/roadmaps/flux-roadmap-to-1.0.md §9,§1,§12,§13
related_adrs:
  - ADR-0047
---

# PRD-U: Beta, Hardening & 1.0 Cut

- **Lane:** LANE-U (Phase 9 — maps the roadmap §9 "Phase 9" item, unmapped by the LANE table)
- **Depends on:** all prior PRDs (it is the terminal gate)
- **Source:** `docs/roadmaps/flux-roadmap-to-1.0.md` §9, §1, §12, §13
- **Related ADRs:** ADR-0047 (codegen), PRD-J (render parity), PRD-K (FluxError), PRD-M (wire versioning)

## Problem Statement

The 1.0 cut has no lane in the §12 table, yet it is the gate that decides whether Flux actually ships.
The roadmap is explicit that 1.0 ships only when the five §1 criteria are met *with evidence*, not when
the feature checklist is full. Today there is no dogfood app, no closed beta tracking DX friction against
real time-to-fix data, no frozen wire/adapter contract versions with a backward-compatibility policy,
and no bug bash across the full stdlib + capability surface.

## Solution

Dogfood: the core team ships one real app on Flux before external users. Run a closed beta with a small
set of external teams building real apps; track every DX friction point against the §1 "10x" claim with
actual time-to-diagnose / time-to-fix data, not vibes. Freeze the wire protocol and adapter-contract
versions for 1.0 with an explicit backward-compatibility policy (semver already adopted per CHANGELOG.md —
extend it to wire/adapter contracts, which version independently of crate versions today). Bug-bash the
full stdlib + capability surface. Cut 1.0 only when the five §1 criteria are met with evidence.

## User Stories

1. As a Flux core engineer, I want the core team to ship one real app on Flux first, so that 1.0 is
   dogfooded, not theorized.
2. As a Flux core engineer, I want a closed beta with external teams building real apps, so that real
   friction surfaces before GA.
3. As a release manager, I want DX friction tracked as median time-to-diagnose / time-to-fix vs a matched
   RN/Flutter cohort, so that the "10x DX" claim is measured, not asserted.
4. As a Flux core engineer, I want the wire protocol and adapter-contract versions frozen for 1.0 with an
   explicit backward-compatibility policy, so that host binaries and dev servers have a compatibility
   contract (extends PRD-M's versioning).
5. As a Flux core engineer, I want a bug bash across the full stdlib + capability surface, so that 1.0
   ships against exercised code.
6. As a release manager, I want 1.0 cut only when the five §1 criteria are met with evidence (90% coverage,
   10x DX, DevTools parity+, verified perf budget, iOS/Android parity), so that 1.0 means what the roadmap
   says it means.
7. As a Fluff app developer, I want the frozen wire/adapter contract to be published, so that I can plan
   host-app upgrades against a known compatibility policy.

## Implementation Decisions

- **Evidence over checklist:** the cut decision is gated on the §13 dashboards (render-perf budget met in
  CI, published RN/Flutter numbers, stdlib coverage checklist, zero `unwrap`/etc., DevTools parity
  checklist, beta-tracked time-to-diagnose/fix) — not on a feature count. This is the roadmap's explicit
  warning against shipping "when the feature checklist is full."
- **Version independence is real:** wire protocol and adapter contract version *independently* of crate
  versions today; the freeze extends semver (already in CHANGELOG.md) to those contracts explicitly,
  building on PRD-M's version-compatibility test.
- **Beta data is the differentiator:** the time-to-diagnose / time-to-fix tracking against an RN/Flutter
  cohort is what makes "10x DX" defensible; it is collected during the closed beta, not estimated upfront.
- **Dogfood app is the integration anchor:** the core team's real app exercises the PRD-N stdlib + PRD-Q
  capabilities end-to-end and becomes the canonical 1.0 fixture.

## Testing Decisions

- **Good test:** the §13 exit dashboards are green (harness gate passes, coverage checklist complete,
  DevTools parity checklist complete, beta time-to-fix within target); a contract test asserting the frozen
  wire/adapter versions reject incompatible peers with the published policy. Not a feature test.
- **Modules to test:** the frozen wire/adapter version-negotiation (PRD-M), the §13 dashboard gates
  (PRD-J/PRD-T/PRD-P), and the dogfood-app CI build.
- **Prior art:** PRD-M's wire version-compatibility matrix and PRD-J/PRD-T's CI gates are the foundation;
  this PRD assembles them into the release decision.

## Out of Scope

- Building any feature — PRD-U only gates and cuts; all capability/primitive/tooling work is in prior PRDs.
- The individual perf harness / DevTools / LSP — those are delivered by PRD-J/P/O and consumed here.

## Further Notes

PRD-U is the terminal PRD and the deliverable behind roadmap §1 ("Ship when all five are true") and §9.
It depends on every other PRD because its only job is to verify the evidence and freeze the contracts.
