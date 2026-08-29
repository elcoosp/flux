// @ts-check
/// Generates `syntaxes/flux.tmLanguage.json` from the canonical Flux lexer
/// keyword map (`crates/flux-parser/src/lexer.rs`, `keyword_kind`).
///
/// The grammar is GENERATED, never hand-maintained, so it cannot drift from the
/// surface grammar the parser accepts (FLUX-026 / ADR-0029). `scripts/check-grammar.mjs`
/// asserts the round-trip: every keyword string in `keyword_kind` appears in
/// the emitted grammar.
///
// Usage: `node ./src/generate-grammar.mjs` (run from the extension root).

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..", "..");
const lexerPath = resolve(repoRoot, "crates", "flux-parser", "src", "lexer.rs");
const outPath = resolve(here, "..", "syntaxes", "flux.tmLanguage.json");

const BOOLEANS = new Set(["true", "false"]);

/** Extracts every `"word"` keyword literal from the `keyword_kind` match arms. */
function extractKeywords(src) {
  const start = src.indexOf("pub fn keyword_kind");
  if (start < 0) throw new Error(`keyword_kind not found in ${lexerPath}`);
  const braceOpen = src.indexOf("{", start);
  // Find the matching closing brace by scanning depth.
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
  if (end < 0) throw new Error("unterminated keyword_kind block");
  const block = src.slice(braceOpen, end);

  const keywords = [];
  const re = /"([a-zA-Z_][a-zA-Z0-9_]*)"/g;
  let m;
  while ((m = re.exec(block)) !== null) {
    const kw = m[1];
    if (!keywords.includes(kw)) keywords.push(kw);
  }
  return keywords;
}

function buildGrammar(keywords) {
  const decl = keywords.filter((k) => !BOOLEANS.has(k));
  const bools = keywords.filter((k) => BOOLEANS.has(k));
  const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

  return {
    $schema: "https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json",
    name: "Flux",
    scopeName: "source.flux",
    patterns: [
      { include: "#comment" },
      { include: "#string" },
      { include: "#boolean" },
      { include: "#keyword" },
      { include: "#number" },
    ],
    repository: {
      comment: {
        name: "comment.line.double-slash.flux",
        match: "//.*$",
      },
      string: {
        name: "string.quoted.double.flux",
        begin: '"',
        end: '"',
        patterns: [{ name: "constant.character.escape.flux", match: "\\\\." }],
      },
      boolean: {
        name: "constant.language.boolean.flux",
        match: `\\b(?:${bools.map(escape).join("|")})\\b`,
      },
      keyword: {
        name: "keyword.control.flux",
        match: `\\b(?:${decl.map(escape).join("|")})\\b`,
      },
      number: {
        name: "constant.numeric.flux",
        match: "\\b-?\\d+(\\.\\d+)?\\b",
      },
    },
  };
}

function main() {
  const src = readFileSync(lexerPath, "utf8");
  const keywords = extractKeywords(src);
  if (keywords.length === 0) throw new Error("no keywords extracted — parser changed?");
  const grammar = buildGrammar(keywords);
  writeFileSync(outPath, JSON.stringify(grammar, null, 2) + "\n", "utf8");
  process.stdout.write(
    `Wrote ${keywords.length} keywords to ${outPath}\n`,
  );
}

main();
