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

# FLUX-030: en/es/fr i18n-drift checker for the existing docs site

- **Lane:** LANE-R (Phase 7–8)
- **Depends on:** none
- **Source:** `CHANGELOG.md` §PRD-R deferred follow-ups (the `website/` base +
  interactive trace player already exist and are committed)

## Problem Statement

The `website/` docs site (Astro + Starlight, locales **en/es/fr**) is already
built and committed — the base is NOT a follow-up. The deferred item that
remains open is the **i18n-drift checker**: drift between the en source docs and
their es/fr translations rots silently (a translated page that lags the en
original, or an en page with no es/fr counterpart, goes unnoticed).

> NOTE: the DISPATCH-DAG LANE-R lane table erroneously points at `docs/site/`,
> which does not exist. The real site is `website/`. This issue targets `website/`.

## Solution

Add an i18n-drift checker that tests the existing `website/src/content/docs/`
i18n layout and wire it into CI: fail when an en doc has no es/fr counterpart, or
when es/fr is stale vs the en mtime. Do NOT build a new site — the base exists.
Keep the trace player intact. The guide *content* gaps (FLUX-031..034) are
separate issues and out of scope here.

## Implementation Decisions

- The drift checker is a CI script (tests the existing `website/` i18n layout), not
  a new runtime dep. Reuse the `scripts/` convention.
- en is the source of truth; es/fr are the translation targets.

## Testing Decisions

- A fixture where es or fr lags en by content triggers a non-zero drift check.
- Adding the matching es/fr file(s) returns clean.

## Out of Scope

- The guide *content* (FLUX-031..034), crash reporting (FLUX-035), app i18n
  (FLUX-036).
