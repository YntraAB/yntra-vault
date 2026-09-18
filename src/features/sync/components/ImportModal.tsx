import { useState, useMemo, useEffect, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  X,
  Upload,
  AlertTriangle,
  Search,
  ArrowRight,
  RefreshCw,
  FolderOpen,
  FolderInput,
  CheckCircle2,
  HelpCircle,
} from 'lucide-react';
import { useBackend } from '@/lib/useBackend';
import { useEntries } from '@/features/entries';
import { useToast } from '@/contexts/ToastContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { openFileDialog, type ParsedImportEntry, type ImportPreviewResult } from '@/lib/backend';
import { BRANDS, type BrandInfo, BrandLogo } from '../brands';

export interface ImportModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess?: () => void;
}

export function ImportModal({ isOpen, onClose, onSuccess }: ImportModalProps) {
  const { t } = useTranslation();
  const { backend } = useBackend();
  const { refreshEntries } = useEntries();
  const { addToast } = useToast();

  const [step, setStep] = useState<'brand' | 'file' | 'preview' | 'importing' | 'complete'>('brand');
  const [selectedBrand, setSelectedBrand] = useState<BrandInfo>(BRANDS[0]);

  const [isDragging, setIsDragging] = useState<boolean>(false);
  const [parsing, setParsing] = useState<boolean>(false);
  const [previewResult, setPreviewResult] = useState<ImportPreviewResult | null>(null);

  const [searchQuery, setSearchQuery] = useState<string>('');
  const [selectedEntries, setSelectedEntries] = useState<Record<number, boolean>>({});
  const [duplicateStrategy, setDuplicateStrategy] = useState<'skip' | 'overwrite' | 'keep_both'>('skip');

  const [importedCount, setImportedCount] = useState<number>(0);

  // Reset modal state on open
  const handleReset = useCallback(() => {
    setStep('brand');
    setPreviewResult(null);
    setSelectedEntries({});
    setSearchQuery('');
    setImportedCount(0);
    setIsDragging(false);
  }, []);

  const handleClose = useCallback(() => {
    handleReset();
    onClose();
  }, [handleReset, onClose]);

  // Close on Escape key press
  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        handleClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, handleClose]);

  const applyPreviewResult = useCallback((res: ImportPreviewResult) => {
    setPreviewResult(res);
    const initSelected: Record<number, boolean> = {};
    res.entries.forEach((_, idx) => {
      initSelected[idx] = true;
    });
    setSelectedEntries(initSelected);
    setStep('preview');
  }, []);

  const parseFile = useCallback(async (path: string) => {
    if (!backend) return;
    setParsing(true);
    try {
      const res = await backend.parseImportFile(path, selectedBrand.supportedFormatKey);
      applyPreviewResult(res);
    } catch (err) {
      addToast({ message: t('toast.parse_failed', { err: String(err) }), type: 'error' });
    } finally {
      setParsing(false);
    }
  }, [addToast, applyPreviewResult, backend, selectedBrand.supportedFormatKey, t]);

  const parseContent = useCallback(async (content: string) => {
    if (!backend) return;
    setParsing(true);
    try {
      const res = await backend.parseImportContent(content, selectedBrand.supportedFormatKey);
      applyPreviewResult(res);
    } catch (err) {
      addToast({ message: t('toast.parse_failed', { err: String(err) }), type: 'error' });
    } finally {
      setParsing(false);
    }
  }, [addToast, applyPreviewResult, backend, selectedBrand.supportedFormatKey, t]);

  const handleSelectFile = useCallback(async () => {
    try {
      const selected = await openFileDialog({
        multiple: false,
        filters: [
          {
            name: 'Password Export Files',
            extensions: ['json', 'csv', 'xml', 'txt', '1pux'],
          },
        ],
      });

      if (selected) {
        const filePath = typeof selected === 'string' ? selected : selected[0];
        if (filePath) {
          await parseFile(filePath);
        }
      }
    } catch (err) {
      console.error('File dialog error:', err);
    }
  }, [parseFile]);

  const handleDrop = async (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);

    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      const file = e.dataTransfer.files[0];
      if ('path' in file && typeof (file as any).path === 'string') {
        await parseFile((file as any).path);
      } else {
        const reader = new FileReader();
        reader.onload = (event) => {
          const text = event.target?.result as string;
          if (text) {
            parseContent(text);
          }
        };
        reader.readAsText(file);
      }
    }
  };

  const filteredPreviewEntries = useMemo(() => {
    if (!previewResult) return [];
    if (!searchQuery.trim()) return previewResult.entries;
    const q = searchQuery.toLowerCase().trim();
    return previewResult.entries.filter(
      (e) =>
        e.title.toLowerCase().includes(q) ||
        e.username.toLowerCase().includes(q) ||
        e.url.toLowerCase().includes(q)
    );
  }, [previewResult, searchQuery]);

  const toggleSelectAll = (check: boolean) => {
    if (!previewResult) return;
    const updated: Record<number, boolean> = {};
    previewResult.entries.forEach((_, idx) => {
      updated[idx] = check;
    });
    setSelectedEntries(updated);
  };

  const handleExecuteImport = async () => {
    if (!backend || !previewResult) return;
    setStep('importing');

    const toImport: ParsedImportEntry[] = previewResult.entries.filter((_, idx) => selectedEntries[idx]);

    try {
      const count = await backend.importEntries(toImport, duplicateStrategy);
      setImportedCount(count);
      await refreshEntries();
      setStep('complete');
      addToast({ message: t('toast.import_success', { count }), type: 'success' });
      if (onSuccess) onSuccess();
    } catch (err) {
      addToast({ message: t('toast.import_failed', { err: String(err) }), type: 'error' });
      setStep('preview');
    }
  };

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-50 flex items-start sm:items-center justify-center overflow-y-auto bg-black/60 select-none p-3 sm:p-4 touch-pan-y"
          onClick={handleClose}
        >
          <motion.div
            initial={{ scale: 0.97, opacity: 0, y: 6 }}
            animate={{ scale: 1, opacity: 1, y: 0 }}
            exit={{ scale: 0.97, opacity: 0, y: 6 }}
            transition={{ duration: 0.15, ease: 'easeOut' }}
            className="w-full max-w-[620px] my-auto rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] shadow-xl overflow-hidden flex flex-col max-h-[calc(100dvh-1.5rem)] sm:max-h-[85vh]"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-5 py-3.5 bg-[var(--bg-base)]">
              <div className="flex items-center gap-2.5">
                <div className="flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)]">
                  <FolderInput size={14} />
                </div>
                <div>
                  <h2 className="text-[14px] font-medium text-[var(--text-primary)] leading-tight">
                    {t('settings.importer_title') || 'Password Importer'}
                  </h2>
                  <p className="text-[11px] text-[var(--text-tertiary)]">
                    {t('onboarding.step_import_desc') || 'Migrate logins from Bitwarden, 1Password, KeePass, or Chrome in RAM'}
                  </p>
                </div>
              </div>

              <button
                type="button"
                onClick={handleClose}
                className="rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
              >
                <X size={15} />
              </button>
            </div>

        {/* Step Indicator Bar */}
        <div className="flex items-center justify-between border-b border-[var(--border-subtle)] bg-[var(--bg-base)]/50 px-6 py-2">
          {[
            { key: 'brand', label: 'Select Manager' },
            { key: 'file', label: 'Choose File' },
            { key: 'preview', label: 'Review & Import' },
          ].map((st, i) => {
            const isActive =
              step === st.key ||
              (st.key === 'file' && (step === 'file' || step === 'preview' || step === 'complete')) ||
              (st.key === 'preview' && (step === 'preview' || step === 'complete'));
            return (
              <div key={st.key} className="flex items-center gap-1.5 text-[11px] font-medium">
                <span
                  className={`flex h-4 w-4 items-center justify-center rounded-full text-[9px] ${
                    isActive ? 'bg-[var(--text-primary)] text-[var(--bg-base)] font-bold' : 'bg-[var(--border)] text-[var(--text-tertiary)]'
                  }`}
                >
                  {i + 1}
                </span>
                <span className={isActive ? 'text-[var(--text-primary)]' : 'text-[var(--text-tertiary)]'}>
                  {st.label}
                </span>
              </div>
            );
          })}
        </div>

        {/* Body Content */}
        <div className="p-5 overflow-y-auto flex-1 min-h-0 touch-pan-y">
          <AnimatePresence mode="wait">
            {/* STEP 1: BRAND SELECTION */}
            {step === 'brand' && (
              <motion.div
                key="step-brand"
                initial={{ opacity: 0, x: -10 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: 10 }}
                className="flex flex-col gap-4"
              >
                <div>
                  <h3 className="text-[13px] font-medium text-[var(--text-secondary)] mb-1">
                    Select source password manager:
                  </h3>
                  <div className="grid grid-cols-2 gap-2.5">
                    {BRANDS.map((b) => (
                      <button
                        key={b.id}
                        type="button"
                        onClick={() => {
                          setSelectedBrand(b);
                          setStep('file');
                        }}
                        className={`flex items-start gap-3 rounded-[3px] border p-3 text-left transition-all cursor-pointer ${
                          selectedBrand.id === b.id
                            ? 'border-[var(--border-focus)] bg-[var(--bg-active)] shadow-xs'
                            : 'border-[var(--border)] bg-[var(--bg-base)] hover:bg-[var(--bg-hover)]'
                        }`}
                      >
                        <div className="mt-0.5 shrink-0">
                          <BrandLogo brandId={b.id} className="h-7 w-7" />
                        </div>
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center justify-between">
                            <span className="text-[13px] font-medium text-[var(--text-primary)]">
                              {b.name}
                            </span>
                            <span className="rounded bg-[var(--bg-hover)] border border-[var(--border)] px-1.5 py-0.5 text-[9px] font-semibold text-[var(--text-tertiary)] uppercase tracking-wider">
                              {b.badge}
                            </span>
                          </div>
                          <p className="mt-1 text-[11px] text-[var(--text-tertiary)] line-clamp-2 leading-tight">
                            {b.description}
                          </p>
                        </div>
                      </button>
                    ))}
                  </div>
                </div>
              </motion.div>
            )}

            {/* STEP 2: FILE SELECTION & INSTRUCTIONS */}
            {step === 'file' && (
              <motion.div
                key="step-file"
                initial={{ opacity: 0, x: 10 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: -10 }}
                className="flex flex-col gap-4"
              >
                {/* Brand Banner */}
                <div className="flex items-center justify-between rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-3">
                  <div className="flex items-center gap-3">
                    <div className="shrink-0">
                      <BrandLogo brandId={selectedBrand.id} className="h-8 w-8" />
                    </div>
                    <div>
                      <h4 className="text-[13px] font-semibold text-[var(--text-primary)]">
                        {selectedBrand.name} Importer
                      </h4>
                      <p className="text-[11px] text-[var(--text-secondary)]">
                        Format: {selectedBrand.badge}
                      </p>
                    </div>
                  </div>
                  <button
                    type="button"
                    onClick={() => setStep('brand')}
                    className="text-[11px] font-medium text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
                  >
                    Change Source
                  </button>
                </div>

                {/* Instructions Card */}
                <div className="rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-3.5">
                  <div className="flex items-center gap-1.5 text-[12px] font-medium text-[var(--text-primary)] mb-2">
                    <HelpCircle size={14} className="text-[var(--text-tertiary)]" />
                    <span>How to export from {selectedBrand.name}:</span>
                  </div>
                  <ol className="list-decimal list-inside text-[11px] text-[var(--text-secondary)] space-y-1 pl-1">
                    {selectedBrand.instructions.map((inst, i) => (
                      <li key={i}>{inst}</li>
                    ))}
                  </ol>
                </div>

                {/* Dropzone / Select Button */}
                <div
                  onDragOver={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    setIsDragging(true);
                  }}
                  onDragLeave={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    setIsDragging(false);
                  }}
                  onDrop={handleDrop}
                  className={`flex flex-col items-center justify-center border-2 border-dashed rounded-[3px] p-6 text-center transition-all ${
                    isDragging
                      ? 'border-[var(--border-focus)] bg-[var(--bg-active)] shadow-md'
                      : 'border-[var(--border)] hover:border-[var(--border-focus)] bg-[var(--bg-base)]/60'
                  }`}
                >
                  <div className="mb-2 flex h-10 w-10 items-center justify-center rounded-full bg-[var(--bg-hover)] text-[var(--text-secondary)]">
                    <Upload size={18} />
                  </div>
                  <p className="text-[13px] font-medium text-[var(--text-primary)]">
                    {isDragging ? 'Drop file to parse' : 'Drag & drop file here or browse disk'}
                  </p>
                  <p className="mt-0.5 text-[11px] text-[var(--text-tertiary)]">
                    Supports .json, .csv, .xml format files
                  </p>

                  <button
                    type="button"
                    onClick={handleSelectFile}
                    disabled={parsing}
                    className="mt-4 flex h-8 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-4 text-[12px] font-semibold text-[var(--bg-base)] hover:opacity-90 transition-opacity cursor-pointer disabled:opacity-50"
                  >
                    {parsing ? <RefreshCw size={13} className="animate-spin" /> : <FolderOpen size={13} />}
                    <span>Browse File</span>
                  </button>
                </div>
              </motion.div>
            )}

            {/* STEP 3: PREVIEW & CONFLICT RESOLUTION */}
            {step === 'preview' && previewResult && (
              <motion.div
                key="step-preview"
                initial={{ opacity: 0, x: 10 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: -10 }}
                className="flex flex-col gap-3"
              >
                {/* Format Mismatch Auto-Recovery Alert */}
                {previewResult.is_format_mismatch && (
                  <div className="flex items-center justify-between rounded-[3px] border border-[var(--border)] bg-[var(--accent-bg)] p-2.5 text-[12px] text-[var(--text-secondary)]">
                    <div className="flex items-center gap-2">
                      <AlertTriangle size={15} className="shrink-0 text-[var(--text-secondary)]" />
                      <span>
                        Format Mismatch: You selected <strong>{selectedBrand.name}</strong>, but this file was auto-detected as <strong>{previewResult.format_detected}</strong>. We automatically parsed {previewResult.total_found} entries!
                      </span>
                    </div>
                  </div>
                )}

                {/* Empty / Invalid File Alert */}
                {previewResult.total_found === 0 && (
                  <div className="flex items-center gap-2.5 rounded-[3px] border border-[var(--border-subtle)] bg-[var(--bg-base)] p-3 text-[12px] text-[var(--text-secondary)] font-medium">
                    <AlertTriangle size={15} className="shrink-0 text-[var(--text-tertiary)]" />
                    <span>
                      No valid password entries found in this file. Please ensure the file is an unencrypted export from {selectedBrand.name} or try selecting a different manager.
                    </span>
                  </div>
                )}

                {/* Stats Summary Bar */}
                <div className="flex items-center justify-between rounded-[3px] border border-[var(--border-subtle)] bg-[var(--bg-base)] px-3 py-2 text-[12px]">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-[var(--text-primary)]">
                      Detected: {previewResult.format_detected}
                    </span>
                    <span className="rounded-[3px] bg-[var(--bg-elevated)] border border-[var(--border)] px-2 py-0.5 text-[10px] font-medium text-[var(--text-primary)]">
                      {previewResult.total_found} Items Found
                    </span>
                  </div>

                  {previewResult.duplicates_count > 0 && (
                    <div className="flex items-center gap-1 text-[var(--text-secondary)] text-[11px] font-medium">
                      <AlertTriangle size={13} className="text-[var(--text-tertiary)]" />
                      <span>{previewResult.duplicates_count} Duplicates Detected</span>
                    </div>
                  )}
                </div>

                {/* Duplicate Strategy & Controls */}
                <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
                  <div className="flex items-center gap-2 text-[12px]">
                    <span className="text-[var(--text-secondary)] font-medium">If Duplicate:</span>
                    <div className="flex rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)]">
                      {(
                        [
                          { id: 'skip', label: 'Skip' },
                          { id: 'overwrite', label: 'Overwrite' },
                          { id: 'keep_both', label: 'Keep Both' },
                        ] as const
                      ).map((opt) => (
                        <button
                          key={opt.id}
                          type="button"
                          onClick={() => setDuplicateStrategy(opt.id)}
                          className={`px-2.5 py-1 text-[11px] font-medium transition-colors cursor-pointer ${
                            duplicateStrategy === opt.id
                              ? 'bg-[var(--bg-active)] text-[var(--text-primary)] font-bold'
                              : 'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]'
                          }`}
                        >
                          {opt.label}
                        </button>
                      ))}
                    </div>
                  </div>

                  {/* Search Filter */}
                  <div className="relative">
                    <Search size={13} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)]" />
                    <input
                      type="text"
                      value={searchQuery}
                      onChange={(e) => setSearchQuery(e.target.value)}
                      placeholder={t('import.search_preview')}
                      className="h-7 w-48 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] pl-7 pr-2 text-[11px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                    />
                  </div>
                </div>

                {/* Table Header Select All */}
                <div className="flex items-center justify-between px-1 text-[11px] text-[var(--text-tertiary)]">
                  <div className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={
                        previewResult.entries.length > 0 &&
                        Object.values(selectedEntries).filter(Boolean).length === previewResult.entries.length
                      }
                      onChange={(e) => toggleSelectAll(e.target.checked)}
                      className="rounded border-[var(--border)] cursor-pointer"
                    />
                    <span className="font-medium text-[var(--text-secondary)]">
                      Selected {Object.values(selectedEntries).filter(Boolean).length} of {previewResult.entries.length} items
                    </span>
                  </div>

                  <button
                    type="button"
                    onClick={() => {
                      const allSelected =
                        Object.values(selectedEntries).filter(Boolean).length === previewResult.entries.length;
                      toggleSelectAll(!allSelected);
                    }}
                    className="text-[11px] font-medium text-[var(--text-primary)] hover:underline cursor-pointer"
                  >
                    {Object.values(selectedEntries).filter(Boolean).length === previewResult.entries.length
                      ? 'Deselect All'
                      : 'Select All'}
                  </button>
                </div>

                {/* Preview List Table */}
                <div className="max-h-[260px] overflow-y-auto rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] divide-y divide-[var(--border-subtle)]">
                  {filteredPreviewEntries.length === 0 ? (
                    <div className="py-8 text-center text-[12px] text-[var(--text-tertiary)]">
                      No matching items found
                    </div>
                  ) : (
                    filteredPreviewEntries.map((item, idx) => {
                      const origIndex = previewResult.entries.indexOf(item);
                      const isChecked = !!selectedEntries[origIndex];
                      return (
                        <div
                          key={idx}
                          onClick={() =>
                            setSelectedEntries((prev) => ({
                              ...prev,
                              [origIndex]: !prev[origIndex],
                            }))
                          }
                          className={`flex items-center justify-between px-3 py-2 text-[12px] transition-colors cursor-pointer ${
                            isChecked ? 'bg-[var(--bg-elevated)]' : 'opacity-60 hover:opacity-100'
                          }`}
                        >
                          <div className="flex items-center gap-2.5 min-w-0 flex-1">
                            <input
                              type="checkbox"
                              checked={isChecked}
                              onChange={() => {}}
                              className="rounded border-[var(--border)] cursor-pointer"
                            />
                            <div className="min-w-0 flex-1">
                              <div className="flex items-center gap-1.5 truncate">
                                <span className="font-medium text-[var(--text-primary)] truncate">
                                  {item.title || 'Untitled'}
                                </span>
                                {item.is_duplicate && (
                                  <span className="rounded-[3px] bg-[var(--bg-base)] border border-[var(--border)] px-1.5 py-0.5 text-[9px] font-mono text-[var(--text-secondary)] shrink-0">
                                    Duplicate
                                  </span>
                                )}
                              </div>
                              <div className="flex items-center gap-3 text-[10px] text-[var(--text-tertiary)] truncate">
                                {item.username && <span>User: {item.username}</span>}
                                {item.url && <span className="truncate">URL: {item.url}</span>}
                              </div>
                            </div>
                          </div>

                          <div className="flex items-center gap-1 shrink-0 text-[10px]">
                            {item.totp_secret && (
                              <span className="rounded-[3px] bg-[var(--bg-base)] border border-[var(--border)] px-1.5 py-0.5 font-mono text-[9px] text-[var(--text-secondary)]">
                                TOTP
                              </span>
                            )}
                            {item.notes && (
                              <span className="rounded-[3px] bg-[var(--bg-base)] border border-[var(--border)] px-1.5 py-0.5 font-mono text-[9px] text-[var(--text-tertiary)]">
                                Notes
                              </span>
                            )}
                          </div>
                        </div>
                      );
                    })
                  )}
                </div>
              </motion.div>
            )}

            {/* STEP 4: IMPORTING IN PROGRESS */}
            {step === 'importing' && (
              <motion.div
                key="step-importing"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="flex flex-col items-center justify-center py-10 text-center gap-3"
              >
                <RefreshCw size={28} className="animate-spin text-[var(--text-primary)]" />
                <h3 className="text-[14px] font-semibold text-[var(--text-primary)]">
                  Encrypting and importing entries...
                </h3>
                <p className="text-[11px] text-[var(--text-tertiary)]">
                  Applying multi-layer XChaCha20-Poly1305 encryption & zeroizing temporary RAM
                </p>
              </motion.div>
            )}

            {/* STEP 5: COMPLETE */}
            {step === 'complete' && (
              <motion.div
                key="step-complete"
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                className="flex flex-col items-center justify-center py-8 text-center gap-2"
              >
                <div className="flex h-10 w-10 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-surface)] text-[var(--text-primary)] mb-1">
                  <CheckCircle2 size={20} />
                </div>
                <h3 className="text-[16px] font-semibold text-[var(--text-primary)]">
                  Import Successful!
                </h3>
                <p className="text-[12px] text-[var(--text-secondary)]">
                  Successfully imported <strong className="text-[var(--text-primary)]">{importedCount}</strong> entries into your vault.
                </p>
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        {/* Footer Navigation Buttons */}
        <div className="flex items-center justify-between border-t border-[var(--border)] px-5 py-3 bg-[var(--bg-base)]">
          {step === 'file' ? (
            <button
              type="button"
              onClick={() => setStep('brand')}
              className="flex h-8 items-center gap-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
            >
              Back
            </button>
          ) : step === 'preview' ? (
            <button
              type="button"
              onClick={() => setStep('file')}
              className="flex h-8 items-center gap-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
            >
              Back
            </button>
          ) : (
            <div />
          )}

          {step === 'preview' && (
            <button
              type="button"
              onClick={handleExecuteImport}
              disabled={Object.values(selectedEntries).filter(Boolean).length === 0}
              className="flex h-8 items-center gap-1.5 rounded-[3px] bg-[var(--text-primary)] px-4 text-[12px] font-semibold text-[var(--bg-base)] hover:opacity-90 transition-opacity cursor-pointer disabled:opacity-50"
            >
              <span>
                Import {Object.values(selectedEntries).filter(Boolean).length} Items
              </span>
              <ArrowRight size={13} />
            </button>
          )}

          {step === 'complete' && (
            <button
              type="button"
              onClick={handleClose}
              className="flex h-8 items-center gap-1 rounded-[3px] bg-[var(--text-primary)] px-5 text-[12px] font-semibold text-[var(--bg-base)] hover:opacity-90 transition-opacity cursor-pointer ml-auto"
            >
              {t('common.done') || 'Done'}
            </button>
          )}
        </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

export default ImportModal;
