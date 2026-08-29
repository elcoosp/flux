---
id: FLUX-046
status: done
lane: LANE-C
phase: "Phase 6"
blocked_by:
  - PRD-Q
labels:
  - capability
  - escape-hatch
source: CHANGELOG.md §PRD-Q (deferred: native SDK escape hatch per-ADR) + roadmap §8
related_adrs:
  - ADR-0045
---

# FLUX-046: Documented first-class native-module escape hatch (user-facing)

- **Lane:** LANE-C (Phase 6)
- **Depends on:** PRD-Q (`CapabilityRegistry::register` + `derive_capability_id`)
- **Source:** `CHANGELOG.md` §PRD-Q + roadmap §8
- **Related ADRs:** ADR-0045

## Problem Statement

PRD-Q shipped the *mechanism* (`register(cap_id, method_id, impl)`) but the
roadmap (§8) calls for a *documented, first-class way to wrap an arbitrary native
SDK as a capability* — a user-facing story (how a team declares + registers a
wrapper, how ids stay deterministic, how the host binds it).

## Solution

Author the escape-hatch as a user-facing capability: a `.flux` declaration syntax
for a user capability + a host-side registration guide, with a reference wrapper
(e.g. wrap one real SDK) proving the end-to-end path. Deterministic ids via
`derive_capability_id` so server + both hosts agree (AGENTS.md §3.4).

## Implementation Decisions

- Uses PRD-Q's registry; this issue is the *documentation + reference wrapper*, not
  new contract code.
- A denied grant still settles `Ready`/`Error` (ADR-0044) — the escape hatch never
  bypasses the gate.

## Testing Decisions

- A reference wrapper registered + called on both hosts asserts the real native
  side-effect; a denied grant surfaces a typed error.

## Out of Scope

- The six built-in capabilities (FLUX-045) — those are concrete, this is the
  general mechanism's user story.
