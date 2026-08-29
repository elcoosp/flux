---
id: FLUX-050
status: todo
lane: LANE-Q
phase: "Phase 6"
blocked_by: []
labels:
  - capability
  - prod
source: CHANGELOG.md §roadmap §0.6 (production update-integrity story) + ADR-0050
related_adrs:
  - ADR-0050
---

# FLUX-050: Production update-integrity story (release codegen advantage)

- **Lane:** LANE-Q (Phase 6, security axis)
- **Depends on:** ADR-0050 (runtime protocol-versioning), PRD-U (1.0 contract freeze)
- **Source:** `CHANGELOG.md` roadmap §0.6
- **Related ADRs:** ADR-0050

## Problem Statement

Roadmap §0.6: "Decide and document the production update-integrity story: since
release builds are native codegen (no interpreter), there is no 'JS bundle OTA'
attack surface RN has — make this a documented security advantage once verified,
not an assumption." Unverified today.

## Solution

Document + verify the release update-integrity story: release builds ship native
codegen (no interpreter), so there is no JS-bundle OTA attack surface. Pair with
ADR-0050's fail-closed version handshake: a host refuses a mismatched protocol
version with an actionable error. Publish this as a security *advantage* doc.

## Implementation Decisions

- No new wire field; relies on ADR-0050's version handshake being fail-closed.
- Verification = a test asserting an old host refuses a new-server `Init`.

## Testing Decisions

- Host-side version-mismatch test on both platforms: actionable error, no misdecode.

## Out of Scope

- The distribution artifacts themselves (LANE-G / ADR-0050 distribution).
