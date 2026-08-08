import { useState, useMemo, useEffect } from 'react';
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
import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
import { useBackend } from '@/lib/useBackend';
import { useAppState } from '@/contexts/AppStateContext';
import type { ParsedImportEntry, ImportPreviewResult } from '@/lib/backend';

export interface ImportModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess?: () => void;
}

export type CompetitorBrand =
  | 'auto_detect'
  | 'bitwarden'
  | 'onepassword'
  | 'keepass'
  | 'chrome'
  | 'lastpass'
  | 'dashlane'
  | 'protonpass'
  | 'generic';

function BrandLogo({ brandId, className = "h-5 w-5" }: { brandId: CompetitorBrand; className?: string }) {
  switch (brandId) {
    case 'auto_detect':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <rect width="24" height="24" rx="5" fill="#6366F1" />
          <path d="M12 4l1.8 3.6L18 9.4l-3 2.9.7 4.2-3.7-2-3.7 2 .7-4.2-3-2.9 4.2-1.8L12 4z" fill="#FFFFFF" />
        </svg>
      );
    case 'bitwarden':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <path d="M12 1.75L3.5 5.5v6.25c0 5.86 3.63 11 8.5 12.5 4.87-1.5 8.5-6.64 8.5-12.5V5.5L12 1.75z" fill="#175DDC" />
          <path d="M12 4.25v16.5c3.67-1.35 6.5-5.54 6.5-10.25V7.1L12 4.25z" fill="#1148AA" />
          <path d="M12 8.5a2.5 2.5 0 00-2.5 2.5v3h5v-3A2.5 2.5 0 0012 8.5z" fill="#FFFFFF" />
        </svg>
      );
    case 'onepassword':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <rect width="24" height="24" rx="5" fill="#0094F5" />
          <circle cx="12" cy="12" r="6" stroke="#FFFFFF" strokeWidth="2.2" fill="none" />
          <rect x="10.8" y="8" width="2.4" height="8" rx="1.2" fill="#FFFFFF" />
        </svg>
      );
    case 'keepass':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <rect width="24" height="24" rx="5" fill="#369336" />
          <path d="M7 12a3.5 3.5 0 116.12 2.3l.58.58v1.62h1.62v1.62H17v1.62h-1.62L13.1 17.5a3.5 3.5 0 01-6.1-5.5z" fill="#FFFFFF" />
          <circle cx="9.5" cy="10.5" r="1" fill="#369336" />
        </svg>
      );
    case 'chrome':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <circle cx="12" cy="12" r="10" fill="#4285F4" />
          <path d="M12 12L17.2 3.8A10 10 0 006.8 3.8L12 12z" fill="#EA4335" />
          <path d="M12 12L6.8 3.8A10 10 0 0012 22L17.2 12H12z" fill="#34A853" />
          <path d="M12 12H22A10 10 0 0017.2 3.8L12 12z" fill="#FBBC05" />
          <circle cx="12" cy="12" r="4.5" fill="#FFFFFF" />
          <circle cx="12" cy="12" r="3.5" fill="#1A73E8" />
        </svg>
      );
    case 'lastpass':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <rect width="24" height="24" rx="5" fill="#D32F2F" />
          <circle cx="7.5" cy="12" r="1.8" fill="#FFFFFF" />
          <path d="M13 8.5h4v7h-4z" fill="#FFFFFF" />
        </svg>
      );
    case 'dashlane':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <rect width="24" height="24" rx="5" fill="#0F7060" />
          <path d="M7 6.5h6a4.5 4.5 0 010 9H7v-9zm2.5 2.5v4h3.5a2 2 0 100-4h-3.5z" fill="#FFFFFF" />
        </svg>
      );
    case 'protonpass':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <rect width="24" height="24" rx="5" fill="#6D4AFF" />
          <path d="M12 5L6 8v4c0 4.2 2.7 8 6 9 3.3-1 6-4.8 6-9V8l-6-3z" fill="#FFFFFF" fillOpacity="0.25" />
          <circle cx="12" cy="10.5" r="2" fill="#FFFFFF" />
          <path d="M12 12.5v4" stroke="#FFFFFF" strokeWidth="2" strokeLinecap="round" />
        </svg>
      );
    case 'generic':
    default:
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <rect width="24" height="24" rx="5" fill="#4B5563" />
          <path d="M7 7h10v10H7V7zm2 2v2h2V9H9zm4 0v2h2V9h-2zm-4 4v2h2v-2H9zm4 0v2h2v-2h-2z" fill="#FFFFFF" fillOpacity="0.9" />
        </svg>
      );
  }
}

interface BrandInfo {
  id: CompetitorBrand;
  name: string;
  badge: string;
  description: string;
  instructions: string[];
  supportedFormatKey: string;
}

