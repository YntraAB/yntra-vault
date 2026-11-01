import { DEFAULT_LANGUAGE,LANGUAGES,getLanguageByCode } from '@/i18n/languages';
import { getTranslation,isTranslationLoaded,loadTranslation } from '@/i18n/translations';
import { useCallback,useEffect,useMemo,useState,type ReactNode } from 'react';
import { LanguageContext } from './LanguageContext.model';
import { useSettings } from './SettingsContext';


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
    return getTranslation(activeLoadedLang === language ? activeLoadedLang : language, key, params);
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