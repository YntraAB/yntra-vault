import { en } from './locales/en';
import { es } from './locales/es';
import { fr } from './locales/fr';
import { de } from './locales/de';
import { it } from './locales/it';
import { pt } from './locales/pt';
import { ru } from './locales/ru';
import { ja } from './locales/ja';
import { ko } from './locales/ko';
import { zhCN } from './locales/zh-CN';
import { zhTW } from './locales/zh-TW';
import { ar } from './locales/ar';
import { hi } from './locales/hi';
import { nl } from './locales/nl';
import { pl } from './locales/pl';
import { uk } from './locales/uk';
import { sv } from './locales/sv';
import { tr } from './locales/tr';
import { da } from './locales/da';
import { fi } from './locales/fi';
import { no } from './locales/no';
import { cs } from './locales/cs';
import { el } from './locales/el';
import { he } from './locales/he';

export type TranslationDict = Record<string, string>;

export const translations: Record<string, TranslationDict> = {
  en,
  es,
  fr,
  de,
  it,
  pt,
  ru,
  ja,
  ko,
  'zh-CN': zhCN,
  'zh-TW': zhTW,
  ar,
  hi,
  nl,
  pl,
  uk,
  sv,
  tr,
  da,
  fi,
  no,
  cs,
  el,
  he,
};

/**
 * Returns translated string for key with parameter interpolation.
 * Falls back to English dictionary if key is missing in target language,
 * and returns the key itself if missing everywhere.
 */
export function getTranslation(lang: string, key: string, params?: Record<string, string | number>): string {
  const dict = translations[lang] || translations['en'];
  let text = dict[key] || translations['en']?.[key] || key;

  if (params) {
    Object.entries(params).forEach(([paramKey, value]) => {
      text = text.replace(new RegExp(`{\\s*${paramKey}\\s*}`, 'g'), String(value));
    });
  }

  return text;
}
