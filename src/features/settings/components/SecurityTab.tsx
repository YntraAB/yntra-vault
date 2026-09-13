import { useState } from 'react';
import { Fingerprint, KeyRound, Loader2 } from 'lucide-react';
import { useSettings } from '../context/SettingsContext';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { useBackend } from '@/lib/useBackend';
import { SecurityDashboard } from '@/features/audit';
import { SettingSection, Toggle } from './SettingSection';
import { isTauri, type BiometricInfo } from '@/lib/backend';

export interface SecurityTabProps {
  bioActive: boolean;
  bioInfo: BiometricInfo | null;
  onToggleBiometric: () => void;
  isTogglingBio?: boolean;
  hwActive: boolean;
  onOpenHwModal: (mode: 'enroll' | 'test') => void;
  onDisableHw: () => void;
  onOpenChangePassword: () => void;
}

export function SecurityTab({
  bioActive,
  bioInfo,
  onToggleBiometric,
  isTogglingBio = false,
  hwActive,
  onOpenHwModal,
  onDisableHw,
  onOpenChangePassword,
}: SecurityTabProps) {
  const { settings, updateSettings } = useSettings();
  const { addToast } = useToast();
  const { t } = useTranslation();
  const { backend } = useBackend();

  // Shamir state
  const [shamirPass, setShamirPass] = useState('');
  const [shares, setShares] = useState<string[]>([]);
  const [shareA, setShareA] = useState('');
  const [shareB, setShareB] = useState('');
  const [reconstructedPass, setReconstructedPass] = useState('');

  return (
    <div className="flex flex-col gap-6">
      {/* Security Health Dashboard */}
      <SettingSection label={t('security.title')}>
        <SecurityDashboard />
      </SettingSection>

      {/* Master Password */}
      <SettingSection
        label={t('settings.master_password')}
        tooltip={t('settings.tooltip_change_password')}
      >
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          {t('security.master_password')}
        </p>
        <button
          onClick={onOpenChangePassword}
          className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer"
        >
          {t('settings.change_password')}
        </button>
      </SettingSection>

      {/* Window Capture Protection */}
      <SettingSection
        label={t('settings.capture_protection_label')}
        tooltip={t('settings.tooltip_capture_protection')}
      >
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          {t('settings.capture_protection_desc')}
        </p>
        <div className="flex items-center justify-between rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
          <div className="flex flex-col">
            <span className="text-[13px] font-medium text-[var(--text-primary)]">
              {t('settings.capture_protection_enable')}
            </span>
            <span className="text-[11px] text-[var(--text-tertiary)]">
              {settings.windowCaptureProtection !== false ? t('settings.capture_protection_active') : t('common.disabled')}
            </span>
          </div>
          <Toggle
            checked={settings.windowCaptureProtection !== false}
            onChange={(checked) => updateSettings({ windowCaptureProtection: checked })}
          />
        </div>
      </SettingSection>

      {/* Aggressive Auto-Lock */}
      <SettingSection
        label={t('settings.aggressive_autolock_label')}
        tooltip={t('settings.tooltip_aggressive_autolock')}
      >
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          {t('settings.aggressive_autolock_desc')}
        </p>
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
            <div className="flex flex-col">
              <span className="text-[13px] font-medium text-[var(--text-primary)]">
                {t('settings.lock_on_system_lock_label')}
              </span>
              <span className="text-[11px] text-[var(--text-tertiary)]">
                {settings.lockOnSystemLock !== false ? t('settings.lock_on_system_lock_active') : t('common.disabled')}
              </span>
            </div>
            <Toggle
              checked={settings.lockOnSystemLock !== false}
              onChange={(checked) => updateSettings({ lockOnSystemLock: checked })}
            />
          </div>

          <div className="flex items-center justify-between rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
            <div className="flex flex-col">
              <span className="text-[13px] font-medium text-[var(--text-primary)]">
                {t('settings.lock_on_focus_loss_label')}
              </span>
              <span className="text-[11px] text-[var(--text-tertiary)]">
                {settings.lockOnFocusLoss === true ? t('settings.lock_on_focus_loss_active') : t('common.disabled')}
              </span>
            </div>
            <Toggle
              checked={settings.lockOnFocusLoss === true}
              onChange={(checked) => updateSettings({ lockOnFocusLoss: checked })}
            />
          </div>
        </div>
      </SettingSection>

      {/* Biometric Unlock */}
      <SettingSection
        label={t('settings.biometric_unlock')}
        tooltip={t('settings.tooltip_biometric')}
      >
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          {t('settings.biometric_desc')}
        </p>
        <div className="flex items-center justify-between rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
          <div className="flex items-center gap-3">
            <Fingerprint className="text-[var(--accent)]" size={20} />
            <div className="flex flex-col">
              <span className="text-[13px] font-medium text-[var(--text-primary)]">
                {bioInfo?.biometric_type || t('settings.biometric_hardware_title')}
              </span>
              <span className="text-[11px] text-[var(--text-tertiary)]">
                {hwActive
                  ? 'Disabled while Hardware 2FA is active'
                  : bioActive
                    ? t('settings.biometric_enrolled')
                    : t('settings.biometric_disabled')}
              </span>
            </div>
          </div>
          <button
            onClick={onToggleBiometric}
            disabled={hwActive || isTogglingBio}
            className={`h-8 rounded-[3px] px-3 text-[12px] font-medium transition-colors cursor-pointer inline-flex items-center gap-1.5 select-none ${
              hwActive || isTogglingBio
                ? 'border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-tertiary)] opacity-50 cursor-not-allowed'
                : bioActive
                  ? 'border border-[var(--destructive)] bg-transparent text-[var(--destructive)] hover:bg-[var(--destructive)]/10'
                  : 'border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-primary)] hover:bg-[var(--bg-hover)]'
            }`}
          >
            {isTogglingBio ? (
              <>
                <Loader2 size={12} className="animate-spin" />
                <span>Verifying...</span>
              </>
            ) : bioActive ? (
              t('common.disable')
            ) : (
              t('common.enable')
            )}
          </button>
        </div>
      </SettingSection>

      {/* Hardware 2FA / YubiKey */}
      <SettingSection
        label={t('settings.hardware_2fa')}
        tooltip={t('settings.tooltip_hardware_2fa')}
      >
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          {t('settings.hardware_2fa_desc')}
        </p>
        <div className="flex items-center justify-between rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
          <div className="flex items-center gap-3">
            <KeyRound className="text-white" size={20} />
            <div className="flex flex-col">
              <span className="text-[13px] font-medium text-[var(--text-primary)]">
                {t('settings.yubikey_title')}
              </span>
              <span className="text-[11px] text-[var(--text-tertiary)]">
                {hwActive ? t('settings.yubikey_enrolled') : t('settings.yubikey_not_enrolled')}
              </span>
            </div>
          </div>
          <div className="flex gap-2">
            {hwActive ? (
              <>
                <button
                  onClick={() => onOpenHwModal('test')}
                  className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                >
                  {t('settings.test_key')}
                </button>
                <button
                  onClick={onDisableHw}
                  className="h-8 rounded-[3px] border border-[var(--destructive)] bg-transparent px-3 text-[12px] font-medium text-[var(--destructive)] hover:bg-[var(--destructive)]/10 transition-colors cursor-pointer"
                >
                  {t('common.disable')}
                </button>
              </>
            ) : (
              <button
                onClick={() => onOpenHwModal('enroll')}
                className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
              >
                {t('common.enable')}
              </button>
            )}
          </div>
        </div>
      </SettingSection>

      {/* Emergency Recovery */}
      <SettingSection
        label={t('settings.emergency_recovery')}
        tooltip={t('settings.tooltip_emergency_recovery')}
      >
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          {t('settings.emergency_recovery_desc')}
        </p>
        <div className="flex flex-col gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
          <div className="flex gap-2">
            <input
              type="password"
              placeholder={t('settings.verify_master_placeholder')}
              value={shamirPass}
              onChange={(e) => setShamirPass(e.target.value)}
              className="h-8 flex-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
            />
            <button
              onClick={async () => {
                if (!backend || !shamirPass) return;
                try {
                  const res = await backend.splitMasterPassword(shamirPass);
                  setShares(res);
                  addToast({ message: t('toast.recovery_shares_generated'), type: 'success' });
                } catch (err) {
                  addToast({ message: t('toast.split_failed', { err: String(err) }), type: 'error' });
                }
              }}
              className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer"
            >
              {t('settings.split_button')}
            </button>
          </div>

          {shares.length > 0 && (
            <div className="flex flex-col gap-1.5 mt-2 border-t border-[var(--border-subtle)] pt-2.5">
              <span className="text-[10px] font-bold uppercase tracking-wider text-[var(--text-tertiary)]">{t('settings.recovery_shares_label')}</span>
              {shares.map((s, idx) => (
                <div key={idx} className="flex items-center justify-between gap-2 rounded-[3px] bg-[var(--bg-base)] px-2 py-1">
                  <span className="font-mono text-[10px] text-[var(--text-secondary)] select-all truncate">{s}</span>
                  <button
                    onClick={() => {
                      if (isTauri()) {
                        backend?.copyToClipboard(s, true, 30).catch(() => {});
                      } else {
                        navigator.clipboard.writeText(s).catch(() => {});
                      }
                      addToast({ message: t('toast.share_copied', { index: idx + 1 }), type: 'success' });
                    }}
                    className="text-[10px] font-medium text-[var(--text-primary)] hover:underline cursor-pointer"
                  >
                    {t('common.copy')}
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="flex flex-col gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3 mt-3">
          <div className="flex flex-col">
            <span className="text-[11px] font-medium text-[var(--text-primary)]">
              {t('settings.verify_recovery_shares') || 'Verify & Test Recovery Shares'}
            </span>
            <span className="text-[11px] text-[var(--text-tertiary)]">
              {t('settings.verify_recovery_shares_desc') || 'Test your paper recovery shares to verify they successfully reconstruct your master password.'}
            </span>
          </div>
          <div className="flex flex-col gap-2">
            <input
              type="text"
              placeholder={t('settings.share1_placeholder')}
              value={shareA}
              onChange={(e) => setShareA(e.target.value)}
              className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
            />
            <input
              type="text"
              placeholder={t('settings.share2_placeholder')}
              value={shareB}
              onChange={(e) => setShareB(e.target.value)}
              className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
            />
            <button
              onClick={async () => {
                if (!backend || !shareA || !shareB) return;
                try {
                  const res = await backend.reconstructMasterPassword(shareA, shareB);
                  setReconstructedPass(res);
                  addToast({ message: t('toast.password_reconstructed') || 'Master password reconstructed successfully', type: 'success' });
                } catch (err) {
                  addToast({ message: t('toast.reconstruction_failed', { err: String(err) }), type: 'error' });
                }
              }}
              className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer"
            >
              {t('settings.verify_shares_button') || 'Verify & Reconstruct Shares'}
            </button>
            {reconstructedPass && (
              <div className="flex flex-col gap-1 mt-1 bg-[var(--bg-base)] p-2.5 rounded-[3px] border border-green-500/30">
                <div className="flex items-center justify-between">
                  <span className="text-[10px] font-bold uppercase tracking-wider text-[var(--text-tertiary)]">{t('settings.reconstructed_password_label') || 'Reconstructed Master Password'}</span>
                  <button
                    onClick={() => {
                      if (isTauri()) {
                        backend?.copyToClipboard(reconstructedPass, true, 30).catch(() => {});
                      } else {
                        navigator.clipboard.writeText(reconstructedPass).catch(() => {});
                      }
                      addToast({ message: t('toast.password_copied') || 'Password copied to clipboard', type: 'success' });
                    }}
                    className="text-[10px] font-medium text-green-500 hover:underline cursor-pointer"
                  >
                    {t('common.copy')}
                  </button>
                </div>
                <span className="font-mono text-[12px] text-green-400 break-all select-all font-semibold">{reconstructedPass}</span>
              </div>
            )}
          </div>
        </div>
      </SettingSection>
    </div>
  );
}

export default SecurityTab;
