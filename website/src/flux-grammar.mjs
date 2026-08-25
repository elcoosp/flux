// Minimal TextMate-style grammar for the Flux language, so code blocks tagged
// ```flux``` highlight instead of falling back to plain text. This is a
// lightweight regex grammar (comments, strings, keywords, numbers, booleans,
// built-in types) — enough for documentation readability; it is not a full
// parser. Consumed by Starlight's Expressive Code (shiki) via the `langs`
// option in astro.config.mjs. Shape follows shiki's `LanguageRegistration`.
/** @type {import('@expressive-code/core').GraphicsLang | any} */
export const fluxGrammar = {
  name: 'Flux',
  scopeName: 'source.flux',
  fileTypes: ['flux'],
  aliases: ['flux'],
  patterns: [
    { include: '#comment' },
    { include: '#string' },
    { include: '#keyword' },
    { include: '#boolean' },
    { include: '#type' },
    { include: '#number' },
  ],
  repository: {
    comment: {
      match: '#.*$',
      name: 'comment.line.number-sign.flux',
    },
    string: {
      match: '"(?:[^"\\\\]|\\\\.)*"',
      name: 'string.quoted.double.flux',
    },
    keyword: {
      match:
        '\\b(component|state|let|derived|effect|fn|trait|type|match|if|else|return|for|while|onMount|onCleanup|batch|in|import|export|struct|enum|impl|pub|where)\\b',
      name: 'keyword.control.flux',
    },
    boolean: {
      match: '\\b(true|false|null|None)\\b',
      name: 'constant.language.flux',
    },
    type: {
      match:
        '\\b(Int|Float|String|Bool|List|Map|Option|Ref|Handler|Signal|Color|Alignment|Overflow|KeyboardType|WebSocket)\\b',
      name: 'support.type.flux',
    },
    number: {
      match: '\\b\\d+(\\.\\d+)?\\b',
      name: 'constant.numeric.flux',
    },
  },
};
