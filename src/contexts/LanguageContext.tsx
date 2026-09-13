import { createContext, useContext, useEffect, useMemo, useState, useCallback, type ReactNode } from 'react';
import { useSettings } from './SettingsContext';
import { getTranslation, loadTranslation, isTranslationLoaded } from '@/i18n/translations';
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
  const { settings, updateSettings } = useSettings();

  const language = settings?.language || DEFAULT_LANGUAGE;

  const currentLanguage = useMemo(() => getLanguageByCode(language), [language]);

  // Track the active loaded language to trigger re-renders once chunk resolves
  const [activeLoadedLang, setActiveLoadedLang] = useState<string>(() =>
    isTranslationLoaded(language) ? language : ''
  );

  useEffect(() => {
    let cancelled = false;
    loadTranslation(language).then(() => {
      if (!cancelled) {
        setActiveLoadedLang(language);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [language]);

  useEffect(() => {
    document.documentElement.lang = currentLanguage.code;
    document.documentElement.dir = currentLanguage.dir || 'ltr';
  }, [currentLanguage]);

  const setLanguage = useCallback((newLang: string) => {
    updateSettings({ language: newLang });
  }, [updateSettings]);

  const t = useCallback((key: string, params?: Record<string, string | number>) => {
    return getTranslation(language, key, params);
  }, [language, activeLoadedLang]);

  const value = useMemo(
    () => ({
      language,
      currentLanguage,
      setLanguage,
      t,
      languages: LANGUAGES,
    }),
    [language, currentLanguage, setLanguage, t]
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
