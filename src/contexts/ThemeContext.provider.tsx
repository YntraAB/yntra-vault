import { appMetadata } from '@/lib/appMetadata';
import { useCallback,useEffect,useState,useSyncExternalStore } from 'react';
import { type Theme,ThemeContext } from './ThemeContext.model';


export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(() => {
    const saved = appMetadata.getItem('yntra-vault-theme');
    return (saved as Theme) || 'system';
  });

  const systemDark = useSyncExternalStore(subscribeToSystemTheme, () => window.matchMedia('(prefers-color-scheme: dark)').matches, () => true);
  const resolvedTheme = theme === 'system' ? (systemDark ? 'dark' : 'light') : theme;
  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove('light', 'dark');
    root.classList.add(resolvedTheme);
  }, [resolvedTheme]);

  const setTheme = useCallback((newTheme: Theme) => {
    setThemeState(newTheme);
    appMetadata.setItem('yntra-vault-theme', newTheme);
  }, []);

  return (
    <ThemeContext.Provider value={{ theme, resolvedTheme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}
function subscribeToSystemTheme(notify: () => void) {
  const media = window.matchMedia('(prefers-color-scheme: dark)');
  media.addEventListener('change', notify);
  return () => media.removeEventListener('change', notify);
}
