# 0004 — i18n scope: English today, locale-ready structure

- **Status:** Accepted
- **Date:** 2026-11-07
- **Deciders:** Documentation Architect (autonomous run)
- **Supersedes:** none
- **Superseded by:** none

## Context

The repository's own `website/` ships a Starlight site with `root`/`es`/`fr`
locales and a build-gating i18n-drift checker (FLUX-092) that fails the
build when an English page lacks its `es`/`fr` counterparts.

This site is a fresh, standalone Starlight 0.42 build with new information
architecture (Diátaxis directories). Porting the upstream `es`/`fr`
translations would (a) translate pages whose upstream counterparts have a
different structure, and (b) ship translations the upstream drift checker
cannot see — a silent-drift factory.

## Decision

Ship **English as the only authored locale**, but declare the locale map
explicitly so additional locales are additive:

```js
defaultLocale: 'en',
locales: {
  en: { label: 'English' },
},
```

## Consequences

- Translators port content by creating locale subdirectories under
  `src/content/docs/` (for example `src/content/docs/es/`) and adding the
  locale to the map — no config surgery.
- Until a locale exists in this repository, its config entry must not be
  added: an enabled locale with missing pages is the drift this decision
  exists to prevent.
- When this site replaces the repository's `website/`, reuse the upstream
  drift-checker approach (build-gating parity) before re-enabling `es`/`fr`.
- UI strings stay in Starlight's built-in dictionaries; no custom UI-string
  collection is shipped today.
