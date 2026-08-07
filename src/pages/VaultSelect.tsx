import { useState, useEffect } from 'react';
import { motion } from 'framer-motion';
import { Database, Plus, Download, Clock, AlertTriangle, Trash2 } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { useAppState } from '@/contexts/AppStateContext';
import { useTranslation } from '@/contexts/LanguageContext';
import CreateVaultModal from '@/components/CreateVaultModal';
import { isTauri, getBackend } from '@/lib/backend';
import type { Vault } from '@/types';
import { ActionTooltip } from '@/components/ui/tooltip';

export default function VaultSelect() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const { setCurrentVault, setIsLocked } = useAppState();
  const [showCreate, setShowCreate] = useState(false);
  const [recentVaults, setRecentVaults] = useState<Vault[]>([]);
  const [missingVaults, setMissingVaults] = useState<Set<string>>(new Set());

  // Load recent vaults from localStorage
  useEffect(() => {
    try {
      const saved = JSON.parse(localStorage.getItem('yntra-vault-recent-vaults') || '[]');
      setRecentVaults(saved);
    } catch {
      setRecentVaults([]);
    }
  }, []);

  // Check which vault files exist
  useEffect(() => {
    const checkFiles = async () => {
      if (!isTauri() || recentVaults.length === 0) return;
      try {
        const backend = await getBackend();
        const missing = new Set<string>();
        for (const vault of recentVaults) {
          try {
            const fileExists = await backend.checkVaultFileExists(vault.path);
            if (!fileExists) {
              missing.add(vault.id);
            }
          } catch {
            missing.add(vault.id);
          }
        }
        setMissingVaults(missing);
      } catch (e) {
        console.error('File check failed:', e);
      }
    };
    checkFiles();
  }, [recentVaults]);

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
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        title: 'Open Vault File',
        filters: [{ name: 'Yntra Vault', extensions: ['vdb', 'db'] }],
        multiple: false,
      });
      if (selected) {
        const path = typeof selected === 'string' ? selected : selected;
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
      className="flex h-screen w-screen items-center justify-center bg-[var(--bg-base)]"
    >
      <div className="w-[420px] px-6">
        {/* Header */}
        <div className="flex flex-col items-center gap-3">
          <img
            src="/white-logo.png"
            alt="Yntra Vault Logo"
            className="h-24 w-24 rounded-xl object-cover"
          />
          <h1 className="text-[20px] font-semibold tracking-tight text-[var(--text-primary)]">
            {t('vault_select.title')}
          </h1>
          <p className="text-[13px] text-[var(--text-secondary)]">
            {t('vault_select.subtitle')}
          </p>
        </div>

        {!isTauri() && (
          <div className="mt-6 flex flex-col gap-2 rounded-[3px] border border-amber-500/20 bg-amber-500/10 p-3 text-[12px] text-amber-400">
            <div className="flex items-center gap-2 font-medium">
              <AlertTriangle size={14} className="shrink-0 animate-pulse" />
              <span>{t('vault_select.web_warning_title')}</span>
            </div>
            <p className="text-[11px] leading-relaxed text-amber-500/80 dark:text-amber-400/80">
              {t('vault_select.web_warning_desc')}
              <code className="mt-1.5 block rounded border border-amber-500/20 bg-black/30 px-2 py-1 font-mono text-[10px] text-amber-300">
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
                            <span className="inline-flex items-center gap-1 rounded bg-red-500/10 px-1.5 py-0.5 text-[10px] font-medium text-red-400 border border-red-500/20">
                              <AlertTriangle size={10} />
                              {t('vault_select.file_not_found')}
                            </span>
                          </ActionTooltip>
                        )}
                      </div>
                      <div className="truncate text-[12px] text-[var(--text-tertiary)]">{vault.path}</div>
                    </div>
                  </button>

                  <ActionTooltip content={t('vault_select.remove_recent')}>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        removeRecent(vault.id);
                      }}
                      className="shrink-0 flex h-8 w-8 items-center justify-center rounded-[3px] text-[var(--text-tertiary)] hover:bg-red-500/10 hover:text-red-400 transition-colors"
                    >
                      <Trash2 size={14} />
                    </button>
                  </ActionTooltip>
                </motion.div>
              ))}
            </div>
          </div>
        )}

        {/* Actions */}
        <div className={`flex gap-2 ${recentVaults.length > 0 ? 'mt-4' : 'mt-8'}`}>
          <ActionTooltip content="Create a new encrypted vault file" side="top">
            <button
              onClick={() => setShowCreate(true)}
              disabled={!isTauri()}
              className="flex h-10 w-full items-center justify-center gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-[var(--bg-elevated)]"
            >
              <Plus size={15} />
              {t('vault_select.new_vault')}
            </button>
          </ActionTooltip>
          <ActionTooltip content="Open an existing vault file (.vdb)" side="top">
            <button
              onClick={handleImport}
              disabled={!isTauri()}
              className="flex h-10 w-full items-center justify-center gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-[var(--bg-elevated)]"
            >
              <Download size={15} />
              {t('vault_select.open_file')}
            </button>
          </ActionTooltip>
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
    </motion.div>
  );
}



