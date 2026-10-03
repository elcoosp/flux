/**
 * i18n-helper.ts
 *
 * Thin wrapper around Starlight's `src/content/i18n/{en,es,fr}.json` dictionaries
 * for use inside `.astro` components (which cannot use the React `useTranslations`
 * hook). Components read `Astro.currentLocale` and call `getTranslation(locale)`.
 *
 * This is the custom-UI-string tier: chrome strings (built-in Starlight dicts) are
 * free; these are the project-specific strings (trace-player buttons, frame-inspector
 * headers, status badge).
 */
import en from '../content/i18n/en.json';
import es from '../content/i18n/es.json';
import fr from '../content/i18n/fr.json';

export type TranslationKey = keyof typeof en;

const dictionaries: Record<string, Record<string, string>> = {
  en,
  es,
  fr,
};

/** Returns the UI string dictionary for a locale, falling back to English. */
export function getTranslation(locale: string): Record<string, string> {
  return dictionaries[locale] ?? dictionaries.en;
}

/**
 * The keys `DispatchTracePlayer` reads on its `i18n` prop. Every field is
 * required — the helper below fails loudly at compile time if a translation
 * is missing a key rather than silently rendering an empty label.
 */
export interface TracePlayerStrings {
  title: string;
  intro: string;
  webImpossible: string;
  sourcePane: string;
  wirePane: string;
  treePane: string;
  nativePane: string;
  nativePending: string;
  tapCounter: string;
  tapHint: string;
  step: string;
  phase: string;
  signals: string;
  dirty: string;
  updated: string;
  built: string;
}

/**
 * Strips the `tracePlayer.` prefix from the flat locale dictionary so the
 * React island can read `i18n.title` / `i18n.step` (the pre-existing prop
 * shape). Without this the player received `tracePlayer.title`-prefixed keys
 * and every label rendered `undefined` in en/es/fr — the audit flagged this
 * as a HIGH bug.
 */
export function getTracePlayerStrings(locale: string): TracePlayerStrings {
  const dict = getTranslation(locale);
  const out: Record<string, string> = {};
  const prefix = "tracePlayer.";
  for (const [k, v] of Object.entries(dict)) {
    if (k.startsWith(prefix)) {
      out[k.slice(prefix.length)] = v;
    }
  }
  // Build with the required fields so TS enforces the contract at the call
  // site; a missing key becomes `undefined` here only if the JSON itself is
  // incomplete (which the i18n drift check catches in CI).
  return {
    title: out.title ?? "",
    intro: out.intro ?? "",
    webImpossible: out.webImpossible ?? "",
    sourcePane: out.sourcePane ?? "",
    wirePane: out.wirePane ?? "",
    treePane: out.treePane ?? "",
    nativePane: out.nativePane ?? "",
    nativePending: out.nativePending ?? "",
    tapCounter: out.tapCounter ?? "",
    tapHint: out.tapHint ?? "",
    step: out.step ?? "",
    phase: out.phase ?? "",
    signals: out.signals ?? "",
    dirty: out.dirty ?? "",
    updated: out.updated ?? "",
    built: out.built ?? "",
  };
}
