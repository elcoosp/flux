import { defineDocsConfig } from '@elcoosp/docs-toolkit/config';
import mdx from '@astrojs/mdx';
import linksValidator from 'starlight-links-validator';
import imageZoom from 'starlight-image-zoom';
import headingBadges from 'starlight-heading-badges';

import fluxGrammar from './src/flux.tmLanguage.json';


export default await defineDocsConfig({
  llmsTxt: {},
  changelog: { repo: 'elcoosp/flux' },
  site: "https://flux-ui.vercel.app",
  domain: 'flux-language-docs.vercel.app',
  title: "Flux",
  description: "Flux — a write-once UI language for native iOS and Android. Write .flux, hot-reload on device over binary patches, and ship SwiftUI + Jetpack Compose.",
  sidebar: [],
  extraStarlight: {
      expressiveCode: {
      	themes: ['github-light', 'github-dark']
      },
      title: 'Flux',
      description:
        'Flux — a write-once UI language for native iOS and Android. Write .flux, hot-reload on device over binary patches, and ship SwiftUI + Jetpack Compose.',
      favicon: '/favicon.png',
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/elcoosp/flux' },
      ],
      editLink: {
        baseUrl: 'https://github.com/elcoosp/flux/edit/main/website/src/content/docs/'
      },
      // English is the only authored locale today. The locale map is
      // declared so future translations (the upstream site ships es/fr)
      // drop into locale subdirectories without touching the config.
      // See docs/decisions/0004-i18n-scope.md.
      defaultLocale: 'en',
      locales: {
        en: { label: 'English' }
      },
      // Starlight plugins. Pagefind ships with Starlight and stays enabled —
      // no DocSearch application exists, so Pagefind is the search solution.
      plugins: [
        // Release blocker: the build fails on broken internal links.
        linksValidator(),
        imageZoom(),
        headingBadges(),
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
