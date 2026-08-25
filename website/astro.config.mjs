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
// dictionaries (zero effort), sidebar labels use plain `label` strings with a
// `translations` record per locale (Starlight 0.39+ API), and prose is authored
// per-locale under `src/content/docs/{en,es}`. Custom UI strings (trace-player
// buttons, frame-inspector headers) live in `src/content/i18n/{en,es}.json` and
// are read by the React island via `Astro.currentLocale`.
export default defineConfig({
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
        'Flux — a write-once UI language for native iOS and Android. Specs, ADRs, concepts, and an honest recorded-dispatch playground.',
      defaultLocale: 'root',
      locales: {
        root: { label: 'English', lang: 'en' },
        es: { label: 'Español', lang: 'es' },
        fr: { label: 'Français', lang: 'fr' },
      },
      // Custom component overrides (see src/components/).
      components: {
        Hero: './src/components/Hero.astro',
        // Adds a locale-aware "Docs" link to the header so the splash homepage
        // can reach the documentation without its (intentionally hidden) sidebar.
        Header: './src/components/Header.astro',
      },
      // The sidebar uses BOTH mechanisms documented in the design:
      //  - autogenerate groups (labels derived from each page's frontmatter `title`)
      //  - manual items (group headers / index links) with `translations` for locale variants.
      sidebar: [
        {
          label: 'ADRs',
          translations: { es: 'ADR', fr: 'ADR' },
          items: [{ autogenerate: { directory: 'adr' } }],
        },
        {
          label: 'Concepts',
          translations: { es: 'Conceptos', fr: 'Concepts' },
          items: [{ autogenerate: { directory: 'concepts' } }],
        },
        {
          label: 'Guides',
          translations: { es: 'Guías', fr: 'Guides' },
          items: [{ autogenerate: { directory: 'guides' } }],
        },
        {
          label: 'Reference',
          translations: { es: 'Referencia', fr: 'Référence' },
          items: [{ autogenerate: { directory: 'reference' } }],
        },
      ],
      customCss: ['./src/styles/custom.css'],
    }),
    react(),
    mdx(),
    sitemap(),
  ],
});