const BRANDS: BrandInfo[] = [
  {
    id: 'auto_detect',
    name: 'Smart Auto-Detect',
    badge: 'Recommended',
    description: 'Select or drop any export file (JSON, CSV, XML). Format is automatically recognized.',
    instructions: [
      'Export your vault from your existing password manager',
      'Drop the file directly into the dropzone or select it from disk',
      'Yntra Vault will automatically identify the format and structure',
    ],
    supportedFormatKey: 'auto',
  },
  {
    id: 'bitwarden',
    name: 'Bitwarden',
    badge: 'JSON / CSV',
    description: 'Export your vault as encrypted or unencrypted JSON or CSV from Bitwarden.',
    instructions: [
      'Open Bitwarden Web Vault or Desktop app',
      'Go to Settings → Export Vault',
      'Select JSON or CSV format and enter your master password',
      'Save the file to your computer and select it below',
    ],
    supportedFormatKey: 'bitwarden_json',
  },
  {
    id: 'onepassword',
    name: '1Password',
    badge: '1PUX / CSV',
    description: 'Export logins and items from 1Password 7 or 8 as CSV format.',
    instructions: [
      'Open 1Password on your desktop',
      'Click File → Export → All Items',
      'Choose CSV format and save the exported file',
      'Select the generated CSV file below',
    ],
    supportedFormatKey: 'onepassword_csv',
  },
  {
    id: 'keepass',
    name: 'KeePass / XC',
    badge: 'XML / CSV',
    description: 'Import database entries exported from KeePass 2.x or KeePassXC.',
    instructions: [
      'Open KeePassXC or KeePass 2.x',
      'Go to Database → Export → XML or CSV File',
      'Confirm export and save to disk',
      'Select the XML or CSV file below',
    ],
    supportedFormatKey: 'keepass_csv',
  },
  {
    id: 'chrome',
    name: 'Google Chrome / Edge',
    badge: 'Browser CSV',
    description: 'Import passwords saved in Chrome, Edge, Brave, or Firefox browsers.',
    instructions: [
      'Open Chrome/Edge Settings → Passwords',
      'Click the three dots next to Saved Passwords → Export passwords',
      'Confirm with your OS password and save CSV',
      'Select the browser CSV file below',
    ],
    supportedFormatKey: 'chrome_csv',
  },
  {
    id: 'lastpass',
    name: 'LastPass',
    badge: 'CSV Export',
    description: 'Import items exported from your LastPass vault.',
    instructions: [
      'Log into LastPass browser extension or website',
      'Go to Advanced Options → Export',
      'Enter Master Password and download CSV',
      'Select the LastPass CSV file below',
    ],
    supportedFormatKey: 'lastpass_csv',
  },
  {
    id: 'dashlane',
    name: 'Dashlane',
    badge: 'CSV Export',
    description: 'Import credentials exported from Dashlane web application.',
    instructions: [
      'Open Dashlane Web App → Settings → Export Data',
      'Choose CSV format and export',
      'Select the exported CSV file below',
    ],
    supportedFormatKey: 'dashlane_csv',
  },
  {
    id: 'protonpass',
    name: 'Proton Pass',
    badge: 'JSON / CSV',
    description: 'Import logins and notes exported from Proton Pass.',
    instructions: [
      'Open Proton Pass web or desktop application',
      'Go to Settings → Export Vault → JSON or CSV',
      'Select the exported file below',
    ],
    supportedFormatKey: 'protonpass_json',
  },
  {
    id: 'generic',
    name: 'Generic CSV',
    badge: 'Any CSV',
    description: 'Import from any password manager CSV with automatic column header detection.',
    instructions: [
      'Ensure CSV has column headers like Title, Username, Password, URL, Notes',
      'Save as .csv file',
      'Select the file below for auto-detection',
    ],
    supportedFormatKey: 'generic_csv',
  },
];

import { useTranslation } from '@/contexts/LanguageContext';

