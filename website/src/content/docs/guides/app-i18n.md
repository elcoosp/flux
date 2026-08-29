---
title: App-level i18n
description: Externalize user-facing strings and format per-locale in a Flux app (distinct from the docs-site locales).
---

> This guide is about **app** i18n — making the *app you ship* speak multiple
> languages. It is distinct from the `website/` docs-site locales (en/es/fr),
> which are a separate Astro/Starlight concern handled by the i18n-drift checker
> (FLUX-030).

Flux has no built-in i18n library yet, but the building blocks already exist in
the runtime. The recommended pattern reuses the dev server's **string-interning**
story ([AGENTS.md §3.8](https://github.com/elcoosp/flux/blob/main/AGENTS.md)):
canonical strings live on the server; the host references them by id and falls
back to a deterministic local id only when the server is unreachable (dev only).

## Step 1 — externalize strings as signals

Don't hardcode user-facing text in `Text`. Put it in a `state` (or a store) so a
locale switch just rewrites the signal:

```flux
compo Greeting
  state hello: String = "Hello"     // en
  // state hello: String = "Bonjour" // fr — selected at startup
  Column
    Text text: hello
```

## Step 2 — pick the locale at startup, then load the bundle

A locale is just data. Read it from a capability (e.g. a `Storage` or platform
capability) and map it to a string table:

```flux
// pseudocode shape — a capability returns the selected locale string
compo App
  state locale: String = "en"
  state strings: Map[String, String] = {}   // loaded per locale
  // on mount: read locale, set `strings` to the right bundle
```

> `Map`/dictionary literals are part of the stdlib roadmap; today you model a
> small fixed table with individual `state` signals or pass the bundle via a
> store `compo` (see [State management](/guides/state-management/)).

## Step 3 — locale-aware formatting

For numbers, dates, and currencies, prefer the platform's locale-aware
formatters surfaced through a capability (the same deterministic-id contract as
strings). Keep formatting **off** the Flux source and **on** the host, which
already knows the device locale.

## What's missing (be honest)

- No first-class `i18n`/pluralization/ICU primitive yet — you wire it through
  signals + capabilities today.
- No compile-time string extraction tool. Tracked alongside the docs-site
  drift checker's evolution.
- Plurals and gender are not handled by the runtime; model them as discrete
  strings in your bundle.

For a docs-site that itself needs translation, see the `website/` i18n setup
(Astro/Starlight, en/es/fr) — that's CI-enforced parity, not app code.
