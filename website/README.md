# Flux documentation site

[![Built with Starlight](https://astro.badg.es/v2/built-with-starlight/tiny.svg)](https://starlight.astro.build)

The Flux docs site is a single-locale (English) [Starlight](https://starlight.astro.build)
site. ADRs live in the repo under `docs/adr` and are linked from the sidebar
rather than copied in.

## Commands

All commands run from `website/`:

| Command         | Action                                              |
| :-------------- | :-------------------------------------------------- |
| `pnpm install`  | Install dependencies                                |
| `pnpm dev`      | Start the local dev server at `localhost:4321`      |
| `pnpm build`    | Build the production site to `./dist/`              |
| `pnpm preview`  | Preview the built site locally                      |
| `pnpm check`    | Run `astro check` (TypeScript / content diagnostics)|
| `pnpm astro …`  | Run any Astro CLI command                           |

## Project structure

```
website/
├── src/
│   ├── assets/traces/   # recorded counter dispatch trace + wire frame
│   ├── components/      # Hero, StatusBadge, DispatchTracePlayer, etc.
│   ├── content/docs/    # markdown/mdx docs (index, concepts, guides, reference)
│   └── content/i18n/    # custom UI strings (single locale today)
├── astro.config.mjs     # Starlight config + sidebar
├── package.json
└── tsconfig.json
```

Docs are `.md`/`.mdx` files under `src/content/docs/`, each exposed as a route by
its file name. The homepage (`index.mdx`) is a Starlight splash page that also
hosts the recorded-dispatch playground.

## Notes

- The ADR ingest script (`scripts/ingest-adrs.ts`) was removed — ADRs are no
  longer copied into the site; the sidebar links out to the repo instead.
- `scripts/check-i18n-drift.ts` is a no-op safety net while the site is
  single-locale; re-enable parity checking there if a new locale is added.
