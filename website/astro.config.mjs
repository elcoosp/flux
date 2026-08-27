// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import react from '@astrojs/react';
import mdx from '@astrojs/mdx';
import sitemap from '@astrojs/sitemap';
import { fluxGrammar } from './src/flux-grammar.mjs';

// Flux documentation site.
//
// Single English site (dropped the es/fr locales and the ADR ingest). The ADRs
// live in the repo under docs/adr and are linked from the site rather than
// copied in. Authoring is plain English under src/content/docs.
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
      // Manual sidebar groups keep the most useful entry points on top and let
      // the directory contents autogenerate beneath each header.
      sidebar: [
        {
          label: 'Get started',
          items: [
            { label: 'Introduction', link: '/' },
            { label: 'Quickstart', link: '/guides/quickstart/' },
            { label: 'The Counter example', link: '/guides/counter-example/' },
          ],
        },
        {
          label: 'Concepts',
          items: [{ autogenerate: { directory: 'concepts' } }],
        },
        {
          label: 'Guides',
          items: [{ autogenerate: { directory: 'guides' } }],
        },
        {
          label: 'Reference',
          items: [{ autogenerate: { directory: 'reference' } }],
        },
        {
          label: 'Architecture decisions',
          items: [
            {
              label: 'Read the ADRs (in-repo)',
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
