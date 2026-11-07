import { defineCollection, z } from 'astro:content';
import { docsLoader } from '@astrojs/starlight/loaders';
import { docsSchema } from '@astrojs/starlight/schema';

// Every docs page declares its Diátaxis type in frontmatter. The build fails
// if a page omits it, which keeps the information architecture honest:
// one page teaches one thing, and the sidebar groups match the type.
export const collections = {
  docs: defineCollection({
    loader: docsLoader(),
    schema: docsSchema({
      extend: z.object({
        type: z.enum(['tutorial', 'how-to', 'reference', 'explanation']),
      }),
    }),
  }),
};
