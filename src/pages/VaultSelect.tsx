import { useState, useEffect } from 'react';
import { motion } from 'framer-motion';
import { Database, Plus, Download, Clock, AlertTriangle, Trash2, Wifi } from 'lucide-react';
import { useNavigate, useLocation } from 'react-router-dom';
import { useAuth, CreateVaultModal } from '@/features/auth';
import { useTranslation } from '@/contexts/LanguageContext';
import { isTauri, getBackend, openFileDialog } from '@/lib/backend';
import { DevicePairingWizard } from '@/features/sync';
import type { Vault } from '@/types';
import { ActionTooltip } from '@/components/ui/tooltip';

export default function VaultSelect() {
  const navigate = useNavigate();
  const location = useLocation();
  const { t } = useTranslation();
  const { setCurrentVault, setIsLocked } = useAuth();
  const [showCreate, setShowCreate] = useState(false);
  const [recentVaults, setRecentVaults] = useState<Vault[]>([]);
  const [missingVaults, setMissingVaults] = useState<Set<string>>(new Set());
  const [showPairing, setShowPairing] = useState(false);

  const manualSelect = location.state?.manualSelect === true;

  // Load recent vaults and auto-select most recent non-deleted vault
  useEffect(() => {
    const initVaults = async () => {
      let saved: Vault[] = [];
      try {
        saved = JSON.parse(localStorage.getItem('yntra-vault-recent-vaults') || '[]');
      } catch {
        saved = [];
      }

      setRecentVaults(saved);
      if (saved.length === 0) return;

      if (isTauri()) {
        try {
          const backend = await getBackend();
          const missing = new Set<string>();
          const validVaults: Vault[] = [];

          for (const vault of saved) {
            try {
              const fileExists = await backend.checkVaultFileExists(vault.path);
              if (fileExists) {
                validVaults.push(vault);
              } else {
                missing.add(vault.id);
              }
            } catch {
              missing.add(vault.id);
            }
          }

          setMissingVaults(missing);

          if (!manualSelect && validVaults.length > 0) {
            setCurrentVault(validVaults[0]);
            setIsLocked(true);
            navigate('/login', { replace: true });
          }
        } catch (e) {
          console.error('File check error:', e);
        }
      } else if (!manualSelect && saved.length > 0) {
        setCurrentVault(saved[0]);
        setIsLocked(true);
        navigate('/login', { replace: true });
      }
    };

    initVaults();
  }, [manualSelect, navigate, setCurrentVault, setIsLocked]);

  const handleSelect = (vault: Vault) => {
    setCurrentVault(vault);
    setIsLocked(true);
    navigate('/login');
  };

  const handleVaultCreated = (vault: Vault) => {
    setShowCreate(false);
    setCurrentVault(vault);
    setIsLocked(false);
    navigate('/app');
  };

  const handleImport = async () => {
    if (!isTauri()) return;
    try {
      const selected = await openFileDialog({
        title: 'Open Vault File',
        filters: [{ name: 'Yntra Vault', extensions: ['vdb', 'db'] }],
        multiple: false,
      });
      if (selected) {
        const path = typeof selected === 'string' ? selected : selected[0];
        const fileName = String(path).split(/[/\\]/).pop()?.replace(/\.[^.]+$/, '') || 'Vault';
        // Use a temporary ID for import. Upon successful login, Login.tsx will update
        // this with the real vault ID and save it to Recent list.
        const vault: Vault = { id: `temp-import-${crypto.randomUUID()}`, name: fileName, path: String(path) };
        handleSelect(vault);
      }
    } catch (e) {
      console.error('Import failed:', e);
    }
  };

  const removeRecent = (id: string) => {
    const updated = recentVaults.filter(v => v.id !== id);
    setRecentVaults(updated);
    localStorage.setItem('yntra-vault-recent-vaults', JSON.stringify(updated));
  };

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      className="flex min-h-dvh w-full justify-center bg-[var(--bg-base)] px-4 pt-[env(safe-area-inset-top,0px)] pb-[env(safe-area-inset-bottom,0px)] overflow-y-auto touch-pan-y overscroll-contain"
    >
      <div className="w-full max-w-[420px] py-6 my-auto">
        {/* Logo and App Title */}
        <div className="flex flex-col items-center select-none">
          <img
            src="/white-logo.png"
            alt="Yntra Vault"
            className="h-20 w-20 rounded-[3px] object-cover invert dark:invert-0"
          />
          <h1 className="mt-4 text-[20px] font-semibold tracking-tight text-[var(--text-primary)]">
            {t('vault_select.title')}
          </h1>
        </div>

        {!isTauri() && (
          <div className="mt-6 flex flex-col gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3 text-[12px] text-[var(--text-secondary)]">
            <div className="flex items-center gap-2 font-medium text-[var(--text-primary)]">
              <AlertTriangle size={14} className="shrink-0 text-[var(--text-secondary)]" />
              <span>{t('vault_select.web_warning_title')}</span>
            </div>
            <p className="text-[11px] leading-relaxed text-[var(--text-tertiary)]">
              {t('vault_select.web_warning_desc')}
              <code className="mt-1.5 block rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2 py-1 font-mono text-[10px] text-[var(--text-primary)] select-all">
                bun tauri dev
              </code>
            </p>
          </div>
        )}

        {/* Recent Vaults */}
        {recentVaults.length > 0 && (
          <div className="mt-8">
            <div className="mb-2 flex items-center gap-1.5 px-1">
              <Clock size={12} className="text-[var(--text-tertiary)]" />
              <span className="text-[11px] font-semibold uppercase tracking-wide text-[var(--text-tertiary)]">
                {t('vault_select.recent_vaults')}
              </span>
            </div>
            <div className="flex flex-col gap-1 max-h-[176px] overflow-y-auto pr-1">
              {recentVaults.map((vault, i) => (
                <motion.div
                  key={vault.id}
                  initial={{ opacity: 0, y: 4 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ duration: 0.15, delay: i * 0.05 }}
                  className="group flex h-14 items-center gap-3 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3.5 transition-colors hover:border-[var(--border-focus)] hover:bg-[var(--bg-hover)]"
                >
                  <button
                    onClick={() => {
                      if (!missingVaults.has(vault.id)) {
                        handleSelect(vault);
                      }
                    }}
                    disabled={missingVaults.has(vault.id)}
                    className="flex flex-1 items-center gap-3 min-w-0 text-left h-full disabled:opacity-60 disabled:cursor-not-allowed"
                  >
                    <Database size={18} className="shrink-0 text-[var(--text-secondary)]" />
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="truncate text-[14px] font-medium text-[var(--text-primary)]">
                          {vault.name}
                        </span>
                        {missingVaults.has(vault.id) && (
                          <ActionTooltip content={t('vault_select.file_not_found')}>
                            <span className="inline-flex items-center gap-1 rounded-[3px] bg-[var(--bg-base)] px-1.5 py-0.5 text-[10px] font-mono text-[var(--text-secondary)] border border-[var(--border)]">
                              <AlertTriangle size={10} />
                              {t('vault_select.file_not_found')}
                            </span>
                          </ActionTooltip>
                        )}
                      </div>
                      <div className="truncate text-[12px] text-[var(--text-tertiary)] select-text">{vault.path}</div>
                    </div>
                  </button>

                  <ActionTooltip content={t('vault_select.remove_recent')}>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        removeRecent(vault.id);
                      }}
                      className="shrink-0 flex h-8 w-8 items-center justify-center rounded-[3px] text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors"
                    >
                      <Trash2 size={14} />
                    </button>
                  </ActionTooltip>
                </motion.div>
              ))}
            </div>
          </div>
        )}

        {/* Empty State when no vaults are opened yet */}
        {recentVaults.length === 0 && (
          <div className="mt-8 mb-2 flex flex-col items-center justify-center rounded-[3px] border border-dashed border-[var(--border)] p-6 text-center bg-[var(--bg-elevated)]/30 select-none">
            <Database size={26} className="text-[var(--text-tertiary)] mb-2 opacity-70" />
            <p className="text-[13px] font-medium text-[var(--text-primary)]">
              {t('vault_select.no_recent_title') || 'No Vault Open'}
            </p>
            <p className="text-[11px] text-[var(--text-secondary)] mt-1 max-w-[280px] leading-relaxed">
              {t('vault_select.no_recent_desc') || 'Create a new local vault, open an existing .vdb file, or pair with your computer over Wi-Fi.'}
            </p>
          </div>
        )}

        {/* Actions */}
        <div className={`flex gap-2.5 ${recentVaults.length > 0 ? 'mt-4' : 'mt-4'}`}>
          <ActionTooltip content={t('vault_select.create_tooltip')} side="top">
            <button
              onClick={() => setShowCreate(true)}
              disabled={!isTauri()}
              className="flex h-10 w-full items-center justify-center gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-[var(--bg-elevated)] cursor-pointer"
            >
              <Plus size={15} />
              <span>{t('vault_select.new_vault')}</span>
            </button>
          </ActionTooltip>
          <ActionTooltip content={t('vault_select.open_tooltip')} side="top">
            <button
              onClick={handleImport}
              disabled={!isTauri()}
              className="flex h-10 w-full items-center justify-center gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-[var(--bg-elevated)] cursor-pointer"
            >
              <Download size={15} />
              <span>{t('vault_select.open_file')}</span>
            </button>
          </ActionTooltip>
        </div>

        {/* Divider with label */}
        <div className="relative my-3.5 flex items-center justify-center">
          <div className="absolute inset-0 flex items-center">
            <div className="w-full border-t border-[var(--border)]" />
          </div>
          <span className="relative bg-[var(--bg-base)] px-2.5 text-[10px] uppercase tracking-wider text-[var(--text-tertiary)] font-medium">
            {t('common.or') || 'or'}
          </span>
        </div>

        {/* Pairing / Quick Link Button */}
        <div>
          <button
            onClick={() => setShowPairing(true)}
            disabled={!isTauri()}
            className="flex h-10 w-full items-center justify-center gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
          >
            <Wifi size={15} />
            <span>{t('vault_select.pair_device')}</span>
          </button>
        </div>

        {/* Version */}
        <p className="mt-6 text-center text-[11px] text-[var(--text-tertiary)]">
          {t('vault_select.crypto_info')}
        </p>
      </div>

      <CreateVaultModal
        open={showCreate}
        onClose={() => setShowCreate(false)}
        onCreated={handleVaultCreated}
      />

      <DevicePairingWizard
        isOpen={showPairing}
        onClose={() => setShowPairing(false)}
        defaultRole="client"
        isAdoptMode={true}
        onSuccess={(stats) => {
          if (stats?.vault_path) {
            const fileName = stats.vault_path.split(/[/\\]/).pop()?.replace(/\.[^.]+$/, '') || 'Yntra Vault';
            const newVault: Vault = {
              id: `vault-${Date.now()}`,
              name: fileName,
              path: stats.vault_path,
            };
            const updated = [newVault, ...recentVaults.filter(v => v.path !== stats.vault_path)];
            setRecentVaults(updated);
            localStorage.setItem('yntra-vault-recent-vaults', JSON.stringify(updated));
            setCurrentVault(newVault);
            setIsLocked(false);
            navigate('/app');
          } else {
            window.location.reload();
          }
        }}
      />
    </motion.div>
  );
}



