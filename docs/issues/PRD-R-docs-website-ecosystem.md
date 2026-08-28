---
id: PRD-R
status: open
lane: LANE-R
phase: "Phase 7-8"
blocked_by:
  - PRD-N
  - PRD-Q
labels:
  - epic
  - prd
  - docs
  - website
  - ecosystem
  - i18n
source: docs/roadmaps/flux-roadmap-to-1.0.md §7,§8,§10,§12,§13
related_adrs:
  - ADR-0047
  - ADR-0029
---

# PRD-R: Docs, Website, Ecosystem & Production Concerns

- **Lane:** LANE-R (Phase 7–8)
- **Depends on:** rolling, behind PRD-N / PRD-Q
- **Source:** `docs/roadmaps/flux-roadmap-to-1.0.md` §7, §8, §10, §12, §13
- **Related ADRs:** ADR-0047 (codegen), ADR-0029 (grammar), PRD-K (FluxError taxonomy),
  AGENTS.md §0.7 (spec/doc reconciliation)

## Problem Statement

The website is docs + one interactive trace player in two locales (en/es) with an i18n-drift checker —
a real base, but thin on guides, cookbooks, and migration content. There is no package manager for
`.flux` components, no testing framework for `.flux` apps, no crash-reporting story for release builds,
no published state-management guidance, and no app-level i18n. The roadmap's 1.0 requires a full guide
set, 2–3 substantial showcase apps, and the ecosystem plumbing that lets a community stdlib form — the
core team cannot build 90% coverage alone.

## Solution

Convert concept docs (`dev-vs-release`, `host-authoritative-state`, `the-wire`) into a full guide set:
getting started, a cookbook per new stdlib primitive (PRD-N), honest migration guides *from* RN and
Flutter, and a troubleshooting guide keyed to the PRD-K `FluxError` taxonomy. Keep the i18n-drift
checker in the loop. Build a minimal package registry + `flux add <pkg>`. Ship a headless `.flux` app
testing framework (mirrors `flux-parity` dev/release parity, user-facing). Add a release crash-reporting
story (Sentry-equivalent via Swift/Kotlin reporters). Publish opinionated state-management patterns.
Add app-level i18n (string externalization + locale-aware formatting). Build 2–3 showcase apps that
exercise the PRD-N stdlib end-to-end and double as living integration tests + marketing.

## User Stories

1. As a Fluff app developer, I want a getting-started guide + a cookbook per stdlib primitive, so that I
   can build real features without reading the source.
2. As a Fluff app developer, I want an honest migration guide from RN and from Flutter, so that I know
   the real differences before committing.
3. As a Fluff app developer, I want a troubleshooting guide keyed to the `FluxError` taxonomy (PRD-K),
   so that an error message points me to a fix.
4. As a Fluff app developer, I want `flux add <pkg>` against a minimal registry, so that I can reuse
   community `.flux` components instead of rebuilding them.
5. As a Fluff app developer, I want a headless `.flux` app testing framework, so that I can test my app
   against the dev VM without a device.
6. As a release engineer, I want crash reporting in release builds, so that production apps ship with
   visibility (Sentry-equivalent).
7. As a Fluff app developer, I want published state-management patterns (global stores, derived signals,
   async fetch), so that I do not reinvent Redux badly on top of the signal graph.
8. As a Fluff app developer, I want app-level i18n (string externalization + locale formatting), so that
   I can ship in multiple languages.
9. As a Flux core engineer, I want 2–3 showcase apps exercising the PRD-N stdlib, so that they double as
   integration tests and marketing.
10. As a docs maintainer, I want the en/es i18n-drift checker to keep es from rotting behind en, so that
    localized docs stay honest.

## Implementation Decisions

- **Docs mirror code, not aspiration:** AGENTS.md §0.7 already flags spec/code drift; PRD-R docs are
  written against the *actual* shipped primitives/capabilities (PRD-N/PRD-Q) and the reconciled spec,
  not the roadmap's ambitions. Migration guides name differences honestly (roadmap calls this a
  credibility move).
- **Package registry is minimal first:** `flux add <pkg>` + a registry index; not a full versioned
  dependency resolver in v1. It unlocks a community stdlib the core team does not have to build alone.
- **App testing reuses `flux-parity`:** the user-facing `.flux` test framework mirrors `flux-parity`'s
  dev/release parity engine — component-level tests run headlessly against the dev VM.
- **Crash reporting is "just" a Swift/Kotlin integration:** because release is native codegen, there is
  no interpreter to instrument; this PRD wires a standard reporter, it does not invent a Flux-specific
  one.
- **Showcase apps are integration tests:** the 2–3 showcase apps are first-class fixtures in CI (exercising
  the PRD-N stdlib) so they cannot silently rot.

## Testing Decisions

- **Good test:** docs build + i18n-drift check pass; `flux add` installs a fixture package and it
  compiles; the `.flux` test framework runs a fixture app headlessly and asserts behavior; a showcase app
  builds in CI. Not tests of prose.
- **Modules to test:** the `flux add` resolver (minimal), the `.flux` test harness, the showcase-app CI
  builds, and the docs i18n-drift checker.
- **Prior art:** `flux-parity`'s dev/release parity testing is the engine for the app test framework; the
  existing docs i18n-drift checker is the seed for the docs gate.

## Out of Scope

- Building the stdlib primitives the cookbooks document (PRD-N).
- The capability error contract (PRD-K) — docs reference it.
- The LSP (PRD-O) — separate DX surface.
- The iOS/Android render-tier (PRD-J).

## Further Notes

PRD-R is intentionally "rolling, behind N/Q": docs and ecosystem are most valuable once the surface
they document exists. It is the deliverable behind roadmap §1.3 ("ship it") and §10.
