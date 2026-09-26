import { useCapabilities } from '@/lib/platform';
import { LocalProtectionSettings, type LocalProtectionInfo } from '@/features/auth/components/LocalProtection';
import { useState } from 'react';
import { Loader2 } from 'lucide-react';
import { useSettings } from '../context/SettingsContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { SecurityDashboard } from '@/features/audit';
import { SettingSection, SettingRow, Toggle } from './SettingSection';
import { type BiometricInfo } from '@/lib/backend';

export interface SecurityTabProps {
  bioActive: boolean;
  bioInfo: BiometricInfo | null;
  onToggleBiometric: () => void;
  isTogglingBio?: boolean;
  hwActive: boolean;
  hwLegacy?: boolean;
  onOpenHwModal: (mode: 'enroll' | 'test') => void;
  onDisableHw: () => void;
  onOpenChangePassword: () => void;
  onNavigateToEntry?: (entryId: string) => void;
}

export function SecurityTab({
  bioActive,
  bioInfo,
  onToggleBiometric,
  isTogglingBio = false,
  hwActive,
  hwLegacy = false,
  onOpenHwModal,
  onDisableHw,
  onOpenChangePassword,
  onNavigateToEntry,
}: SecurityTabProps) {
  const { settings, updateSettings } = useSettings();
  const capabilities = useCapabilities();
  const { t, language } = useTranslation();
  const [localProtection,setLocalProtection]=useState<LocalProtectionInfo|null>(null);
  const protectedLocal=localProtection?.protected===true;
  const unavailable=language==='sv'?'Inte tillgängligt med recovery v2 och USB-bindning.':'Unavailable with recovery v2 and USB binding.';

  return (
    <div className="flex flex-col gap-6">
      {/* Security Health Dashboard */}
      <SettingSection label={t('security.title')}>
        <SecurityDashboard
          onNavigateToEntry={onNavigateToEntry}
          onOpenChangePassword={onOpenChangePassword}
        />
      </SettingSection>

      {/* Master Password */}
      <SettingRow
        label={t('settings.master_password')}
        description={t('settings.master_password_desc')}
        tooltip={t('settings.tooltip_change_password')}
      >
        <button
          type="button"
          onClick={onOpenChangePassword}
          className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer whitespace-nowrap shrink-0"
        >
          {t('settings.change_password')}
        </button>
      </SettingRow>

      {/* Window Capture Protection */}
      {capabilities.captureProtection && (<SettingRow
        label={t('settings.capture_protection_label')}
        description={t('settings.capture_protection_desc')}
        tooltip={t('settings.tooltip_capture_protection')}
      >
        <Toggle
          checked={settings.windowCaptureProtection !== false}
          onChange={(checked) => updateSettings({ windowCaptureProtection: checked })}
        />
      </SettingRow>)}

      {/* System Lock & Focus Loss Auto-Lock */}
      {capabilities.desktop && (<SettingRow
        label={t('settings.lock_on_system_lock_label')}
        description={t('settings.lock_on_system_lock_desc')}
        tooltip={t('settings.tooltip_aggressive_autolock')}
      >
        <Toggle
          checked={settings.lockOnSystemLock !== false}
          onChange={(checked) => updateSettings({ lockOnSystemLock: checked })}
        />
      </SettingRow>)}

      {capabilities.desktop && (<SettingRow
        label={t('settings.lock_on_focus_loss_label')}
        description={t('settings.lock_on_focus_loss_desc')}
        tooltip={t('settings.tooltip_aggressive_autolock')}
      >
        <Toggle
          checked={settings.lockOnFocusLoss === true}
          onChange={(checked) => updateSettings({ lockOnFocusLoss: checked })}
        />
      </SettingRow>)}

      {/* Biometric Unlock */}
      {capabilities.nativeAuthentication && (<SettingRow
        label={bioInfo?.biometric_type || t('settings.biometric_hardware_title')}
        description={
          protectedLocal ? unavailable : hwActive
            ? t('settings.biometric_disabled_hw')
            : bioActive
              ? t('settings.biometric_enrolled')
              : t('settings.biometric_desc')
        }
        tooltip={t('settings.tooltip_biometric')}
      >
        <button
          type="button"
          onClick={onToggleBiometric}
          disabled={protectedLocal || hwActive || isTogglingBio}
          className={`h-8 rounded-[3px] px-3 text-[12px] font-medium transition-colors cursor-pointer inline-flex items-center gap-1.5 select-none whitespace-nowrap shrink-0 ${
            hwActive || isTogglingBio
              ? 'border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-tertiary)] opacity-50 cursor-not-allowed'
              : bioActive
                ? 'border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]'
                : 'border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-primary)] hover:bg-[var(--bg-hover)]'
          }`}
        >
          {isTogglingBio ? (
            <>
              <Loader2 size={12} className="animate-spin" />
              <span>{t('common.verifying') || 'Verifying...'}</span>
            </>
          ) : bioActive && !protectedLocal ? (
            t('common.disable')
          ) : (
            t('common.enable')
          )}
        </button>
      </SettingRow>)}

      {hwLegacy && capabilities.nativeAuthentication && <p role="alert" className="text-[12px] text-[var(--destructive)]">{language === 'sv' ? 'Det här valvet har äldre hårdvaruskydd. Registrera nyckeln igen för att krypteringen ska kräva säkerhetsnyckeln. Gamla kopior behåller det äldre skyddet.' : 'This vault uses legacy hardware protection. Re-enroll the key to make the encryption require the security key. Old copies retain the legacy protection.'}</p>}
      {/* Hardware 2FA / YubiKey */}
      {capabilities.nativeAuthentication && (<SettingRow
        label={t('settings.yubikey_title')}
        description={protectedLocal ? unavailable : hwActive ? t('settings.yubikey_enrolled') : t('settings.hardware_2fa_desc')}
        tooltip={t('settings.tooltip_hardware_2fa')}
      >
        <div className="flex flex-wrap gap-2">
          {hwActive ? (
            <>
              <button type="button" onClick={() => onOpenHwModal('enroll')} className="h-8 rounded-[3px] border border-[var(--border)] px-3 text-[12px]">
                {language === 'sv' ? 'Registrera igen' : 'Re-enroll'}
              </button>
              <button
                type="button"
                onClick={() => onOpenHwModal('test')}
                className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer whitespace-nowrap shrink-0"
              >
                {t('settings.test_key')}
              </button>
              <button
                type="button"
                onClick={onDisableHw}
                className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer whitespace-nowrap shrink-0"
              >
                {t('common.disable')}
              </button>
            </>
          ) : (
            <button
              type="button"
              onClick={() => onOpenHwModal('enroll')}
              disabled={protectedLocal}
              className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer whitespace-nowrap shrink-0"
            >
              {t('common.enable')}
            </button>
          )}
        </div>
      </SettingRow>)}


      <LocalProtectionSettings onProtectionChanged={setLocalProtection} />
    </div>
  );
}
