---
id: FLUX-049
status: done
lane: LANE-Q
phase: "Phase 6"
blocked_by:
  - FLUX-045
labels:
  - capability
  - security
source: CHANGELOG.md §roadmap §8 (capability permission/security hardening) + PRD-Q + LANE-I
related_adrs:
  - ADR-0045
---

# FLUX-049: Capability permission gate + threat model for CALL_CAP

- **Lane:** LANE-Q (Phase 6, security axis)
- **Depends on:** FLUX-045 (concrete capabilities), PRD-K (permission gate)
- **Source:** `CHANGELOG.md` roadmap §0.6 + §8
- **Related ADRs:** ADR-0045, PRD-K

## Problem Statement

Roadmap §0.6: "Formal threat model for `CALL_CAP`: can a malicious `.flux` patch
escalate to a capability the manifest didn't declare?" PRD-Q deferred the native
security story. A denied permission must surface a red banner, not a crash
(LANE-I).

## Solution

Complete the permission gate across all capabilities: a manifest declares granted
capabilities; `CALL_CAP` to an undeclared/denied capability settles a typed error
(never panics). Author the threat model doc and add fuzz coverage on the dispatch
path (LANE-D already fuzzed the wire; extend to capability dispatch).

## Implementation Decisions

- Reuses PRD-K's `FluxError::Capability` variant + the `Permission` gate.
- Must hold on both hosts (iOS/Android) identically.

## Testing Decisions

- A `CALL_CAP` to an undeclared capability on both hosts settles `Error`, never
  panics; a fuzz target over the dispatch path never panics on malformed args.

## Out of Scope

- The wire fuzz target itself (LANE-D, done) — this extends it to capability
  dispatch.
