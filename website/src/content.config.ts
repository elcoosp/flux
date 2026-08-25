import { defineCollection } from 'astro:content';
import { z } from 'astro/zod';
import { docsLoader } from '@astrojs/starlight/loaders';
import { docsSchema } from '@astrojs/starlight/schema';

// The Flux ADRs are ingested verbatim from `docs/adr` (repo source of truth) and
// use an H1 `# Title` heading instead of Starlight frontmatter `title`. Starlight's
// built-in `docsSchema()` makes `title` required, which would reject every ADR.
// We relax it via the schema `extend` option: `title` becomes optional. Starlight
// surfaces the H1 as the page heading. Everything else from the Starlight schema
// (sidebar autogenerate, i18n, etc.) is preserved.
const relaxedDocsSchema = docsSchema({
  extend: z.object({ title: z.string().optional() }),
});

export const collections = {
  docs: defineCollection({ loader: docsLoader(), schema: relaxedDocsSchema }),
};
