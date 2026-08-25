/**
 * i18n-helper.ts
 *
 * Thin wrapper around Starlight's `src/content/i18n/{en,es}.json` dictionaries
 * for use inside `.astro` components (which cannot use the React `useTranslations`
 * hook). Components read `Astro.currentLocale` and call `getTranslation(locale)`.
 *
 * This is the custom-UI-string tier from the design: chrome strings (built-in
 * Starlight dicts) are free; these are the project-specific strings (trace-player
 * buttons, frame-inspector headers, status badge).
 */
import en from '../content/i18n/en.json';
import es from '../content/i18n/es.json';

export type TranslationKey = keyof typeof en;

const dictionaries: Record<string, Record<string, string>> = {
  en,
  es,
};

/** Returns the UI string dictionary for a locale, falling back to English. */
export function getTranslation(locale: string): Record<string, string> {
  return dictionaries[locale] ?? dictionaries.en;
}
