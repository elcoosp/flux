# 0005 — Site URL and llms.txt generation

- **Status:** Accepted
- **Date:** 2026-11-07
- **Deciders:** Documentation Architect (autonomous run)
- **Supersedes:** none
- **Superseded by:** none

## Context

The llms.txt convention expects absolute URLs in the index so AI agents can
resolve links without knowing the deployment root. A Starlight site that
wants absolute URLs in generated artifacts needs a canonical `site` origin.

The repository's own `website/astro.config.mjs` already declares
`site: 'https://flux-lang.dev'` as its canonical URL.

## Decision

1. Set `site: 'https://flux-lang.dev'` — the same origin the upstream site
   config declares. No `base` is set: the site deploys from the domain root.
2. Generate **`llms.txt`** and **`llms-full.txt`** at the site root as part
   of `pnpm build`, via `scripts/generate-llms.mjs`:

   - `llms.txt` — one line per page (`- [title](URL): description`),
     grouped Overview / Tutorials / How-to guides / Reference / Explanation
     in sidebar order, plus a link to the source repository.
   - `llms-full.txt` — the full text of every page (frontmatter and MDX
     imports stripped), each with its canonical URL.

3. `public/robots.txt` allows all agents.

## Consequences

- `dist/llms.txt` and `dist/llms-full.txt` exist on every build; the build
  fails loudly if `site:` is missing from `astro.config.mjs`.
- If the canonical domain changes, update `site:` in `astro.config.mjs`
  only — the generator reads it from there rather than duplicating it.
- The generated files mirror the shipped pages; agents reading
  `llms-full.txt` get exactly what human readers get, which keeps AI-facing
  and human-facing content from drifting apart by construction.
