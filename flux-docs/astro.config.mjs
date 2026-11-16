import { defineDocsConfig } from '@elcoosp/docs-toolkit/config';
import mdx from '@astrojs/mdx';

import fluxGrammar from './src/flux.tmLanguage.json';

export default await defineDocsConfig({
  topics: true,
  llmsTxt: {},
  site: "https://flux-rs-docs.vercel.app",
  domain: 'flux-language-docs.vercel.app',
  title: "Flux",
  description: "Flux — a write-once UI language for native iOS and Android. Write .flux, hot-reload on device over binary patches, and ship SwiftUI + Jetpack Compose.",
  extraStarlight: {
      title: 'Flux',
      description:
        'Flux — a write-once UI language for native iOS and Android. Write .flux, hot-reload on device over binary patches, and ship SwiftUI + Jetpack Compose.',
      social: [
        { icon: 'github', label: 'GitHub', translations: { fr: "GitHub" }, href: 'https://github.com/elcoosp/flux' },
      ],
      editLink: {
        baseUrl: 'https://github.com/elcoosp/flux/edit/main/website/src/content/docs/'
      },
      // English is the only authored locale today. The locale map is
      // declared so future translations (the upstream site ships es/fr)
      // drop into locale subdirectories without touching the config.
      // See docs/decisions/0004-i18n-scope.md.

      // Starlight plugins. Pagefind ships with Starlight and stays enabled —
      // no DocSearch application exists, so Pagefind is the search solution.
      plugins: [
        // Release blocker: the build fails on broken internal links.
      ],
      sidebar: [
        { label: 'Overview', link: '/' },
        {
          label: 'Tutorials',
          items: [{ autogenerate: { directory: 'tutorials' } }]
        },
        {
          label: 'How-to guides',
          items: [{ autogenerate: { directory: 'guides' } }]
        },
        {
          label: 'Reference',
          collapsed: true,
          items: [{ autogenerate: { directory: 'reference' } }]
        },
        {
          label: 'Explanation',
          collapsed: true,
          items: [{ autogenerate: { directory: 'explanation' } }]
        },
      ]
    },
});
