// Generates dist/llms.txt and dist/llms-full.txt from the docs content
// collection (src/content/docs). llms.txt follows the llms.txt convention:
// a curated index for AI agents, with absolute URLs and one line per page;
// llms-full.txt carries the full text of every page.
//
// The script runs after `astro build` (see package.json) so dist/ is already
// populated; it never mutates src/.
import { readdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const docsDir = join(root, 'src', 'content', 'docs');
const distDir = join(root, 'dist');

// Site URL: read from astro.config.mjs (single source of truth).
const configText = readFileSync(join(root, 'astro.config.mjs'), 'utf8');
const siteMatch = configText.match(/site:\s*['"]([^'"]+)['"]/);
if (!siteMatch) {
  console.error('generate-llms: could not read `site` from astro.config.mjs');
  process.exit(1);
}
const site = siteMatch[1].replace(/\/$/, '');

const TITLE_RE = /^title:\s*(?:["'](.+?)["']|(.+?))\s*$/m;
const DESC_RE = /^description:\s*(?:["'](.+?)["']|(.+?))\s*$/m;
const ORDER_RE = /order:\s*(\d+)/;

function stripFrontmatter(text) {
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/);
  if (!m) return { fm: '', body: text };
  return { fm: m[1], body: text.slice(m[0].length) };
}

function pageUrl(relFile) {
  const noExt = relFile.replace(/\.(md|mdx)$/, '');
  if (noExt === 'index') return '/';
  return '/' + noExt + '/';
}

function collect(dir, prefix = '') {
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.name.startsWith('.')) continue;
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...collect(full, prefix + entry.name + '/'));
    } else if (/\.(md|mdx)$/.test(entry.name)) {
      const rel = prefix + entry.name;
      const raw = readFileSync(full, 'utf8');
      const { fm, body } = stripFrontmatter(raw);
      const title = ((fm.match(TITLE_RE) || [])[1] || (fm.match(TITLE_RE) || [])[2]) || entry.name;
      const description = ((fm.match(DESC_RE) || [])[1] || (fm.match(DESC_RE) || [])[2]) || '';
      const order = Number((fm.match(ORDER_RE) || [])[1] || 99);
      // MDX: drop import statements so llms-full.txt stays pure prose/markdown.
      const cleaned = body.replace(/^import\s+.*?;?\s*$/gm, '').replace(/\n{3,}/g, '\n\n').trim();
      out.push({ rel, title, description, order, url: pageUrl(rel), body: cleaned });
    }
  }
  return out;
}

const all = collect(docsDir);

// Order sections the way the sidebar presents them; pages sort by frontmatter order.
const SECTION_ORDER = ['index', 'tutorials/', 'guides/', 'reference/', 'explanation/'];
function sectionKey(p) {
  const i = SECTION_ORDER.findIndex((s) => (s === 'index' ? p.rel === 'index.mdx' : p.rel.startsWith(s)));
  return i === -1 ? SECTION_ORDER.length : i;
}
const sorted = all.sort(
  (a, b) => sectionKey(a) - sectionKey(b) || a.order - b.order || a.title.localeCompare(b.title),
);

const SECTION_TITLES = ['Overview', 'Tutorials', 'How-to guides', 'Reference', 'Explanation'];

let llms = `# Flux\n\n${siteMatch[1]} — documentation for Flux, a write-once UI language for native iOS and Android. Write .flux, hot-reload on device over binary patches, and ship SwiftUI + Jetpack Compose from the same source.\n\n`;
let section = -1;
for (const p of sorted) {
  const key = sectionKey(p);
  if (key !== section) {
    section = key;
    llms += `\n## ${SECTION_TITLES[key] ?? 'Pages'}\n\n`;
  }
  llms += `- [${p.title}](${site}${p.url}): ${p.description}\n`;
}
llms += `\n## Source repository\n\n- [elcoosp/flux](https://github.com/elcoosp/flux): the Flux language, CLI, dev server, hosts, and specification.\n`;

let full = `# Flux — full documentation\n\n${llms.split('\n').slice(2).join('\n')}\n`;
for (const p of sorted) {
  full += `\n---\n\n# ${p.title}\n\nURL: ${site}${p.url}\n\n${p.body}\n`;
}

if (!existsSync(distDir)) {
  console.error('generate-llms: dist/ not found — run `astro build` first');
  process.exit(1);
}
writeFileSync(join(distDir, 'llms.txt'), llms);
writeFileSync(join(distDir, 'llms-full.txt'), full);
console.log(`generate-llms: wrote llms.txt (${llms.length} bytes) and llms-full.txt (${full.length} bytes) with ${sorted.length} pages`);
