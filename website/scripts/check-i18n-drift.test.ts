/**
 * check-i18n-drift.test.ts
 *
 * Self-contained tests for `findMissingTranslations` (FLUX-030, Testing
 * Decisions). Exercises the parity checker against temporary fixture trees so
 * CI catches a translation silently falling behind English — without depending
 * on any test framework (runs under `tsx`, the only runner installed).
 *
 * Run with: `pnpm test:i18n`
 */
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { findMissingTranslations } from './check-i18n-drift.ts';

let passed = 0;
let failed = 0;

function assert(cond: boolean, message: string): void {
  if (cond) {
    passed += 1;
  } else {
    failed += 1;
    console.error(`  ✗ ${message}`);
  }
}

/** Builds a temp docs root where `en` (root) has `enSlugs` and each locale set. */
function makeTree(enSlugs: string[], esSlugs: string[], frSlugs: string[]): string {
  const root = mkdtempSync(join(tmpdir(), 'i18n-drift-'));
  const write = (base: string, slugs: string[]) => {
    for (const slug of slugs) {
      const full = join(root, base, `${slug}.md`);
      mkdirSync(join(full, '..'), { recursive: true });
      writeFileSync(full, `# ${slug}\n`);
    }
  };
  write('', enSlugs); // English lives at the content root
  write('es', esSlugs);
  write('fr', frSlugs);
  return root;
}

async function main(): Promise<void> {
  // 1. Clean parity: en == es == fr -> no missing translations.
  {
    const root = makeTree(['a', 'b/c'], ['a', 'b/c'], ['a', 'b/c']);
    const result = await findMissingTranslations(root);
    assert(
      result.es.length === 0 && result.fr.length === 0,
      'clean parity reports no missing translations',
    );
    rmSync(root, { recursive: true, force: true });
  }

  // 2. Drift: es lags en (missing `b/c`) -> es reports exactly that slug.
  {
    const root = makeTree(['a', 'b/c'], ['a'], ['a', 'b/c']);
    const result = await findMissingTranslations(root);
    assert(
      result.es.length === 1 && result.es[0] === 'b/c',
      'es lagging en is detected with the right slug',
    );
    assert(result.fr.length === 0, 'fr in parity is not flagged');
    rmSync(root, { recursive: true, force: true });
  }

  // 3. Drift: fr missing an entire top-level page -> detected as that slug.
  //    (We use a non-exempt slug; `guides/quickstart` is exempt in production
  //     and is intentionally honored here too, since the exemption set is global.)
  {
    const root = makeTree(['guides/overview', 'x'], ['guides/overview', 'x'], ['x']);
    const result = await findMissingTranslations(root);
    assert(
      result.fr.length === 1 && result.fr[0] === 'guides/overview',
      'fr missing a non-exempt page is detected with the right slug',
    );
    rmSync(root, { recursive: true, force: true });
  }

  // 4. End-to-end gating (FLUX-092): the CLI exits 1 on a drifted tree and 0
  //    on a synced tree. This is what the required CI step actually enforces,
  //    so we exercise the real script (with I18N_DOCS_ROOT pointed at a temp
  //    fixture) rather than only the library function.
  {
    const { execFileSync } = await import('node:child_process');
    const { join } = await import('node:path');
    const here = new URL('.', import.meta.url).pathname;
    const tsx = join(here, '..', 'node_modules', '.bin', 'tsx');
    const script = join(here, 'check-i18n-drift.ts');

    // 4a. synced tree -> exit 0.
    const synced = makeTree(['a', 'b/c'], ['a', 'b/c'], ['a', 'b/c']);
    let syncedExit = -1;
    try {
      execFileSync(tsx, [script], {
        env: { ...process.env, I18N_DOCS_ROOT: synced },
        stdio: 'ignore',
      });
      syncedExit = 0;
    } catch (e) {
      syncedExit = (e as { status?: number }).status ?? 1;
    }
    assert(syncedExit === 0, 'synced tree exits 0 (CI passes)');
    rmSync(synced, { recursive: true, force: true });

    // 4b. drifted tree (es lags) -> exit 1.
    const drifted = makeTree(['a', 'b/c'], ['a'], ['a', 'b/c']);
    let driftedExit = 0;
    try {
      execFileSync(tsx, [script], {
        env: { ...process.env, I18N_DOCS_ROOT: drifted },
        stdio: 'ignore',
      });
    } catch (e) {
      driftedExit = (e as { status?: number }).status ?? 1;
    }
    assert(driftedExit === 1, 'drifted tree exits 1 (CI fails the build)');
    rmSync(drifted, { recursive: true, force: true });
  }

  console.log(`\ni18n-drift tests: ${passed} passed, ${failed} failed.`);
  if (failed > 0) process.exit(1);
}

main().catch((err) => {
  console.error(err instanceof Error ? err.message : err);
  process.exit(1);
});
