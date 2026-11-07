# 0002 — Plugin selection

- **Status:** Accepted
- **Date:** 2026-11-07
- **Deciders:** Documentation Architect (autonomous run)
- **Supersedes:** none
- **Superseded by:** none

## Context

Starlight 0.42 supports first-party and community plugins. The candidate
list evaluated against what this documentation site actually needs:

| Plugin | Version (verified) | Relevant? |
| :-- | :-- | :-- |
| `starlight-links-validator` | 0.27.0 | Yes — 26 pages cross-link heavily |
| `starlight-image-zoom` | 0.16.2 | Yes — screenshots/diagrams grow over time |
| `starlight-heading-badges` | 0.8.0 | Yes — flags reference-only or experimental sections |
| `starlight-typedoc` | 0.23.2 | No |
| `starlight-blog` | 0.30.2 | No |
| `starlight-openapi` | (n/a) | No |
| `starlight-versions` | 0.10.2 | No |
| `@astrojs/starlight-docsearch` | 0.8.1 | No |

## Decision

Install and enable: **`starlight-links-validator`**, **`starlight-image-zoom`**,
**`starlight-heading-badges`**.

Omit the rest, with reasons:

- **`starlight-typedoc`** — Flux is a Rust project; there is no TypeScript
  API surface to generate documentation from. The API analog (`flux doc`,
  the stdlib JSON schema) is documented as a reference page instead.
- **`starlight-blog`** — the project records history in `CHANGELOG.md` in
  the repository. A blog would invite invented entries; none exist.
- **`starlight-openapi`** — no OpenAPI spec exists for Flux.
- **`starlight-versions`** — Flux is a single-version, pre-1.0 project;
  versioned docs would ship empty version sets.
- **DocSearch** — no approved Algolia DocSearch application exists for this
  project. Starlight's built-in **Pagefind** remains the search solution and
  is not disabled.

## Consequences

- `pnpm build` fails on broken internal links (release blocker).
- Images support click-to-zoom in both themes.
- Section headings can carry badges via heading-badges syntax.
- Each omission above must be revisited if the project gains the matching
  artifact (TS API surface, blog, OpenAPI spec, release versions, Algolia
  app).
