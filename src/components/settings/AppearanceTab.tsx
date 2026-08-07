import { Sun, Moon, Monitor } from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';
import { useTheme } from '@/contexts/ThemeContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { SettingSection } from './SettingSection';

export function AppearanceTab() {
  const { settings, updateSettings } = useAppState();
  const { theme, setTheme } = useTheme();
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-6">
      <SettingSection label={t('settings.theme_label')}>
        <div className="flex gap-2">
          {([
            { value: 'light' as const, label: t('settings.theme_light'), icon: <Sun size={20} /> },
            { value: 'dark' as const, label: t('settings.theme_dark'), icon: <Moon size={20} /> },
            { value: 'system' as const, label: t('settings.theme_system'), icon: <Monitor size={20} /> },
          ]).map((tItem) => (
            <button
              key={tItem.value}
              onClick={() => setTheme(tItem.value)}
              className={`flex h-[72px] w-[100px] flex-col items-center justify-center gap-1.5 rounded-[3px] border text-[12px] font-medium transition-colors ${
                theme === tItem.value
                  ? 'border-[var(--text-primary)] text-[var(--text-primary)]'
                  : 'border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--border-focus)]'
              }`}
            >
              {tItem.icon}
              {tItem.label}
            </button>
          ))}
        </div>
      </SettingSection>

      <SettingSection label={t('settings.font_size')}>
        <div className="flex items-center gap-4">
          <input
            type="range"
            min={12}
            max={16}
            step={1}
            value={settings.fontSize}
            onChange={(e) => updateSettings({ fontSize: Number(e.target.value) })}
            className="h-1 flex-1 appearance-none rounded-full bg-[var(--border)] outline-none [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-[var(--text-primary)]"
          />
          <span className="w-10 text-right text-[12px] text-[var(--text-secondary)]">
            {settings.fontSize}px
          </span>
        </div>
      </SettingSection>

      <SettingSection label={t('settings.density')}>
        <p className="mb-2 text-[12px] text-[var(--text-secondary)]">
          {t('settings.density_desc')}
        </p>
        <div className="flex rounded-[3px] border border-[var(--border)]">
          {(['compact', 'normal', 'comfortable'] as const).map((d) => (
            <button
              key={d}
              onClick={() => updateSettings({ density: d })}
              className={`flex-1 py-1.5 text-[12px] font-medium capitalize transition-colors ${
                settings.density === d
                  ? 'bg-[var(--bg-active)] text-[var(--text-primary)]'
                  : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
              }`}
            >
              {d === 'compact' ? t('settings.density_compact') : d === 'normal' ? t('settings.density_normal') : t('settings.density_comfortable')}
            </button>
          ))}
        </div>
      </SettingSection>
    </div>
  );
}
