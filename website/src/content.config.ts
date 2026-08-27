import { defineCollection } from 'astro:content';
import { docsLoader } from '@astrojs/starlight/loaders';
import { docsSchema } from '@astrojs/starlight/schema';

// Flux docs schema. ADRs live in the repo under `docs/adr` and are linked from
// the site (not ingested as pages), so the standard Starlight `docsSchema()` is
// used as-is. `title` is supplied via frontmatter on every page.
const relaxedDocsSchema = docsSchema();

export const collections = {
  docs: defineCollection({ loader: docsLoader(), schema: relaxedDocsSchema }),
};
