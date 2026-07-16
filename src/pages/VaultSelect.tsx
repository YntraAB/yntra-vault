import { useState, useEffect } from 'react';
import { motion } from 'framer-motion';
import { Database, Plus, Download, Clock, AlertTriangle, Trash2 } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { useAppState } from '@/contexts/AppStateContext';
import CreateVaultModal from '@/components/CreateVaultModal';
import { isTauri, getBackend } from '@/lib/backend';
import type { Vault } from '@/types';

export default function VaultSelect() {
  const navigate = useNavigate();
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

  const handleVaultCreated = (name: string, path: string) => {
    setShowCreate(false);
    const vault: Vault = { id: crypto.randomUUID(), name, path };
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
        filters: [{ name: 'Yntra Vault Vault', extensions: ['Yntra Vault', 'db'] }],
        multiple: false,
      });
      if (selected) {
        const path = typeof selected === 'string' ? selected : selected;
        const fileName = String(path).split(/[/\\]/).pop()?.replace(/\.[^.]+$/, '') || 'Vault';
        const vault: Vault = { id: crypto.randomUUID(), name: fileName, path: String(path) };
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
          <div className="flex h-10 w-10 items-center justify-center rounded-[3px] border border-[var(--border)]">
            <Database size={22} className="text-[var(--text-primary)]" />
          </div>
          <h1 className="text-[20px] font-semibold tracking-tight text-[var(--text-primary)]">
            Yntra Vault
          </h1>
          <p className="text-[13px] text-[var(--text-secondary)]">
            Secure password manager
          </p>
        </div>

        {!isTauri() && (
          <div className="mt-6 flex flex-col gap-2 rounded-[3px] border border-amber-500/20 bg-amber-500/10 p-3 text-[12px] text-amber-400">
            <div className="flex items-center gap-2 font-medium">
              <AlertTriangle size={14} className="shrink-0 animate-pulse" />
              <span>Web Browser Mode (Limited)</span>
            </div>
            <p className="text-[11px] leading-relaxed text-amber-500/80 dark:text-amber-400/80">
              Local vault files, imports, and cryptographic operations require running as a local desktop app. Please launch the app using:
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
                Recent Vaults
              </span>
            </div>
            <div className="flex flex-col gap-1">
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
                          <span className="inline-flex items-center gap-1 rounded bg-red-500/10 px-1.5 py-0.5 text-[10px] font-medium text-red-400 border border-red-500/20">
                            <AlertTriangle size={10} />
                            File Not Found
                          </span>
                        )}
                      </div>
                      <div className="truncate text-[12px] text-[var(--text-tertiary)]">{vault.path}</div>
                    </div>
                  </button>

                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      removeRecent(vault.id);
                    }}
                    className="shrink-0 flex h-8 w-8 items-center justify-center rounded-[3px] text-[var(--text-tertiary)] hover:bg-red-500/10 hover:text-red-400 transition-colors"
                    title="Remove from recent vaults"
                  >
                    <Trash2 size={14} />
                  </button>
                </motion.div>
              ))}
            </div>
          </div>
        )}

        {/* Actions */}
        <div className={`flex gap-2 ${recentVaults.length > 0 ? 'mt-4' : 'mt-8'}`}>
          <button
            onClick={() => setShowCreate(true)}
            disabled={!isTauri()}
            className="flex h-10 flex-1 items-center justify-center gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-[var(--bg-elevated)]"
          >
            <Plus size={15} />
            New Vault
          </button>
          <button
            onClick={handleImport}
            disabled={!isTauri()}
            className="flex h-10 flex-1 items-center justify-center gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-[var(--bg-elevated)]"
          >
            <Download size={15} />
            Open File
          </button>
        </div>

        {/* Version */}
        <p className="mt-6 text-center text-[11px] text-[var(--text-tertiary)]">
          Yntra Vault v0.1.0 — Encrypted with Argon2id + XChaCha20-Poly1305
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

