/**
 * ingest-adrs.ts
 *
 * Copies the Flux repository's ADR markdown files from `docs/adr` (repo
 * root, orchestrator-owned) into `src/content/docs/en/adr` verbatim before
 * each build. ADRs are plain MDX-compatible markdown; the Starlight sidebar
 * autogenerates from the `en/adr` directory, deriving labels from each
 * ADR's frontmatter `title` (single source of truth — no duplication).
 *
 * This script NEVER edits the source ADRs. It is a one-way copy. The ES
 * mirror is authored by hand under `src/content/docs/es/adr` (translation
 * is a human task, not a copy).
 *
 * Run with: `pnpm ingest` (also invoked during `pnpm build`).
 */
import { cp, mkdir, rm, readdir, access, readFile, writeFile } from 'node:fs/promises';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(__dirname, '..', '..');
const sourceDir = join(repoRoot, 'docs', 'adr');
const targetDir = join(__dirname, '..', 'src', 'content', 'docs', 'adr');

/** True when `path` exists and is accessible. */
async function exists(path: string): Promise<boolean> {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

/**
 * Extracts the first Markdown H1 (`# Title`) from an ADR's body. ADRs carry
 * their title as a heading, not frontmatter, so the sidebar label must be
 * derived from it. Returns `undefined` when no H1 is present.
 */
function extractH1(body: string): string | undefined {
  for (const line of body.split('\n')) {
    const match = /^#\s+(.*\S)\s*$/.exec(line);
    if (match) return match[1];
  }
  return undefined;
}

/**
 * Copies an ADR from the repo into the site, injecting a `title` frontmatter
 * field derived from the document's H1. The repo source is never modified; only
 * the site copy gains frontmatter so Starlight's sidebar shows a human title
 * instead of the file slug.
 */
async function ingestAdr(file: string): Promise<void> {
  const sourcePath = join(sourceDir, file);
  const body = await readFile(sourcePath, 'utf8');
  const h1 = extractH1(body);
  const frontmatter = h1 ? `---\ntitle: ${JSON.stringify(h1)}\n---\n\n` : '';
  await writeFile(join(targetDir, file), frontmatter + body);
}

/**
 * Ingests every `ADR-*.md` file from the repo's `docs/adr` into the site's
 * `src/content/docs/adr`. Returns the number of files copied.
 *
 * @throws if the source ADR directory is missing (the repo layout changed).
 */
export async function ingestAdrs(): Promise<number> {
  if (!(await exists(sourceDir))) {
    throw new Error(
      `ADR source directory not found: ${sourceDir}\n` +
        `The orchestrator-owned docs/adr must exist for ingestion.`,
    );
  }

  await rm(targetDir, { recursive: true, force: true });
  await mkdir(targetDir, { recursive: true });

  const entries = (await readdir(sourceDir)).filter(
    (f) => f.startsWith('ADR-') && f.endsWith('.md'),
  );
  if (entries.length === 0) {
    throw new Error(`No ADR-*.md files found in ${sourceDir}`);
  }

  for (const file of entries) {
    await ingestAdr(file);
  }
  return entries.length;
}

// Run when invoked directly (not when imported by tests).
const isMain =
  process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (isMain) {
  try {
    const count = await ingestAdrs();
    console.log(`ingest-adrs: copied ${count} ADR(s) into ${targetDir}`);
  } catch (err) {
    console.error(err instanceof Error ? err.message : err);
    process.exit(1);
  }
}
