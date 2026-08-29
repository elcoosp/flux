---
id: FLUX-030
status: todo
lane: LANE-R
phase: "Phase 8"
blocked_by: []
labels:
  - docs
  - website
  - ecosystem
source: CHANGELOG.md §PRD-R (deferred: "the docs website + en/es i18n-drift checker")
related_adrs: []
---

# FLUX-030: Docs website + en/es i18n-drift checker

- **Lane:** LANE-R (Phase 7–8)
- **Depends on:** none
- **Source:** `CHANGELOG.md` §PRD-R deferred follow-ups (`website/` already exists
  as docs + one interactive trace player, in two locales)

## Problem Statement

The website is "docs + one interactive trace player, in two locales (en/es) with
an i18n-drift checker" per the roadmap baseline — but the deferred list calls out
the website + the en/es i18n-drift checker as explicit follow-ups. Drift between
en and es content rots silently.

## Solution

Promote the existing `website/` from a base into a maintained docs site: wire the
i18n-drift checker into CI (fail when an en doc has no es counterpart / es is
stale vs en mtime), and fill the guide gaps (FLUX-031..034). Keep the trace player.

## Implementation Decisions

- The drift checker is a CI script (tests the existing `website/` i18n layout), not
  a new runtime dep. Reuse the `scripts/` convention.
- en is the source of truth; es is the translation target.

## Testing Decisions

- A fixture where es lags en by content triggers a non-zero drift check.
- Adding a matching es file returns clean.

## Out of Scope

- The guide *content* (FLUX-031..034), crash reporting (FLUX-035), app i18n
  (FLUX-036).
