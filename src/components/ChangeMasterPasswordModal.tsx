/**
 * ChangeMasterPasswordModal — Secure master password change
 */

import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Loader2, ShieldCheck, KeyRound, FolderOpen } from 'lucide-react';
import { PasswordStrength } from './PasswordStrength';
import { useAppState } from '@/contexts/AppStateContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { isTauri, getBackend } from '@/lib/backend';
import { ActionTooltip } from './ui/tooltip';

interface ChangeMasterPasswordModalProps {
  open: boolean;
  onClose: () => void;
}

export default function ChangeMasterPasswordModal({ open, onClose }: ChangeMasterPasswordModalProps) {
  const { t } = useTranslation();
  const { addToast } = useAppState();
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [showPasswords, setShowPasswords] = useState(false);

  // Key File states
  const [useCurrentKeyFile, setUseCurrentKeyFile] = useState(false);
  const [currentKeyFile, setCurrentKeyFile] = useState('');
  const [useNewKeyFile, setUseNewKeyFile] = useState(false);
  const [newKeyFile, setNewKeyFile] = useState('');

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setCurrentPassword('');
      setNewPassword('');
      setConfirmPassword('');
      setError(null);
      setShowPasswords(false);
      setUseCurrentKeyFile(false);
      setCurrentKeyFile('');
      setUseNewKeyFile(false);
      setNewKeyFile('');
    }
  }, [open]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open) onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onClose]);

  const handleBrowseKeyFile = async (setter: (val: string) => void) => {
    if (!isTauri()) return;
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        title: 'Select Key File',
        multiple: false,
        filters: [{ name: 'Key File (*.key, *.*)', extensions: ['key', '*'] }],
      });
      if (selected) {
        setter(typeof selected === 'string' ? selected : String(selected));
      }
    } catch (e) {
      console.error('Browse failed:', e);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!currentPassword) { setError('Enter your current password'); return; }
    if (useCurrentKeyFile && !currentKeyFile.trim()) { setError('Please select current Key File'); return; }
    if (newPassword.length < 12) { setError('New password must be at least 12 characters'); return; }
    if (newPassword !== confirmPassword) { setError('Passwords do not match'); return; }
    if (useNewKeyFile && !newKeyFile.trim()) { setError('Please select new Key File location'); return; }

    setLoading(true);
    try {
      if (isTauri()) {
        const backend = await getBackend();
        await backend.changeMasterPassword(
          currentPassword,
          newPassword,
          useCurrentKeyFile && currentKeyFile.trim() ? currentKeyFile.trim() : undefined,
          useNewKeyFile && newKeyFile.trim() ? newKeyFile.trim() : undefined,
        );
      }
      addToast({ message: 'Master password changed successfully', type: 'success' });
      onClose();
    } catch (err: any) {
      setError(err?.toString() || 'Failed to change password');
    } finally {
      setLoading(false);
      setCurrentPassword('');
      setNewPassword('');
      setConfirmPassword('');
      setUseCurrentKeyFile(false);
      setCurrentKeyFile('');
      setUseNewKeyFile(false);
      setNewKeyFile('');
    }
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
          onClick={onClose}
        >
          <motion.div
            initial={{ scale: 0.96, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.96, opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="w-[420px] rounded-lg border border-[var(--border)] bg-[var(--bg-base)] shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-5 py-3.5">
              <div className="flex items-center gap-2">
                <ShieldCheck size={18} className="text-[var(--text-primary)]" />
                <h2 className="text-[16px] font-semibold text-[var(--text-primary)]">{t('cmp.title')}</h2>
              </div>
              <ActionTooltip content={t('common.close')}>
                <button onClick={onClose} className="rounded-md p-1 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)]">
                  <X size={16} />
                </button>
              </ActionTooltip>
            </div>

            <form onSubmit={handleSubmit} className="flex flex-col gap-4 p-5 max-h-[80vh] overflow-y-auto">
              <PasswordField
                label={t('cmp.current_pass')}
                value={currentPassword}
                onChange={setCurrentPassword}
                show={showPasswords}
                placeholder={t('cmp.current_pass_ph')}
              />

              {/* Current Key File */}
              <div className="flex flex-col gap-1.5">
                <label className="flex items-center gap-2 cursor-pointer select-none text-[12px] font-medium text-[var(--text-secondary)]">
                  <input
                    type="checkbox"
                    checked={useCurrentKeyFile}
                    onChange={(e) => setUseCurrentKeyFile(e.target.checked)}
                    className="rounded border-[var(--border)] accent-[var(--accent)]"
                  />
                  <KeyRound size={13} className="text-[var(--text-tertiary)]" />
                  <span>{t('cmp.req_keyfile')}</span>
                </label>
                {useCurrentKeyFile && (
                  <div className="flex gap-1.5">
                    <input
                      type="text"
                      value={currentKeyFile}
                      onChange={(e) => setCurrentKeyFile(e.target.value)}
                      placeholder={t('cmp.current_keyfile_ph')}
                      className="h-8 flex-1 rounded border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 font-mono text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                    />
                    {isTauri() && (
                      <ActionTooltip content={t('common.browse')}>
                        <button
                          type="button"
                          onClick={() => handleBrowseKeyFile(setCurrentKeyFile)}
                          className="flex h-8 items-center gap-1 rounded border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
                        >
                          <FolderOpen size={12} />
                          {t('common.browse')}
                        </button>
                      </ActionTooltip>
                    )}
                  </div>
                )}
              </div>

              <div className="h-[1px] bg-[var(--border-subtle)] my-1" />

              <PasswordField
                label={t('cmp.new_pass')}
                value={newPassword}
                onChange={setNewPassword}
                show={showPasswords}
                placeholder={t('cmp.min_chars')}
              />
              {newPassword && <PasswordStrength password={newPassword} compact />}

              <PasswordField
                label={t('cmp.confirm_new_pass')}
                value={confirmPassword}
                onChange={setConfirmPassword}
                show={showPasswords}
                placeholder={t('cmp.reenter_pass_ph')}
                mismatch={!!confirmPassword && confirmPassword !== newPassword}
              />

              {/* New Key File */}
              <div className="flex flex-col gap-1.5">
                <label className="flex items-center gap-2 cursor-pointer select-none text-[12px] font-medium text-[var(--text-secondary)]">
                  <input
                    type="checkbox"
                    checked={useNewKeyFile}
                    onChange={(e) => setUseNewKeyFile(e.target.checked)}
                    className="rounded border-[var(--border)] accent-[var(--accent)]"
                  />
                  <KeyRound size={13} className="text-[var(--text-tertiary)]" />
                  <span>{t('cmp.req_new_keyfile')}</span>
                </label>
                {useNewKeyFile && (
                  <div className="flex gap-1.5">
                    <input
                      type="text"
                      value={newKeyFile}
                      onChange={(e) => setNewKeyFile(e.target.value)}
                      placeholder={t('cmp.new_keyfile_ph')}
                      className="h-8 flex-1 rounded border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 font-mono text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                    />
                    {isTauri() && (
                      <ActionTooltip content={t('common.browse')}>
                        <button
                          type="button"
                          onClick={() => handleBrowseKeyFile(setNewKeyFile)}
                          className="flex h-8 items-center gap-1 rounded border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
                        >
                          <FolderOpen size={12} />
                          {t('common.browse')}
                        </button>
                      </ActionTooltip>
                    )}
                  </div>
                )}
              </div>

              <label className="flex items-center gap-2 text-[12px] text-[var(--text-secondary)]">
                <input
                  type="checkbox"
                  checked={showPasswords}
                  onChange={(e) => setShowPasswords(e.target.checked)}
                  className="accent-[var(--accent)] h-3.5 w-3.5"
                />
                {t('login.show_passwords')}
              </label>

              {error && (
                <div className="rounded-md bg-red-500/10 px-3 py-2 text-[12px] text-red-400">{error}</div>
              )}

              <div className="flex justify-end gap-2 pt-1">
                <button type="button" onClick={onClose} className="h-9 rounded-md px-4 text-[13px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]">
                  {t('common.cancel')}
                </button>
                <button type="submit" disabled={loading} className="flex h-9 items-center gap-2 rounded-md bg-[var(--text-primary)] px-4 text-[13px] font-semibold text-[var(--bg-base)] hover:opacity-90 disabled:opacity-50">
                  {loading ? <><Loader2 size={14} className="animate-spin" /> {t('cmp.changing')}</> : t('settings.change_password')}
                </button>
              </div>
            </form>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function PasswordField({ label, value, onChange, show, placeholder, mismatch }: {
  label: string; value: string; onChange: (v: string) => void;
  show: boolean; placeholder: string; mismatch?: boolean;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <label className="text-[12px] font-medium text-[var(--text-secondary)]">{label}</label>
      <input
        type={show ? 'text' : 'password'}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className={`h-9 rounded-md border bg-[var(--bg-elevated)] px-3 font-mono text-[13px] tracking-wide text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:tracking-normal placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] ${
          mismatch ? 'border-[var(--destructive)]' : 'border-[var(--border)]'
        }`}
      />
      {mismatch && <span className="text-[11px] text-[var(--destructive)]">Passwords do not match</span>}
    </div>
  );
}



