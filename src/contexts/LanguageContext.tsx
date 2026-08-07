import { createContext, useContext, useEffect, useMemo, type ReactNode } from 'react';
import { useAppState } from './AppStateContext';
import { getTranslation } from '@/i18n/translations';
import { LANGUAGES, getLanguageByCode, type LanguageOption, DEFAULT_LANGUAGE } from '@/i18n/languages';

interface LanguageContextType {
  language: string;
  currentLanguage: LanguageOption;
  setLanguage: (lang: string) => void;
  t: (key: string, params?: Record<string, string | number>) => string;
  languages: LanguageOption[];
}

const LanguageContext = createContext<LanguageContextType | undefined>(undefined);

export function LanguageProvider({ children }: { children: ReactNode }) {
  const { settings, updateSettings } = useAppState();

  const language = settings?.language || DEFAULT_LANGUAGE;

  const currentLanguage = useMemo(() => getLanguageByCode(language), [language]);

  useEffect(() => {
    document.documentElement.lang = currentLanguage.code;
    document.documentElement.dir = currentLanguage.dir || 'ltr';
  }, [currentLanguage]);

  const setLanguage = (newLang: string) => {
    updateSettings({ language: newLang });
  };

  const t = (key: string, params?: Record<string, string | number>) => {
    return getTranslation(language, key, params);
  };

  const value = useMemo(
    () => ({
      language,
      currentLanguage,
      setLanguage,
      t,
      languages: LANGUAGES,
    }),
    [language, currentLanguage, settings]
  );

  return <LanguageContext.Provider value={value}>{children}</LanguageContext.Provider>;
}

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
