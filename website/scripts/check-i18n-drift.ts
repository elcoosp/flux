/**
 * check-i18n-drift.ts
 *
 * Fails the build if any English doc slug under `src/content/docs/en` lacks a
 * counterpart in every other locale (currently `es`, `fr`). Content-only pages
 * must ship in all locales so the site never serves an untranslated page behind
 * the language switcher.
 *
 * The ADR directory (`en/adr`) is an exception: ADRs are ingested verbatim
 * from the repo and are English-only source-of-truth documents. Other-locale
 * readers get the English ADR with Starlight's built-in untranslated-content
 * notice.
 *
 * Run with: `pnpm check:i18n` (also invoked during `pnpm build` after ingest).
 */
import { readdir } from 'node:fs/promises';
import { join, dirname, relative, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const docsDir = join(__dirname, '..', 'src', 'content', 'docs');

/** Locales that must mirror every translatable EN page. */
const TRANSLATION_LOCALES = ['es', 'fr'];

/** Directories excluded from the parity requirement (English-only by design). */
const EXEMPT_DIRS = new Set(['adr']);

/** Returns the slug-relative set of doc paths (without extension) for a locale. */
async function slugSet(locale: string): Promise<Set<string>> {
  const localeDir = join(docsDir, locale);
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

/**
 * Verifies that every translatable EN doc exists in all translation locales.
 * Returns a map of locale -> missing slug list (empty when clean).
 */
export async function findMissingTranslations(): Promise<Record<string, string[]>> {
  const en = await slugSet('en');
  const result: Record<string, string[]> = {};
  for (const locale of TRANSLATION_LOCALES) {
    const target = await slugSet(locale);
    const missing: string[] = [];
    for (const slug of en) {
      const topSegment = slug.split('/')[0] ?? '';
      if (EXEMPT_DIRS.has(topSegment)) continue;
      if (!target.has(slug)) missing.push(slug);
    }
    result[locale] = missing;
  }
  return result;
}

const isMain =
  process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (isMain) {
  try {
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
      console.error(
        '\nTranslate the missing pages or add them under src/content/docs/<locale>.',
      );
      process.exit(1);
    }
    console.log(
      `check-i18n-drift: EN/${TRANSLATION_LOCALES.join('/')} parity OK (ADR mirror exempt).`,
    );
  } catch (err) {
    console.error(err instanceof Error ? err.message : err);
    process.exit(1);
  }
}
