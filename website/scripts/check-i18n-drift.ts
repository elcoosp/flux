/**
 * check-i18n-drift.ts
 *
 * Fails the build if any default-locale (English, at the content root) doc lacks a
 * counterpart in every translation locale (`es`, `fr`). Content-only pages must
 * ship in all locales so the site never serves an untranslated page behind the
 * language switcher.
 *
 * Starlight i18n layout: with `defaultLocale: 'root'`, English lives directly
 * under `src/content/docs/` and translations live under `src/content/docs/{es,fr}/`.
 * So an English slug `concepts/the-wire` must exist as `es/concepts/the-wire` and
 * `fr/concepts/the-wire`.
 *
 * Exemptions:
 *  - `adr` directory — ADRs are ingested verbatim from the repo and are
 *    English-only source-of-truth documents (the site links out to them).
 *  - `guides/quickstart` — the quickstart is currently English-first; it is
 *    exempt until a translation lands so the build does not break. Remove this
 *    exemption once `es/guides/quickstart` and `fr/guides/quickstart` exist.
 *
 * Run with: `pnpm check:i18n` (invoked during `pnpm build`).
 */
import { readdir } from 'node:fs/promises';
import { join, dirname, relative, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const docsDir = join(__dirname, '..', 'src', 'content', 'docs');

/** Translation locales that must mirror every English page. */
const TRANSLATION_LOCALES = ['es', 'fr'];

/** Top-level directories that are NOT English content (locales + exempt dirs). */
const NON_EN_DIRS = new Set(['es', 'fr', 'adr']);

/** Specific slugs exempt from parity (documented inline above). */
const EXEMPT_SLUGS = new Set(['guides/quickstart']);

/**
 * Verifies that every English doc exists in all translation locales.
 * Returns a map of locale -> missing slug list (empty when clean).
 *
 * `root` overrides the docs directory (used by tests with temp fixtures);
 * defaults to the real `docsDir` when omitted so production usage is unchanged.
 */
export async function findMissingTranslations(
  root: string = docsDir,
): Promise<Record<string, string[]>> {
  const en = await slugSetIn(root, '');
  const result: Record<string, string[]> = {};
  for (const locale of TRANSLATION_LOCALES) {
    const target = await slugSetIn(root, locale);
    const missing: string[] = [];
    for (const slug of en) {
      if (EXEMPT_SLUGS.has(slug)) continue;
      if (!target.has(slug)) missing.push(slug);
    }
    result[locale] = missing;
  }
  return result;
}

/** slugSet anchored at an explicit docs root (so tests can use temp trees). */
async function slugSetIn(root: string, locale: string): Promise<Set<string>> {
  const localeDir = locale ? join(root, locale) : root;
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
        if (!locale && NON_EN_DIRS.has(entry.name)) continue;
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
      `check-i18n-drift: EN/es/fr parity OK (ADR mirror + guides/quickstart exempt).`,
    );
  } catch (err) {
    console.error(err instanceof Error ? err.message : err);
    process.exit(1);
  }
}
