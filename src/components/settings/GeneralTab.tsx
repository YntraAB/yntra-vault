import { useAppState } from '@/contexts/AppStateContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { LanguageCombobox } from '../ui/LanguageCombobox';
import { SettingSection, SettingRow, Toggle } from './SettingSection';
import { isTauri } from '@/lib/backend';
import { useBackend } from '@/lib/useBackend';

const AUTO_LOCK_OPTIONS = [
  { value: 1, labelKey: 'time.1_min' },
  { value: 5, labelKey: 'time.5_min' },
  { value: 15, labelKey: 'time.15_min' },
  { value: 30, labelKey: 'time.30_min' },
  { value: 0, labelKey: 'time.never' },
];

const CLIPBOARD_OPTIONS = [
  { value: 10, labelKey: 'time.10_sec' },
  { value: 30, labelKey: 'time.30_sec' },
  { value: 60, labelKey: 'time.1_min' },
  { value: 300, labelKey: 'time.5_min' },
  { value: 0, labelKey: 'time.never' },
];

interface GeneralTabProps {
  launchOnStartup: boolean;
  onToggleLaunch: (val: boolean) => void;
}

export function GeneralTab({ launchOnStartup, onToggleLaunch }: GeneralTabProps) {
  const { currentVault, settings, updateSettings, addToast, setIsLocked, setCurrentVault } = useAppState();
  const { t } = useTranslation();
  const { backend } = useBackend();

  return (
    <div className="flex flex-col gap-6">
      {currentVault && (
        <SettingSection label={t('settings.active_vault')}>
          <div className="flex flex-col gap-2.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
            <div className="flex flex-col gap-0.5">
              <span className="text-[10px] font-bold uppercase tracking-wider text-[var(--text-tertiary)]">{t('settings.vault_name')}</span>
              <span className="text-[13px] font-medium text-[var(--text-primary)]">{currentVault.name}</span>
            </div>
            <div className="flex flex-col gap-0.5">
              <span className="text-[10px] font-bold uppercase tracking-wider text-[var(--text-tertiary)]">{t('settings.file_location')}</span>
              <span className="font-mono text-[11px] text-[var(--text-secondary)] break-all">{currentVault.path}</span>
            </div>
            {isTauri() && (
              <button
                type="button"
                onClick={() => {
                  backend?.showInExplorer(currentVault.path).catch((err: any) => {
                    addToast({ message: `Failed to open explorer: ${err}`, type: 'error' });
                  });
                }}
                className="mt-1 h-7 self-start rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[11px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)]"
              >
                {t('settings.show_in_explorer')}
              </button>
            )}
          </div>
        </SettingSection>
      )}

      <SettingSection label={t('settings.language_label')}>
        <p className="mb-2.5 text-[12px] text-[var(--text-secondary)]">
          {t('settings.language_desc')}
        </p>
        <LanguageCombobox />
      </SettingSection>

      <SettingSection label={t('settings.autolock_label')}>
        <p className="mb-2 text-[12px] text-[var(--text-secondary)]">
          {t('settings.autolock_desc')}
        </p>
        <select
          value={settings.autoLockMinutes}
          onChange={(e) => updateSettings({ autoLockMinutes: Number(e.target.value) })}
          className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
        >
          {AUTO_LOCK_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {t(o.labelKey)}
            </option>
          ))}
        </select>
      </SettingSection>

      <SettingSection
        label={t('settings.clipboard_label')}
        tooltip={t('settings.tooltip_clipboard')}
      >
        <p className="mb-2 text-[12px] text-[var(--text-secondary)]">
          {t('settings.clipboard_desc')}
        </p>
        <select
          value={settings.clipboardClearSeconds}
          onChange={(e) =>
            updateSettings({ clipboardClearSeconds: Number(e.target.value) })
          }
          className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
        >
          {CLIPBOARD_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {t(o.labelKey)}
            </option>
          ))}
        </select>
      </SettingSection>

      <SettingSection
        label={t('settings.autotype_title')}
        tooltip={t('settings.tooltip_autotype')}
      >
        <div className="flex flex-col gap-4">
          <div>
            <div className="flex justify-between items-center mb-1">
              <span className="text-[12px] text-[var(--text-secondary)]">{t('settings.char_delay')}</span>
              <span className="text-[11px] font-mono text-[var(--text-primary)]">{(settings.autotypeCharDelayMs ?? 15)} ms</span>
            </div>
            <input
              type="range"
              min={5}
              max={100}
              step={5}
              value={settings.autotypeCharDelayMs ?? 15}
              onChange={(e) => updateSettings({ autotypeCharDelayMs: Number(e.target.value) })}
              className="h-1 w-full appearance-none rounded-full bg-[var(--border)] outline-none [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-[var(--text-primary)]"
            />
          </div>
          <div>
            <div className="flex justify-between items-center mb-1">
              <span className="text-[12px] text-[var(--text-secondary)]">{t('settings.field_delay')}</span>
              <span className="text-[11px] font-mono text-[var(--text-primary)]">{(settings.autotypeFieldDelayMs ?? 300)} ms</span>
            </div>
            <input
              type="range"
              min={100}
              max={2000}
              step={100}
              value={settings.autotypeFieldDelayMs ?? 300}
              onChange={(e) => updateSettings({ autotypeFieldDelayMs: Number(e.target.value) })}
              className="h-1 w-full appearance-none rounded-full bg-[var(--border)] outline-none [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-[var(--text-primary)]"
            />
          </div>
          <div>
            <div className="flex justify-between items-center mb-1">
              <span className="text-[12px] text-[var(--text-secondary)]">{t('settings.settle_delay')}</span>
              <span className="text-[11px] font-mono text-[var(--text-primary)]">{((settings.autotypeSettleDelayMs ?? 3000) / 1000).toFixed(1)} s</span>
            </div>
            <input
              type="range"
              min={500}
              max={5000}
              step={500}
              value={settings.autotypeSettleDelayMs ?? 3000}
              onChange={(e) => updateSettings({ autotypeSettleDelayMs: Number(e.target.value) })}
              className="h-1 w-full appearance-none rounded-full bg-[var(--border)] outline-none [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-[var(--text-primary)]"
            />
          </div>
        </div>
      </SettingSection>

      <SettingRow
        label={t('settings.autotype_launch_browser')}
        description={t('settings.autotype_launch_browser_desc')}
      >
        <Toggle
          checked={settings.autotypeLaunchBrowser !== false}
          onChange={(v) => updateSettings({ autotypeLaunchBrowser: v })}
        />
      </SettingRow>

      <SettingRow
        label={t('settings.minimize_to_tray')}
        description={t('settings.minimize_to_tray_desc')}
      >
        <Toggle
          checked={settings.minimizeToTray}
          onChange={(v) => updateSettings({ minimizeToTray: v })}
        />
      </SettingRow>

      <SettingRow label={t('settings.autostart_label')} description={t('settings.autostart_desc')}>
        <Toggle
          checked={launchOnStartup}
          onChange={onToggleLaunch}
        />
      </SettingRow>

      <SettingRow
        label={t('settings.disable_skeleton_delays')}
        description={t('settings.disable_skeleton_delays_desc')}
        tooltip={t('settings.tooltip_disable_skeleton_delays')}
      >
        <Toggle
          checked={settings.disableSkeletonDelays}
          onChange={(v) => updateSettings({ disableSkeletonDelays: v })}
        />
      </SettingRow>

      <SettingRow
        label={t('settings.breach_label')}
        description={t('settings.breach_desc')}
        tooltip={t('settings.tooltip_breach')}
      >
        <Toggle
          checked={settings.autoBreachCheck}
          onChange={(v) => updateSettings({ autoBreachCheck: v })}
        />
      </SettingRow>

      <SettingRow
        label={t('settings.show_breach_in_list')}
        description={t('settings.show_breach_in_list_desc')}
      >
        <Toggle
          checked={settings.showBreachInList}
          onChange={(v) => updateSettings({ showBreachInList: v })}
        />
      </SettingRow>

      <SettingSection label={t('onboarding.rerun_setup')}>
        <button
          onClick={() => {
            localStorage.removeItem('yntra-vault-setup-completed');
            setIsLocked(true);
            setCurrentVault(null);
            window.location.href = '/#/setup';
          }}
          className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)]"
        >
          {t('onboarding.rerun_setup')}
        </button>
      </SettingSection>
    </div>
  );
}
