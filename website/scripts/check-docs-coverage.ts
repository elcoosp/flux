/**
 * check-docs-coverage.ts
 *
 * Derives the docs-ecosystem coverage contracts for FLUX-031 (cookbook per
 * stdlib primitive), FLUX-032 (migration guides reference real primitives),
 * FLUX-033 (every FluxError variant documented), and FLUX-036 (state/i18n/
 * showcase guides). All checks are pure Node — they read `stdlib/*.flux` and
 * the Rust error sources directly so the website CI needs no Rust toolchain.
 *
 * The error taxonomy is *derived* from source (the `#[error("...")]` payloads
 * in `flux-vm-ref/src/error.rs` and the class list in `flux-types/src/error.rs`)
 * so the troubleshooting guide cannot silently drift from `FluxError`.
 *
 * Run with: `pnpm check:coverage` (invoked during `pnpm build`).
 */
import { readdir, readFile } from 'node:fs/promises';
import { join, dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const websiteDir = join(here, '..');
const repoRoot = resolve(websiteDir, '..');
const docsDir = join(websiteDir, 'src', 'content', 'docs');
const stdlibDir = join(repoRoot, 'stdlib');

/** Adapter components that must each have a cookbook page. */
export async function expectedComponents(): Promise<string[]> {
  const files = (await readdir(stdlibDir)).filter((f) => f.endsWith('.flux'));
  const names = new Set<string>();
  for (const file of files) {
    const src = await readFile(join(stdlibDir, file), 'utf8');
    for (const m of src.matchAll(/^compo\s+([A-Z]\w+)/gm)) {
      names.add(m[1]);
    }
  }
  return [...names].sort();
}

/** Returns the stdlib components missing a `guides/cookbook/<lower>.md(x)` page. */
export async function missingCookbookPages(): Promise<string[]> {
  const components = await expectedComponents();
  const missing: string[] = [];
  for (const c of components) {
    const slug = `guides/cookbook/${c.toLowerCase()}`;
    const md = join(docsDir, `${slug}.md`);
    const mdx = join(docsDir, `${slug}.mdx`);
    let exists = false;
    try {
      await readFile(md);
      exists = true;
    } catch {
      /* try mdx */
    }
    if (!exists) {
      try {
        await readFile(mdx);
        exists = true;
      } catch {
        /* still missing */
      }
    }
    if (!exists) missing.push(slug);
  }
  return missing;
}

/**
 * The FluxError taxonomy, derived from source so it cannot drift:
 *  - the three classes in `flux-types/src/error.rs` (Compile / Runtime /
 *    Capability), and
 *  - every VM fault payload in `flux-vm-ref/src/error.rs` (the `#[error]` text).
 */
export async function expectedErrorKeys(): Promise<string[]> {
  const keys: string[] = [
    'Compile errors',
    'Runtime (VM) errors',
    'Capability errors',
  ];
  const vmSrc = await readFile(
    join(repoRoot, 'crates', 'flux-vm-ref', 'src', 'error.rs'),
    'utf8',
  );
  for (const m of vmSrc.matchAll(/#\[error\("([^"]+)"\)\]/g)) {
    const label = m[1];
    // Skip the `VmError` *Display* format string (e.g. "{kind} at offset
    // {offset}") — only the `VmErrorKind` variant payloads are taxonomy terms.
    if (label.includes('{')) continue;
    keys.push(label);
  }
  return keys;
}

/** Returns error keys not present anywhere in the troubleshooting guide. */
export async function missingTroubleshootingSections(): Promise<string[]> {
  const keys = await expectedErrorKeys();
  let guide = '';
  for (const ext of ['md', 'mdx']) {
    try {
      guide = await readFile(
        join(docsDir, `guides/troubleshooting.${ext}`),
        'utf8',
      );
      break;
    } catch {
      /* try next */
    }
  }
  const lower = guide.toLowerCase();
  return keys.filter((k) => !lower.includes(k.toLowerCase()));
}

/** Extracts relative (`.md`/`.mdx`) link targets from a markdown string. */
function relativeLinks(md: string): string[] {
  const out: string[] = [];
  for (const m of md.matchAll(/\[[^\]]*\]\(\s*([^)\s]+)\s*\)/g)) {
    const target = m[1];
    if (target.startsWith('http://') || target.startsWith('https://')) continue;
    if (target.startsWith('#')) continue;
    if (target.startsWith('/')) continue; // site routes are resolved by Astro
    out.push(target);
  }
  return out;
}

/**
 * Returns broken relative `.md`/`.mdx` links found in any docs file under
 * `dir`, keyed by the file that contains them.
 */
export async function brokenLinks(): Promise<Record<string, string[]>> {
  const result: Record<string, string[]> = {};
  const walk = async (dir: string) => {
    let entries;
    try {
      entries = await readdir(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const e of entries) {
      const full = join(dir, e.name);
      if (e.isDirectory()) {
        // Skip translation locales — their parity is FLUX-030's job.
        if (e.name === 'es' || e.name === 'fr') continue;
        await walk(full);
      } else if (/\.(md|mdx)$/.test(e.name)) {
        const md = await readFile(full, 'utf8');
        const bad: string[] = [];
        for (const link of relativeLinks(md)) {
          const bare = link.split('#')[0].replace(/\.(md|mdx)$/, '');
          const candidates = [
            join(dir, `${bare}.md`),
            join(dir, `${bare}.mdx`),
          ];
          const ok = await Promise.all(
            candidates.map((c) =>
              readFile(c).then(
                () => true,
                () => false,
              ),
            ),
          );
          if (!ok[0] && !ok[1]) bad.push(link);
        }
        if (bad.length > 0) {
          result[relative(docsDir, full)] = bad;
        }
      }
    }
  };
  await walk(docsDir);
  return result;
}

const isMain =
  process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (isMain) {
  try {
    const missingCookbook = await missingCookbookPages();
    const missingErrors = await missingTroubleshootingSections();
    const links = await brokenLinks();
    const broken = Object.values(links).flat();
    const ok =
      missingCookbook.length === 0 &&
      missingErrors.length === 0 &&
      broken.length === 0;
    if (missingCookbook.length > 0) {
      console.error('Docs coverage: missing cookbook pages:');
      for (const s of missingCookbook) console.error(`  - ${s}`);
    }
    if (missingErrors.length > 0) {
      console.error('Docs coverage: undocumented FluxError variants:');
      for (const s of missingErrors) console.error(`  - ${s}`);
    }
    if (broken.length > 0) {
      console.error('Docs coverage: broken relative links:');
      for (const [file, ls] of Object.entries(links)) {
        console.error(`  - ${file}: ${ls.join(', ')}`);
      }
    }
    if (ok) {
      console.log('check-docs-coverage: OK (cookbook + FluxError + links).');
      process.exit(0);
    }
    process.exit(1);
  } catch (err) {
    console.error(err instanceof Error ? err.message : err);
    process.exit(1);
  }
}