export default function ImportModal({ isOpen, onClose, onSuccess }: ImportModalProps) {
  const { t } = useTranslation();
  const { backend } = useBackend();
  const { refreshEntries, addToast } = useAppState();

  const [step, setStep] = useState<'brand' | 'file' | 'preview' | 'importing' | 'complete'>('brand');
  const [selectedBrand, setSelectedBrand] = useState<BrandInfo>(BRANDS[0]);

  const [isDragging, setIsDragging] = useState<boolean>(false);
  const [parsing, setParsing] = useState<boolean>(false);
  const [previewResult, setPreviewResult] = useState<ImportPreviewResult | null>(null);

  const [searchQuery, setSearchQuery] = useState<string>('');
  const [selectedEntries, setSelectedEntries] = useState<Record<number, boolean>>({});
  const [duplicateStrategy, setDuplicateStrategy] = useState<'skip' | 'overwrite' | 'keep_both'>('skip');

  const [importedCount, setImportedCount] = useState<number>(0);

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
  }, [isOpen]);

  // Reset modal state on open
  const handleReset = () => {
    setStep('brand');
    setPreviewResult(null);
    setSelectedEntries({});
    setSearchQuery('');
    setImportedCount(0);
    setIsDragging(false);
  };

  const handleClose = () => {
    handleReset();
    onClose();
  };

  const handleSelectFile = async () => {
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

      if (selected && typeof selected === 'string') {
        await parseFile(selected);
      }
    } catch (err) {
      console.error('File dialog error:', err);
    }
  };

  const parseFile = async (path: string) => {
    if (!backend) return;
    setParsing(true);
    try {
      const res = await backend.parseImportFile(path, selectedBrand.supportedFormatKey);
      applyPreviewResult(res);
    } catch (err) {
      addToast({ message: `Parse failed: ${err}`, type: 'error' });
    } finally {
      setParsing(false);
    }
  };

  const parseContent = async (content: string) => {
    if (!backend) return;
    setParsing(true);
    try {
      const res = await backend.parseImportContent(content, selectedBrand.supportedFormatKey);
      applyPreviewResult(res);
    } catch (err) {
      addToast({ message: `Parse failed: ${err}`, type: 'error' });
    } finally {
      setParsing(false);
    }
  };

  const applyPreviewResult = (res: ImportPreviewResult) => {
    setPreviewResult(res);
    const initSelected: Record<number, boolean> = {};
    res.entries.forEach((_, idx) => {
      initSelected[idx] = true;
    });
    setSelectedEntries(initSelected);
    setStep('preview');
  };

  const handleDrop = async (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);

    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      const file = e.dataTransfer.files[0];
      // Check if Tauri path property exists
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
      addToast({ message: `Successfully imported ${count} entries!`, type: 'success' });
      if (onSuccess) onSuccess();
    } catch (err) {
      addToast({ message: `Import failed: ${err}`, type: 'error' });
      setStep('preview');
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4 select-none">
      <motion.div
        initial={{ opacity: 0, scale: 0.96 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.96 }}
        className="w-full max-w-[620px] rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] shadow-2xl overflow-hidden flex flex-col max-h-[85vh]"
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-[var(--border)] px-5 py-4 bg-[var(--bg-base)]">
          <div className="flex items-center gap-2.5">
            <div className="flex h-8 w-8 items-center justify-center rounded-[3px] bg-blue-500/10 text-blue-500 border border-blue-500/20">
              <FolderInput size={16} />
            </div>
            <div>
              <h2 className="text-[15px] font-semibold text-[var(--text-primary)] tracking-tight">
                Competitor Password Importer
              </h2>
              <p className="text-[11px] text-[var(--text-tertiary)]">
                Migrate seamlessly from Bitwarden, 1Password, KeePass, Chrome & more
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={handleClose}
            className="flex h-7 w-7 items-center justify-center rounded-[3px] text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors"
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
        <div className="p-5 overflow-y-auto flex-1">
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
                  <div className="flex items-center justify-between rounded-[3px] border border-blue-500/30 bg-blue-500/10 p-2.5 text-[12px] text-blue-400">
                    <div className="flex items-center gap-2">
                      <AlertTriangle size={15} className="shrink-0 text-blue-400" />
                      <span>
                        Format Mismatch: You selected <strong>{selectedBrand.name}</strong>, but this file was auto-detected as <strong>{previewResult.format_detected}</strong>. We automatically parsed {previewResult.total_found} entries!
                      </span>
                    </div>
                  </div>
                )}

                {/* Empty / Invalid File Alert */}
                {previewResult.total_found === 0 && (
                  <div className="flex items-center gap-2.5 rounded-[3px] border border-red-500/30 bg-red-500/10 p-3 text-[12px] text-red-400 font-medium">
                    <AlertTriangle size={16} className="shrink-0" />
                    <span>
                      No valid password entries found in this file. Please ensure the file is an unencrypted export from {selectedBrand.name} or try selecting a different manager.
                    </span>
                  </div>
                )}

                {/* Stats Summary Bar */}
                <div className="flex items-center justify-between rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 py-2 text-[12px]">
                  <div className="flex items-center gap-2">
                    <span className="font-semibold text-[var(--text-primary)]">
                      Detected: {previewResult.format_detected}
                    </span>
                    <span className="rounded bg-blue-500/10 border border-blue-500/20 px-2 py-0.5 text-[10px] font-bold text-blue-500">
                      {previewResult.total_found} Items Found
                    </span>
                  </div>

                  {previewResult.duplicates_count > 0 && (
                    <div className="flex items-center gap-1 text-amber-500 text-[11px] font-medium">
                      <AlertTriangle size={13} />
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
                          className={`px-2.5 py-1 text-[11px] font-medium transition-colors ${
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
                    className="text-[11px] font-medium text-blue-500 hover:underline cursor-pointer"
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
                                  <span className="rounded bg-amber-500/10 border border-amber-500/20 px-1.5 py-0.2 text-[9px] font-bold text-amber-500 shrink-0">
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
                              <span className="rounded bg-emerald-500/10 border border-emerald-500/20 px-1.5 py-0.2 font-semibold text-emerald-500">
                                TOTP
                              </span>
                            )}
                            {item.notes && (
                              <span className="rounded bg-zinc-500/10 border border-zinc-500/20 px-1.5 py-0.2 text-[var(--text-tertiary)]">
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
                <div className="flex h-12 w-12 items-center justify-center rounded-full bg-green-500/10 border border-green-500/20 text-green-500 mb-1">
                  <CheckCircle2 size={24} />
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
              Done
            </button>
          )}
        </div>
      </motion.div>
    </div>
  );
}
