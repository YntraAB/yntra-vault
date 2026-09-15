import { useState, useEffect } from 'react';
import { Loader2, FileText, Download, ChevronDown, ChevronUp, ShieldCheck, History, RotateCcw } from 'lucide-react';
import { useSettings } from '../context/SettingsContext';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { useBackend } from '@/lib/useBackend';
import { SecurityDashboard } from '@/features/audit';
import { SettingSection, SettingRow, Toggle } from './SettingSection';
import { isTauri, type BiometricInfo, type EmergencyKit, type EmergencyKitAudit } from '@/lib/backend';

export interface SecurityTabProps {
  bioActive: boolean;
  bioInfo: BiometricInfo | null;
  onToggleBiometric: () => void;
  isTogglingBio?: boolean;
  hwActive: boolean;
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
  onOpenHwModal,
  onDisableHw,
  onOpenChangePassword,
  onNavigateToEntry,
}: SecurityTabProps) {
  const { settings, updateSettings } = useSettings();
  const { addToast } = useToast();
  const { t } = useTranslation();
  const { backend } = useBackend();

  // Shamir & Emergency Kit state
  const [shamirPass, setShamirPass] = useState('');
  const [emergencyKit, setEmergencyKit] = useState<EmergencyKit | null>(null);
  const [audit, setAudit] = useState<EmergencyKitAudit | null>(null);
  const [isGeneratingKit, setIsGeneratingKit] = useState(false);
  const [isResettingAudit, setIsResettingAudit] = useState(false);
  const [showKitSheet, setShowKitSheet] = useState(false);
  const [showAuditLogs, setShowAuditLogs] = useState(false);
  const [passError, setPassError] = useState(false);

  useEffect(() => {
    backend?.getEmergencyKitAudit().then(setAudit).catch(() => {});
  }, [backend]);

  const handleGenerateKit = async () => {
    if (!backend || !shamirPass || isGeneratingKit) return;
    setIsGeneratingKit(true);
    setPassError(false);
    try {
      const kit = await backend.generateEmergencyKit(shamirPass);
      setEmergencyKit(kit);
      setShamirPass('');
      const updatedAudit = await backend.getEmergencyKitAudit();
      setAudit(updatedAudit);
      addToast({ message: t('toast.recovery_shares_generated') || 'Emergency Kit generated', type: 'success' });
    } catch (err) {
      const errStr = String(err);
      if (errStr.toLowerCase().includes('password') || errStr.toLowerCase().includes('lösenord')) {
        setPassError(true);
        addToast({ message: t('error.invalid_password'), type: 'error' });
      } else {
        addToast({ message: t('toast.split_failed', { err: errStr }), type: 'error' });
      }
    } finally {
      setIsGeneratingKit(false);
    }
  };

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
      <SettingRow
        label={t('settings.capture_protection_label')}
        description={t('settings.capture_protection_desc')}
        tooltip={t('settings.tooltip_capture_protection')}
      >
        <Toggle
          checked={settings.windowCaptureProtection !== false}
          onChange={(checked) => updateSettings({ windowCaptureProtection: checked })}
        />
      </SettingRow>

      {/* System Lock & Focus Loss Auto-Lock */}
      <SettingRow
        label={t('settings.lock_on_system_lock_label')}
        description={t('settings.lock_on_system_lock_desc')}
        tooltip={t('settings.tooltip_aggressive_autolock')}
      >
        <Toggle
          checked={settings.lockOnSystemLock !== false}
          onChange={(checked) => updateSettings({ lockOnSystemLock: checked })}
        />
      </SettingRow>

      <SettingRow
        label={t('settings.lock_on_focus_loss_label')}
        description={t('settings.lock_on_focus_loss_desc')}
        tooltip={t('settings.tooltip_aggressive_autolock')}
      >
        <Toggle
          checked={settings.lockOnFocusLoss === true}
          onChange={(checked) => updateSettings({ lockOnFocusLoss: checked })}
        />
      </SettingRow>

      {/* Biometric Unlock */}
      <SettingRow
        label={bioInfo?.biometric_type || t('settings.biometric_hardware_title')}
        description={
          hwActive
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
          disabled={hwActive || isTogglingBio}
          className={`h-8 rounded-[3px] px-3 text-[12px] font-medium transition-colors cursor-pointer inline-flex items-center gap-1.5 select-none whitespace-nowrap shrink-0 ${
            hwActive || isTogglingBio
              ? 'border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-tertiary)] opacity-50 cursor-not-allowed'
              : bioActive
                ? 'border border-[var(--destructive)] bg-transparent text-[var(--destructive)] hover:bg-[var(--destructive)]/10'
                : 'border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-primary)] hover:bg-[var(--bg-hover)]'
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
      </SettingRow>

      {/* Hardware 2FA / YubiKey */}
      <SettingRow
        label={t('settings.yubikey_title')}
        description={hwActive ? t('settings.yubikey_enrolled') : t('settings.hardware_2fa_desc')}
        tooltip={t('settings.tooltip_hardware_2fa')}
      >
        <div className="flex gap-2 shrink-0">
          {hwActive ? (
            <>
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
                className="h-8 rounded-[3px] border border-[var(--destructive)] bg-transparent px-3 text-[12px] font-medium text-[var(--destructive)] hover:bg-[var(--destructive)]/10 transition-colors cursor-pointer whitespace-nowrap shrink-0"
              >
                {t('common.disable')}
              </button>
            </>
          ) : (
            <button
              type="button"
              onClick={() => onOpenHwModal('enroll')}
              className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer whitespace-nowrap shrink-0"
            >
              {t('common.enable')}
            </button>
          )}
        </div>
      </SettingRow>


      {/* Emergency Recovery */}
      <SettingSection
        label={t('settings.emergency_recovery')}
        tooltip={t('settings.tooltip_emergency_recovery')}
      >
        <p className="mb-2.5 text-[12px] text-[var(--text-secondary)]">
          {t('settings.emergency_recovery_desc')}
        </p>
        <div className="flex flex-col gap-3 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3.5">
          {/* Status & Audit Bar */}
          <div className="flex flex-wrap items-center justify-between gap-2 pb-2.5 border-b border-[var(--border-subtle)]">
            <div className="flex flex-col gap-0.5">
              <div className="flex items-center gap-1.5">
                <ShieldCheck size={14} className={audit?.active_fingerprint ? 'text-[var(--text-primary)]' : 'text-[var(--text-tertiary)]'} />
                <span className="text-[12px] font-medium text-[var(--text-primary)]">
                  {audit?.active_fingerprint
                    ? t('settings.emergency_status_active', { fingerprint: audit.active_fingerprint })
                    : t('settings.emergency_status_none')}
                </span>
              </div>
              {audit?.last_generated_at && (
                <span className="text-[10px] text-[var(--text-tertiary)]">
                  {t('settings.emergency_last_generated', { date: new Date(audit.last_generated_at).toLocaleString() })}
                </span>
              )}
            </div>

            <div className="flex items-center gap-2">
              {audit?.history && audit.history.length > 0 && (
                <button
                  type="button"
                  onClick={() => setShowAuditLogs(!showAuditLogs)}
                  className="flex items-center gap-1 h-6.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2 text-[10.5px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
                >
                  <History size={11} />
                  {t('settings.emergency_audit_toggle', { count: audit.history.length })}
                </button>
              )}
              {audit?.active_fingerprint && (
                <button
                  type="button"
                  disabled={isResettingAudit}
                  onClick={async () => {
                    if (!backend) return;
                    setIsResettingAudit(true);
                    try {
                      await backend.resetEmergencyKitAudit();
                      const updated = await backend.getEmergencyKitAudit();
                      setAudit(updated);
                      setEmergencyKit(null);
                      addToast({ message: t('toast.emergency_kit_reset'), type: 'success' });
                    } catch (err) {
                      addToast({ message: String(err), type: 'error' });
                    } finally {
                      setIsResettingAudit(false);
                    }
                  }}
                  className="flex items-center gap-1 h-6.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2 text-[10.5px] font-medium text-[var(--text-secondary)] hover:text-red-400 hover:border-red-500/30 transition-colors cursor-pointer disabled:opacity-50"
                >
                  {isResettingAudit ? <Loader2 size={11} className="animate-spin" /> : <RotateCcw size={11} />}
                  {t('settings.reset_kit_button')}
                </button>
              )}
            </div>
          </div>

          {/* Audit Logs Drawer */}
          {showAuditLogs && audit?.history && audit.history.length > 0 && (
            <div className="flex flex-col gap-1.5 rounded-[3px] bg-[var(--bg-base)] p-2.5 border border-[var(--border-subtle)] text-[10px]">
              <div className="flex items-center justify-between text-[var(--text-tertiary)] font-semibold uppercase tracking-wider pb-1 border-b border-[var(--border-subtle)]">
                <span>{t('settings.emergency_audit_title')}</span>
                <span className="normal-case font-normal text-[9px]">{t('settings.emergency_audit_desc')}</span>
              </div>
              <div className="flex flex-col gap-1 max-h-36 overflow-y-auto pr-1">
                {audit.history.slice().reverse().map((entry, idx) => (
                  <div key={idx} className="flex items-center justify-between py-1 border-b border-[var(--border-subtle)] last:border-b-0 font-mono">
                    <span className="text-[var(--text-tertiary)]">
                      {new Date(entry.timestamp).toLocaleString()}
                    </span>
                    <span className="text-[var(--text-secondary)]">
                      {entry.fingerprint !== '-' ? `#${entry.fingerprint}` : '—'}
                    </span>
                    <span className="text-[var(--text-primary)] uppercase text-[9px] font-sans">
                      {entry.action === 'generated'
                        ? t('settings.emergency_action_generated')
                        : entry.action === 'reset'
                        ? t('settings.emergency_action_reset')
                        : t('settings.emergency_action_invalidated')}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Generator Input */}
          <div className="flex flex-col gap-1.5">
            <div className="flex gap-2">
              <input
                type="password"
                placeholder={t('settings.verify_master_placeholder')}
                value={shamirPass}
                onChange={(e) => {
                  setShamirPass(e.target.value);
                  if (passError) setPassError(false);
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !isGeneratingKit && shamirPass) {
                    handleGenerateKit();
                  }
                }}
                className={`h-8 flex-1 rounded-[3px] border bg-[var(--bg-base)] px-2.5 text-[12px] placeholder:text-[12px] placeholder:text-[var(--text-tertiary)] text-[var(--text-primary)] outline-none transition-colors ${
                  passError
                    ? 'border-[var(--destructive)] focus:border-[var(--destructive)]'
                    : 'border-[var(--border)] focus:border-[var(--border-focus)]'
                }`}
              />
              <button
                disabled={isGeneratingKit || !shamirPass}
                onClick={handleGenerateKit}
                className="h-8 flex items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed shrink-0"
              >
                {isGeneratingKit ? <Loader2 size={13} className="animate-spin" /> : <FileText size={13} />}
                {audit?.active_fingerprint ? t('settings.generate_new_emergency_kit') : t('settings.generate_emergency_kit')}
              </button>
            </div>
            {passError && (
              <span className="text-[11px] text-[var(--destructive)] font-medium">
                {t('error.invalid_password')}
              </span>
            )}
          </div>

          {/* Active Generated Kit Box */}
          {emergencyKit && (
            <div className="flex flex-col gap-2 mt-1 border-t border-[var(--border-subtle)] pt-2.5">
              <div className="flex items-center justify-between">
                <span className="text-[12px] font-semibold text-[var(--text-primary)]">
                  {t('settings.emergency_kit_sheet_title', {
                    vault: emergencyKit.vault_name,
                    threshold: 2,
                    total: emergencyKit.shares.length,
                  })}
                </span>
                <span className="text-[10px] text-[var(--text-tertiary)] font-mono">
                  #{emergencyKit.verification_hash}
                </span>
              </div>

              <div className="flex flex-wrap gap-2 mt-0.5">
                <button
                  type="button"
                  onClick={() => {
                    if (!emergencyKit) return;
                    const blob = new Blob([emergencyKit.document_markdown], { type: 'text/markdown;charset=utf-8' });
                    const url = URL.createObjectURL(blob);
                    const a = document.createElement('a');
                    a.href = url;
                    const cleanName = emergencyKit.vault_name.replace(/\.vdb$/i, '');
                    a.download = `yntra-emergency-kit-${cleanName}.md`;
                    a.click();
                    URL.revokeObjectURL(url);
                    addToast({ message: t('toast.sheet_downloaded'), type: 'success' });
                  }}
                  className="flex items-center gap-1.5 h-7 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[11px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                >
                  <Download size={12} />
                  {t('settings.download_sheet_md')}
                </button>

                <button
                  type="button"
                  onClick={() => setShowKitSheet(!showKitSheet)}
                  className="flex items-center gap-1 h-7 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 text-[11px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors cursor-pointer ml-auto"
                >
                  {showKitSheet ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
                  {showKitSheet ? t('settings.hide_preview') : t('settings.show_preview')}
                </button>
              </div>

              {/* Emergency Shares without green badges or preachy notices */}
              <div className="flex flex-col gap-1.5 mt-1">
                {emergencyKit.shares.map((s) => (
                  <div key={s.share_index} className="flex flex-col gap-1 rounded-[3px] bg-[var(--bg-base)] p-2 border border-[var(--border-subtle)]">
                    <div className="flex items-center justify-between">
                      <span className="text-[11px] font-medium text-[var(--text-primary)]">
                        {t('settings.share_part', { index: s.share_index })}
                      </span>
                      <button
                        onClick={() => {
                          if (isTauri()) {
                            backend?.copyToClipboard(s.share_data, true, 30).catch(() => {});
                          } else {
                            navigator.clipboard.writeText(s.share_data).catch(() => {});
                          }
                          addToast({ message: t('toast.share_copied', { index: s.share_index }), type: 'success' });
                        }}
                        className="text-[10px] font-medium text-[var(--text-primary)] hover:underline cursor-pointer"
                      >
                        {t('common.copy')}
                      </button>
                    </div>
                    <span className="font-mono text-[10px] text-[var(--text-secondary)] select-all truncate">
                      {s.share_data}
                    </span>
                  </div>
                ))}
              </div>

              {/* Collapsible Sheet Preview */}
              {showKitSheet && (
                <pre className="mt-2 max-h-48 overflow-y-auto rounded-[3px] bg-[var(--bg-base)] p-2.5 font-mono text-[10px] text-[var(--text-secondary)] whitespace-pre-wrap border border-[var(--border)] leading-relaxed select-all">
                  {emergencyKit.document_markdown}
                </pre>
              )}
            </div>
          )}
        </div>
      </SettingSection>
    </div>
  );
}

export default SecurityTab;
