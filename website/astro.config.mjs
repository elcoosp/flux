// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import react from '@astrojs/react';
import mdx from '@astrojs/mdx';
import sitemap from '@astrojs/sitemap';
import { fluxGrammar } from './src/flux-grammar.mjs';

// Flux documentation site.
//
// i18n is Starlight-native (no Lingui/ICU): chrome strings come from the built-in
// dictionaries (zero effort), sidebar labels use `translations` per locale, and
// prose is authored per-locale under `src/content/docs/{root,es,fr}`. Custom UI
// strings (trace-player buttons, frame-inspector headers) live in
// `src/content/i18n/{en,es,fr}.json`.
//
// ADRs are NOT part of the site — they live in the repo under docs/adr and are
// linked from the sidebar (see the "Architecture decisions" link group). Keeping
// them out of the build keeps the site fast and avoids duplicating the
// source-of-truth documents.
export default defineConfig({
  // Required by the sitemap integration; replace with the real deployment URL.
  site: 'https://flux-lang.dev',
  // Register the Flux language so ```flux code blocks highlight across the
  // markdown pipeline (astro-expressive-code reads `markdown.shikiConfig.langs`).
  markdown: {
    shikiConfig: {
      langs: [fluxGrammar],
    },
  },
  integrations: [
    starlight({
      title: 'Flux',
      description:
        'Flux — a write-once UI language for native iOS and Android. Get it running in five minutes.',
      defaultLocale: 'root',
      locales: {
        root: { label: 'English', lang: 'en' },
        es: { label: 'Español', lang: 'es' },
        fr: { label: 'Français', lang: 'fr' },
      },
      // Custom component overrides (see src/components/).
      components: {
        Hero: './src/components/Hero.astro',
      },
      // Manual sidebar groups keep the most useful entry points on top and let
      // the directory contents autogenerate beneath each header. Translated
      // labels are provided per locale via `translations`. Pages surfaced in
      // "Get started" are intentionally NOT re-listed under their directory
      // group, to avoid duplicates in the sidebar.
      sidebar: [
        {
          label: 'Get started',
          translations: { es: 'Empezar', fr: 'Démarrer' },
          items: [
            { label: 'Introduction', link: '/' },
            { label: 'Quickstart', link: '/guides/quickstart/', translations: { es: 'Inicio rápido', fr: 'Démarrage rapide' } },
            { label: 'The Counter example', link: '/guides/counter-example/', translations: { es: 'Ejemplo del Contador', fr: 'Exemple du Compteur' } },
          ],
        },
        {
          label: 'Concepts',
          translations: { es: 'Conceptos', fr: 'Concepts' },
          items: [{ autogenerate: { directory: 'concepts' } }],
        },
        {
          label: 'Guides',
          translations: { es: 'Guías', fr: 'Guides' },
          // `quickstart` and `counter-example` are surfaced in "Get started"
          // above, so they are listed here explicitly rather than via
          // autogenerate (which would duplicate them).
          items: [
            { label: 'Adding a Primitive', link: '/guides/adding-a-primitive/', translations: { es: 'Añadir una primitiva', fr: 'Ajouter une primitive' } },
          ],
        },
        {
          label: 'Reference',
          translations: { es: 'Referencia', fr: 'Référence' },
          items: [{ autogenerate: { directory: 'reference' } }],
        },
        {
          label: 'Architecture decisions',
          translations: { es: 'Decisiones de arquitectura', fr: 'Décisions d’architecture' },
          items: [
            {
              label: 'Read the ADRs (in-repo)',
              translations: { es: 'Lee los ADR (en el repo)', fr: 'Lire les ADR (dans le repo)' },
              link: 'https://github.com/flux-lang/flux/tree/main/docs/adr',
            },
          ],
        },
      ],
      customCss: ['./src/styles/custom.css'],
    }),
    react(),
    mdx(),
    sitemap(),
  ],
});
