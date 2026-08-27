/**
 * check-i18n-drift.ts
 *
 * The site is currently authored in a single (English) locale, so there is no
 * translation parity to enforce. This script is kept as a no-op safety net: if a
 * future locale is added under `src/content/docs/<locale>`, re-enable the parity
 * check in `findMissingTranslations()` and wire it into `pnpm build`.
 *
 * Run with: `pnpm check:i18n` (invoked during `pnpm build` if configured).
 */
import { readdir } from 'node:fs/promises';
import { join, dirname, relative, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const docsDir = join(__dirname, '..', 'src', 'content', 'docs');

/** Locales that must mirror every EN page (currently none — single locale). */
const TRANSLATION_LOCALES: string[] = [];

/** Returns the slug-relative set of doc paths (without extension) for a locale. */
async function slugSet(locale: string): Promise<Set<string>> {
  const localeDir = locale ? join(docsDir, locale) : docsDir;
  const slugs = new Set<string>();
  const walk = async (dir: string) => {
    let entries: import('node:fs').Dirent[];
    try {
      entries = await readdir(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) {
        await walk(full);
      } else if (/\.(md|mdx)$/.test(entry.name)) {
        const rel = relative(localeDir, full).replace(/\\/g, '/');
        slugs.add(rel.slice(0, -extname(rel).length));
      }
    }
  };
  await walk(localeDir);
  return slugs;
}

/** Verifies that every translatable EN doc exists in all translation locales. */
export async function findMissingTranslations(): Promise<Record<string, string[]>> {
  const en = await slugSet('');
  const result: Record<string, string[]> = {};
  for (const locale of TRANSLATION_LOCALES) {
    const target = await slugSet(locale);
    const missing = [...en].filter((slug) => !target.has(slug));
    result[locale] = missing;
  }
  return result;
}

const isMain =
  process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (isMain) {
  const missingByLocale = await findMissingTranslations();
  const localesWithGaps = Object.entries(missingByLocale).filter(
    ([, slugs]) => slugs.length > 0,
  );
  if (localesWithGaps.length > 0) {
    console.error('i18n drift detected — missing translations:');
    for (const [locale, slugs] of localesWithGaps) {
      console.error(`  [${locale}] ${slugs.length} missing:`);
      for (const slug of slugs) console.error(`    - ${locale}/${slug}`);
    }
    process.exit(1);
  }
  console.log('check-i18n-drift: single-locale site — nothing to verify.');
}
