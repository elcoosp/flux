// @ts-check
/// CI round-trip check (FLUX-026 testing decision): every keyword string in
/// `flux-parser`'s `keyword_kind` must be present in the generated
/// `syntaxes/flux.tmLanguage.json`. Fails (non-zero exit) if any keyword is
/// missing, forcing the grammar to be regenerated rather than hand-edited.
///
// Usage: `node ./scripts/check-grammar.mjs`.

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..", "..");
const lexerPath = resolve(repoRoot, "crates", "flux-parser", "src", "lexer.rs");
const grammarPath = resolve(here, "..", "syntaxes", "flux.tmLanguage.json");

function extractKeywords(src) {
  const start = src.indexOf("pub fn keyword_kind");
  const braceOpen = src.indexOf("{", start);
  let depth = 0;
  let end = -1;
  for (let i = braceOpen; i < src.length; i++) {
    if (src[i] === "{") depth++;
    else if (src[i] === "}") {
      depth--;
      if (depth === 0) {
        end = i;
        break;
      }
    }
  }
  const block = src.slice(braceOpen, end);
  const keywords = [];
  const re = /"([a-zA-Z_][a-zA-Z0-9_]*)"/g;
  let m;
  while ((m = re.exec(block)) !== null) {
    if (!keywords.includes(m[1])) keywords.push(m[1]);
  }
  return keywords;
}

function main() {
  const keywords = extractKeywords(readFileSync(lexerPath, "utf8"));
  const grammarText = readFileSync(grammarPath, "utf8");

  const missing = keywords.filter((k) => !grammarText.includes(`"${k}"`) && !grammarText.includes(k));
  if (missing.length > 0) {
    process.stderr.write(
      `Grammar is out of sync with the lexer. Missing keywords: ${missing.join(", ")}\n` +
        `Run \`npm run generate-grammar\` in editors/vscode and commit the result.\n`,
    );
    process.exit(1);
  }
  process.stdout.write(`Grammar round-trip OK: ${keywords.length} keywords covered.\n`);
}

main();
