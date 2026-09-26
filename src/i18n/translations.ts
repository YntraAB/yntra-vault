import { appMetadata } from '@/lib/appMetadata';
import { DEFAULT_LANGUAGE } from './languages';

export type TranslationDict = Record<string, string>;

/**
 * In-memory cache of loaded language dictionaries.
 */
export const translations: Partial<Record<string, TranslationDict>> = {};

/**
 * Dynamic import loader registry for all supported languages.
 */
const localeLoaders: Record<string, () => Promise<{ [key: string]: unknown }>> = {
  en: () => import('./locales/en'),
  es: () => import('./locales/es'),
  fr: () => import('./locales/fr'),
  de: () => import('./locales/de'),
  it: () => import('./locales/it'),
  pt: () => import('./locales/pt'),
  ru: () => import('./locales/ru'),
  ja: () => import('./locales/ja'),
  ko: () => import('./locales/ko'),
  'zh-CN': () => import('./locales/zh-CN'),
  'zh-TW': () => import('./locales/zh-TW'),
  ar: () => import('./locales/ar'),
  hi: () => import('./locales/hi'),
  nl: () => import('./locales/nl'),
  pl: () => import('./locales/pl'),
  uk: () => import('./locales/uk'),
  sv: () => import('./locales/sv'),
  tr: () => import('./locales/tr'),
  da: () => import('./locales/da'),
  fi: () => import('./locales/fi'),
  no: () => import('./locales/no'),
  cs: () => import('./locales/cs'),
  el: () => import('./locales/el'),
  he: () => import('./locales/he'),
};

const loadingPromises: Partial<Record<string, Promise<TranslationDict>>> = {};

/**
 * Checks if a language dictionary is already cached in memory.
 */
export function isTranslationLoaded(lang: string): boolean {
  return Boolean(translations[lang]);
}

/**
 * Asynchronously loads and caches a language dictionary chunk on demand.
 */
export async function loadTranslation(lang: string): Promise<TranslationDict> {
  const targetLang = localeLoaders[lang] ? lang : DEFAULT_LANGUAGE;

  const cached = translations[targetLang];
  if (cached) {
    return cached;
  }

  const existingPromise = loadingPromises[targetLang];
  if (existingPromise) {
    return existingPromise;
  }

  const loader = localeLoaders[targetLang] || localeLoaders[DEFAULT_LANGUAGE];
  const promise = loader()
    .then((mod) => {
      // Extract dictionary from default export or the first named export
      const dict = (('default' in mod && mod.default ? mod.default : Object.values(mod)[0]) || {}) as TranslationDict;
      translations[targetLang] = dict;
      delete loadingPromises[targetLang];

      // If non-default language, ensure the fallback language is also loaded in background
      if (targetLang !== DEFAULT_LANGUAGE && !translations[DEFAULT_LANGUAGE] && !loadingPromises[DEFAULT_LANGUAGE]) {
        loadTranslation(DEFAULT_LANGUAGE).catch(() => {});
      }

      return dict;
    })
    .catch((err) => {
      delete loadingPromises[targetLang];
      console.error(`Failed to load translation chunk for "${targetLang}":`, err);
      return translations[DEFAULT_LANGUAGE] || {};
    });

  loadingPromises[targetLang] = promise;
  return promise;
}

/**
 * Synchronously retrieves initial language from persisted settings.
 */
function getInitialLanguage(): string {
  try {
    const saved = typeof localStorage !== 'undefined' ? appMetadata.getItem('yntra-vault-settings') : null;
    if (saved) {
      const parsed = JSON.parse(saved);
      if (parsed?.language && localeLoaders[parsed.language]) {
        return parsed.language;
      }
    }
  } catch {
    // Ignore and fallback to default
  }
  return DEFAULT_LANGUAGE;
}

// Eagerly prefetch active language and default fallback immediately upon module evaluation
const initialLanguage = getInitialLanguage();
loadTranslation(initialLanguage).catch(() => {});
if (initialLanguage !== DEFAULT_LANGUAGE) {
  loadTranslation(DEFAULT_LANGUAGE).catch(() => {});
}

/**
 * Returns translated string for key with parameter interpolation.
 * Falls back to English dictionary if key is missing in target language,
 * and returns the key itself if missing everywhere.
 */
export function getTranslation(lang: string, key: string, params?: Record<string, string | number>): string {
  const dict = translations[lang] || translations[DEFAULT_LANGUAGE];
  const text = dict?.[key] ?? translations[DEFAULT_LANGUAGE]?.[key] ?? key;
  // Replace once: user values may contain dollar signs or other placeholders.
  return params ? text.replace(/\{\s*(\w+)\s*\}/g, (match, name: string) =>
    Object.prototype.hasOwnProperty.call(params, name) ? String(params[name]) : match
  ) : text;
}
