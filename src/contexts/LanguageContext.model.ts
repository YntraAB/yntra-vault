import { DEFAULT_LANGUAGE,LANGUAGES,getLanguageByCode,type LanguageOption } from '@/i18n/languages';
import { getTranslation } from '@/i18n/translations';
import { createContext,useContext } from 'react';

export interface LanguageContextType {
  language: string;
  currentLanguage: LanguageOption;
  setLanguage: (lang: string) => void;
  t: (key: string, params?: Record<string, string | number>) => string;
  languages: LanguageOption[];
}

export const LanguageContext = createContext<LanguageContextType | undefined>(undefined);


export function useTranslation() {
  const context = useContext(LanguageContext);
  if (!context) {
    // Fallback if component is rendered outside provider
    return {
      t: (key: string, params?: Record<string, string | number>) => getTranslation(DEFAULT_LANGUAGE, key, params),
      language: DEFAULT_LANGUAGE,
      currentLanguage: getLanguageByCode(DEFAULT_LANGUAGE),
      setLanguage: () => {},
      languages: LANGUAGES,
    };
  }
  return context;
}