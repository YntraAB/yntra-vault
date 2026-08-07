import { useState } from 'react';
import { Fingerprint, KeyRound, ShieldCheck } from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { useBackend } from '@/lib/useBackend';
import { SecurityDashboard } from '../SecurityDashboard';
import { SettingSection } from './SettingSection';
import type { BiometricInfo } from '@/lib/backend';

interface SecurityTabProps {
  bioActive: boolean;
  bioInfo: BiometricInfo | null;
  onToggleBiometric: () => void;
  hwActive: boolean;
  onOpenHwModal: (mode: 'enroll' | 'test') => void;
  onDisableHw: () => void;
  primaryUnlock: 'master_password' | 'biometric' | 'hardware_2fa';
  onSelectPrimaryUnlock: (val: 'master_password' | 'biometric' | 'hardware_2fa') => void;
  onOpenChangePassword: () => void;
}

export function SecurityTab({
  bioActive,
  bioInfo,
  onToggleBiometric,
  hwActive,
  onOpenHwModal,
  onDisableHw,
  primaryUnlock,
  onSelectPrimaryUnlock,
  onOpenChangePassword,
}: SecurityTabProps) {
  const { addToast } = useAppState();
  const { t } = useTranslation();
  const { backend } = useBackend();

  // Shamir state
  const [shamirPass, setShamirPass] = useState('');
  const [shares, setShares] = useState<string[]>([]);
  const [shareA, setShareA] = useState('');
  const [shareB, setShareB] = useState('');
  const [reconstructedHash, setReconstructedHash] = useState('');

  return (
    <div className="flex flex-col gap-6">
      {/* Biometric Unlock */}
      <SettingSection label="Biometric Unlock">
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          Unlock Yntra Vault using hardware biometrics ({bioInfo?.biometric_type || 'Windows Hello / Touch ID'}).
        </p>
        <div className="flex items-center justify-between rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
          <div className="flex items-center gap-3">
            <Fingerprint className="text-[var(--accent)]" size={20} />
            <div className="flex flex-col">
              <span className="text-[13px] font-medium text-[var(--text-primary)]">
                {bioInfo?.biometric_type || 'Hardware Biometrics'}
              </span>
              <span className="text-[11px] text-[var(--text-tertiary)]">
                {bioActive ? 'Enrolled for this vault' : 'Disabled'}
              </span>
            </div>
          </div>
          <button
            onClick={onToggleBiometric}
            className={`h-8 rounded-[3px] px-3 text-[12px] font-medium transition-colors ${
              bioActive
                ? 'border border-[var(--destructive)] bg-transparent text-[var(--destructive)] hover:bg-[var(--destructive)]/10'
                : 'border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-primary)] hover:bg-[var(--bg-hover)]'
            }`}
          >
            {bioActive ? 'Disable' : 'Enable'}
          </button>
        </div>
      </SettingSection>

      {/* Hardware 2FA / YubiKey */}
      <SettingSection label="Hardware 2FA / YubiKey">
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          Hardware 2FA (CTAP1 YubiKey HMAC-SHA1 & CTAP2 FIDO2 HMAC-Secret) cryptographic vault protection.
        </p>
        <div className="flex items-center justify-between rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
          <div className="flex items-center gap-3">
            <KeyRound className="text-white" size={20} />
            <div className="flex flex-col">
              <span className="text-[13px] font-medium text-[var(--text-primary)]">
                YubiKey / FIDO2 Security Key
              </span>
              <span className="text-[11px] text-[var(--text-tertiary)]">
                {hwActive ? 'Hardware 2FA Enrolled' : 'Not Enrolled'}
              </span>
            </div>
          </div>
          <div className="flex gap-2">
            {hwActive ? (
              <>
                <button
                  onClick={() => onOpenHwModal('test')}
                  className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors"
                >
                  Test Key
                </button>
                <button
                  onClick={onDisableHw}
                  className="h-8 rounded-[3px] border border-[var(--destructive)] bg-transparent px-3 text-[12px] font-medium text-[var(--destructive)] hover:bg-[var(--destructive)]/10 transition-colors"
                >
                  Disable
                </button>
              </>
            ) : (
              <button
                onClick={() => onOpenHwModal('enroll')}
                className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors"
              >
                Enable
              </button>
            )}
          </div>
        </div>
      </SettingSection>

      {/* Primary Choice of Login */}
      <SettingSection label="Primary Choice of Login">
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          Select your default unlock screen view on app launch. Master password fallback is always accessible.
        </p>
        <div className="grid grid-cols-3 gap-2.5">
          <button
            type="button"
            onClick={() => onSelectPrimaryUnlock('master_password')}
            className={`flex flex-col items-center justify-center p-3 rounded-[3px] border transition-all text-center ${
              primaryUnlock === 'master_password'
                ? 'border-[var(--text-primary)] bg-[var(--text-primary)]/10 text-[var(--text-primary)] font-medium'
                : 'border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]'
            }`}
          >
            <KeyRound size={18} className="mb-1 text-white" />
            <span className="text-[12px]">Master Password</span>
            <span className="text-[10px] text-[var(--text-tertiary)] mt-0.5">Always Required</span>
          </button>

          <button
            type="button"
            onClick={() => onSelectPrimaryUnlock('biometric')}
            disabled={!bioActive}
            className={`flex flex-col items-center justify-center p-3 rounded-[3px] border transition-all text-center ${
              !bioActive
                ? 'opacity-40 cursor-not-allowed border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-tertiary)]'
                : primaryUnlock === 'biometric'
                ? 'border-[var(--text-primary)] bg-[var(--text-primary)]/10 text-[var(--text-primary)] font-medium'
                : 'border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]'
            }`}
          >
            <Fingerprint size={18} className="mb-1 text-white" />
            <span className="text-[12px] truncate max-w-full">{bioInfo?.biometric_type?.replace(/\s*\(.*\)/, '') || 'Windows Hello'}</span>
            <span className="text-[10px] text-[var(--text-tertiary)] mt-0.5">
              {bioActive ? 'Enrolled' : 'Not Enrolled'}
            </span>
          </button>

          <button
            type="button"
            onClick={() => onSelectPrimaryUnlock('hardware_2fa')}
            disabled={!hwActive}
            className={`flex flex-col items-center justify-center p-3 rounded-[3px] border transition-all text-center ${
              !hwActive
                ? 'opacity-40 cursor-not-allowed border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-tertiary)]'
                : primaryUnlock === 'hardware_2fa'
                ? 'border-[var(--text-primary)] bg-[var(--text-primary)]/10 text-[var(--text-primary)] font-medium'
                : 'border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]'
            }`}
          >
            <ShieldCheck size={18} className="mb-1 text-white" />
            <span className="text-[12px]">YubiKey / FIDO2</span>
            <span className="text-[10px] text-[var(--text-tertiary)] mt-0.5">
              {hwActive ? 'Enrolled' : 'Not Enrolled'}
            </span>
          </button>
        </div>
      </SettingSection>

      {/* Master Password */}
      <SettingSection label={t('settings.master_password')}>
        <p className="mb-3 text-[12px] text-[var(--text-secondary)]">
          {t('security.master_password')}
        </p>
        <button
          onClick={onOpenChangePassword}
          className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)]"
        >
          {t('settings.change_password')}
        </button>
      </SettingSection>

      {/* Emergency Recovery */}
      <SettingSection label={t('settings.emergency_recovery')}>
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
                  addToast({ message: 'Recovery shares generated!', type: 'success' });
                } catch (err) {
                  addToast({ message: `Split failed: ${err}`, type: 'error' });
                }
              }}
              className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)]"
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
                      navigator.clipboard.writeText(s).catch(() => {});
                      addToast({ message: `Share ${idx + 1} copied`, type: 'success' });
                    }}
                    className="text-[10px] font-medium text-[var(--text-primary)] hover:underline"
                  >
                    {t('common.copy')}
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="flex flex-col gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3 mt-3">
          <span className="text-[11px] font-medium text-[var(--text-primary)]">{t('settings.reconstruct_hash')}</span>
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
                  const res = await backend.reconstructMasterPasswordHash(shareA, shareB);
                  setReconstructedHash(res);
                  addToast({ message: 'Hash reconstructed successfully', type: 'success' });
                } catch (err) {
                  addToast({ message: `Reconstruction failed: ${err}`, type: 'error' });
                }
              }}
              className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)]"
            >
              {t('settings.reconstruct_hash')}
            </button>
            {reconstructedHash && (
              <div className="flex flex-col gap-0.5 mt-1 bg-[var(--bg-base)] p-2 rounded-[3px]">
                <span className="text-[10px] font-bold uppercase tracking-wider text-[var(--text-tertiary)]">{t('settings.reconstructed_hash_label')}</span>
                <span className="font-mono text-[10px] text-green-500 break-all select-all">{reconstructedHash}</span>
              </div>
            )}
          </div>
        </div>
      </SettingSection>

      {/* Security Health Dashboard */}
      <SettingSection label={t('security.title')}>
        <SecurityDashboard />
      </SettingSection>
    </div>
  );
}
