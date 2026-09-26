import { Sparkles, Download, RefreshCw, X, Info } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';
import type { CheckUpdateResult } from '@/types/ipc';

interface UpdateModalProps {
  isOpen: boolean;
  onClose: () => void;
  updateInfo: CheckUpdateResult | null;
  currentVersion: string;
  isDownloading: boolean;
  onInstall: () => void;
}

export function UpdateModal({
  isOpen,
  onClose,
  updateInfo,
  currentVersion,
  isDownloading,
  onInstall,
}: UpdateModalProps) {
  const { t } = useTranslation();

  if (!isOpen || !updateInfo) return null;

  const nativeInstall = ['android', 'windows-portable'].includes(updateInfo.target_platform);
  const available = Boolean(updateInfo.download_url);

  const pubDate = updateInfo.pub_date
    ? new Date(updateInfo.pub_date).toLocaleDateString(undefined, {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
      })
    : null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4 animate-in fade-in duration-200">
      <div className="relative w-full max-w-md overflow-hidden rounded-xl border border-[var(--border)] bg-[var(--bg-elevated)] shadow-2xl shadow-black/40 animate-in zoom-in-95 duration-200">
        {/* Glow accent */}
        <div className="absolute top-0 inset-x-0 h-1 bg-gradient-to-r from-emerald-500 via-teal-400 to-indigo-500" />

        {/* Close button */}
        <button
          onClick={onClose}
          aria-label={t('common.close')}
          disabled={isDownloading}
          className="absolute top-3.5 right-3.5 p-1 rounded-md text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors disabled:opacity-50 cursor-pointer"
        >
          <X className="w-4 h-4" />
        </button>

        <div className="p-6">
          {/* Header */}
          <div className="flex items-start gap-3.5 mb-4">
            <div className="w-10 h-10 rounded-lg bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center text-emerald-400 shrink-0">
              <Sparkles className="w-5 h-5" />
            </div>
            <div>
              <h2 className="text-base font-semibold text-[var(--text-primary)] flex items-center gap-2">
                {t('updater.modal_title') || 'New Update Available'}
              </h2>
              <p className="text-[12px] text-[var(--text-secondary)] mt-0.5">
                {t('updater.modal_subtitle') || 'A new version of Yntra Vault is ready to install.'}
              </p>
            </div>
          </div>

          {/* Version & Metadata Badges */}
          <div className="flex flex-wrap items-center gap-2 mb-4">
            <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-[var(--bg-base)] border border-[var(--border)] font-mono text-[12px]">
              <span className="text-[var(--text-tertiary)]">v{currentVersion}</span>
              <span className="text-[var(--text-tertiary)]">→</span>
              <span className="font-bold text-emerald-400">v{updateInfo.latest_version}</span>
            </div>

            {pubDate && (
              <span className="text-[11px] px-2 py-1 rounded-md bg-[var(--bg-base)] border border-[var(--border)] text-[var(--text-secondary)]">
                {pubDate}
              </span>
            )}

            <span className="text-[11px] px-2 py-1 rounded-md bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 font-medium capitalize">
              {updateInfo.target_platform.replace('-', ' ')}
            </span>
          </div>

          {/* Release Notes Preview */}
          {updateInfo.release_notes ? (
            <div className="mb-5">
              <span className="text-[11px] font-bold uppercase tracking-wider text-[var(--text-tertiary)] block mb-1.5">
                {t('updater.release_notes_label') || "What's New"}
              </span>
              <div className="max-h-48 overflow-y-auto rounded-lg border border-[var(--border)] bg-[var(--bg-base)] p-3 text-[12px] text-[var(--text-secondary)] font-mono leading-relaxed select-text space-y-1">
                {updateInfo.release_notes.split('\n').map((line, idx) => {
                  const isHeader = line.startsWith('#');
                  const isBullet = line.trim().startsWith('-') || line.trim().startsWith('*');
                  return (
                    <div
                      key={idx}
                      className={
                        isHeader
                          ? 'font-bold text-[var(--text-primary)] pt-1'
                          : isBullet
                          ? 'pl-2 text-[var(--text-primary)]'
                          : ''
                      }
                    >
                      {line}
                    </div>
                  );
                })}
              </div>
            </div>
          ) : (
            <div className="mb-5 p-3 rounded-lg border border-[var(--border)] bg-[var(--bg-base)] text-[12px] text-[var(--text-secondary)]">
              {t('updater.no_notes') || 'Includes latest security improvements and bug fixes.'}
            </div>
          )}

          {/* Describe the actual download path before any verification runs. */}
          <div className="flex items-center gap-2 text-[11px] text-[var(--text-tertiary)] mb-5 bg-[var(--bg-base)]/50 p-2 rounded-md border border-[var(--border)]/50">
            <Info className="w-3.5 h-3.5 shrink-0" />
            <span>
              {!available ? t('updater.platform_unavailable') : nativeInstall
                ? t('updater.checksum_before_install') : t('updater.browser_download')}
            </span>
          </div>

          {/* Action Buttons */}
          <div className="flex items-center justify-end gap-2.5">
            <button
              type="button"
              onClick={onClose}
              disabled={isDownloading}
              className="h-8 px-3.5 rounded-md border border-[var(--border)] bg-[var(--bg-base)] text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors disabled:opacity-50 cursor-pointer"
            >
              {t('updater.later_button') || 'Later'}
            </button>

            <button
              type="button"
              onClick={onInstall}
              disabled={isDownloading || !available}
              className="h-8 px-4 rounded-md bg-emerald-600 hover:bg-emerald-500 text-white text-[12px] font-medium shadow-md shadow-emerald-950/30 transition-all flex items-center gap-1.5 disabled:opacity-75 cursor-pointer"
            >
              {isDownloading ? (
                <>
                  <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                  <span>{t('updater.installing') || 'Downloading...'}</span>
                </>
              ) : (
                <>
                  <Download className="w-3.5 h-3.5" />
                  <span>{nativeInstall ? t('updater.download_install') : t('updater.download_button')}</span>
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
