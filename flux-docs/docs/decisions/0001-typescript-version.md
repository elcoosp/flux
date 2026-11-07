# 0001 — Pin TypeScript to 6.x for `astro check`

- **Status:** Accepted
- **Date:** 2026-11-07
- **Deciders:** Documentation Architect (autonomous run)
- **Supersedes:** none
- **Superseded by:** none

## Context

Astro 7.4.7 and Starlight 0.42.10 (the verified baseline for this site) run
type-checking through `astro check`. As of this baseline, `astro check`
fails outright when the workspace resolves TypeScript 7.

Verified at execution time in this project (typescript 7.0.2 installed,
`pnpm astro check` run):

```
13:16:31 [ERROR] [check] astro check does not currently support TypeScript 7.0. To continue using astro check, install TypeScript 6 instead.

astro check will be deprecated in a future Astro release. Experimental support for type-checking Astro files with TypeScript 7.1+ is available through @astrojs/ts-content-mapper. See its README for setup instructions:
https://github.com/withastro/astro/tree/main/packages/language-tools/ts-content-mapper#usage
```

## Decision

Pin `typescript` to the latest 6.x release — **`6.0.3`** — as an exact
devDependency:

```json
"typescript": "6.0.3"
```

`pnpm check` therefore runs against TypeScript 6 and passes.

## Consequences

- `pnpm check` is a working quality gate; TypeScript 7 must not be
  installed in this workspace unless the maintainers explicitly accept the
  experimental `@astrojs/ts-content-mapper` path and verify it.
- The pin is exact (not `^6`) so a routine `pnpm update` cannot resolve
  TypeScript 7 and break `astro check`.
- When Astro ships `astro check` support for TypeScript 7, revisit this
  record, re-verify, and update the pin in one change.
