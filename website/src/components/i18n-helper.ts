/**
 * i18n-helper.ts
 *
 * Thin wrapper around Starlight's `src/content/i18n/en.json` dictionary for use
 * inside `.astro` components (which cannot use the React `useTranslations` hook).
 * Components read `Astro.currentLocale` and call `getTranslation(locale)`.
 *
 * The site is currently single-locale (English); the helper falls back to English
 * for any unknown locale.
 */
import en from '../content/i18n/en.json';

export type TranslationKey = keyof typeof en;

const dictionaries: Record<string, Record<string, string>> = {
  en,
};

/** Returns the UI string dictionary for a locale, falling back to English. */
export function getTranslation(locale: string): Record<string, string> {
  return dictionaries[locale] ?? dictionaries.en;
}
