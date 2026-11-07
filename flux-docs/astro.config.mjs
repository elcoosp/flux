// @ts-check
import { defineConfig } from 'astro/config';
import mdx from '@astrojs/mdx';
import starlight from '@astrojs/starlight';
import linksValidator from 'starlight-links-validator';
import imageZoom from 'starlight-image-zoom';
import headingBadges from 'starlight-heading-badges';

// The Flux TextMate grammar (editors/vscode in the main repository) registers
// the `flux` language with Shiki so every ```flux code block gets real
// syntax highlighting through the same markdown pipeline.
import fluxGrammar from './src/flux.tmLanguage.json';

// Flux documentation site.
//
// Information architecture follows Diátaxis: one page teaches one thing, and
// each page declares its type in frontmatter (`type: tutorial | how-to |
// reference | explanation`) — the schema lives in src/content.config.ts.
// Content lives under src/content/docs/{tutorials,guides,reference,explanation}.
//
// Version baseline (verified 2026-11-07): Astro 7.4.7, @astrojs/starlight
// 0.42.10, TypeScript pinned to 6.0.3 (`astro check` does not support
// TypeScript 7 — see docs/decisions/0001-typescript-version.md).
export default defineConfig({
  // llms.txt / llms-full.txt need absolute URLs, and this mirrors the
  // canonical deployment URL already declared by the repository's own
  // website/ config. See docs/decisions/0005-site-url-and-llms-txt.md.
  site: 'https://flux-lang.dev',
  markdown: {
    shikiConfig: {
      langs: [fluxGrammar],
    },
  },
  integrations: [
    starlight({
      title: 'Flux',
      description:
        'Flux — a write-once UI language for native iOS and Android. Write .flux, hot-reload on device over binary patches, and ship SwiftUI + Jetpack Compose.',
      logo: {
        light: './src/assets/logo-light.svg',
        dark: './src/assets/logo-dark.svg',
      },
      customCss: ['./src/styles/custom.css'],
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/elcoosp/flux' },
      ],
      editLink: {
        baseUrl: 'https://github.com/elcoosp/flux/edit/main/website/src/content/docs/',
      },
      // English is the only authored locale today. The locale map is
      // declared so future translations (the upstream site ships es/fr)
      // drop into locale subdirectories without touching the config.
      // See docs/decisions/0004-i18n-scope.md.
      defaultLocale: 'en',
      locales: {
        en: { label: 'English' },
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
          items: [{ autogenerate: { directory: 'tutorials' } }],
        },
        {
          label: 'How-to guides',
          items: [{ autogenerate: { directory: 'guides' } }],
        },
        {
          label: 'Reference',
          collapsed: true,
          items: [{ autogenerate: { directory: 'reference' } }],
        },
        {
          label: 'Explanation',
          collapsed: true,
          items: [{ autogenerate: { directory: 'explanation' } }],
        },
      ],
    }),
    mdx(),
  ],
});
