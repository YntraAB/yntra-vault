import { useState, useMemo, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  X,
  Download,
  FileText,
  Image as ImageIcon,
  FileArchive,
  Paperclip,
  ZoomIn,
  ZoomOut,
  RotateCcw,
  Copy,
  Check,
  Search,
  File,
  Folder,
} from 'lucide-react';
import * as fflate from 'fflate';
import type { AttachmentInfo } from '@/types';
import { ActionTooltip } from './ui/tooltip';

interface AttachmentPreviewModalProps {
  open: boolean;
  onClose: () => void;
  attachment: AttachmentInfo | null;
  data: Uint8Array | null;
  onDownload?: () => void;
}

function formatBytes(bytes: number, decimals = 1): string {
  if (bytes === 0) return '0 Bytes';
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ['Bytes', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
}

function getFileCategory(mimeType: string = '', fileName: string = ''): 'image' | 'text' | 'zip' | 'other' {
  const mime = mimeType.toLowerCase();
  const ext = fileName.split('.').pop()?.toLowerCase() || '';

  if (mime.startsWith('image/') || ['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'bmp', 'ico'].includes(ext)) {
    return 'image';
  }
  if (
    mime.startsWith('text/') ||
    mime.includes('json') ||
    mime.includes('xml') ||
    mime.includes('javascript') ||
    ['txt', 'md', 'json', 'csv', 'log', 'xml', 'js', 'ts', 'tsx', 'jsx', 'rs', 'py', 'html', 'css', 'yaml', 'toml', 'sh', 'sql'].includes(ext)
  ) {
    return 'text';
  }
  if (mime.includes('zip') || ext === 'zip') {
    return 'zip';
  }
  return 'other';
}

export default function AttachmentPreviewModal({
  open,
  onClose,
  attachment,
  data,
  onDownload,
}: AttachmentPreviewModalProps) {
  const [zoom, setZoom] = useState(1);
  const [copied, setCopied] = useState(false);
  const [zipSearch, setZipSearch] = useState('');

  const category = useMemo(() => {
    if (!attachment) return 'other';
    return getFileCategory(attachment.mimeType || attachment.mime_type, attachment.name);
  }, [attachment]);

  // Object URL for images
  const imageUrl = useMemo(() => {
    if (category !== 'image' || !data) return null;
    const blob = new Blob([Uint8Array.from(data)], { type: attachment?.mimeType || 'image/png' });
    return URL.createObjectURL(blob);
  }, [category, data, attachment]);

  // Revoke object URL on unmount / change
  useEffect(() => {
    return () => {
      if (imageUrl) URL.revokeObjectURL(imageUrl);
    };
  }, [imageUrl]);

  // Decoded text content
  const textContent = useMemo(() => {
    if (category !== 'text' || !data) return '';
    try {
      return new TextDecoder('utf-8').decode(data);
    } catch {
      return 'Failed to decode text content';
    }
  }, [category, data]);

  // Parsed ZIP file entries
  const zipEntries = useMemo(() => {
    if (category !== 'zip' || !data) return [];
    try {
      const unzipped = fflate.unzipSync(data);
      const entries: { path: string; isDir: boolean; size: number }[] = [];

      for (const [path, unzippedData] of Object.entries(unzipped)) {
        const isDir = path.endsWith('/');
        entries.push({
          path,
          isDir,
          size: unzippedData.length,
        });
      }

      entries.sort((a, b) => {
        if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
        return a.path.localeCompare(b.path);
      });

      return entries;
    } catch (err) {
      console.error('Failed to parse zip archive:', err);
      return [];
    }
  }, [category, data]);

  const filteredZipEntries = useMemo(() => {
    if (!zipSearch.trim()) return zipEntries;
    const query = zipSearch.toLowerCase();
    return zipEntries.filter(e => e.path.toLowerCase().includes(query));
  }, [zipEntries, zipSearch]);

  const handleCopyText = () => {
    if (!textContent) return;
    navigator.clipboard.writeText(textContent);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  if (!open || !attachment) return null;

  return (
    <AnimatePresence>
      <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm">
        <motion.div
          initial={{ opacity: 0, scale: 0.96 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.96 }}
          transition={{ duration: 0.15 }}
          className="relative flex flex-col w-full max-w-3xl h-[80vh] rounded-xl border border-[var(--border)] bg-[var(--bg-elevated)] shadow-2xl overflow-hidden"
        >
          {/* Header */}
          <div className="flex items-center justify-between px-5 py-3 border-b border-[var(--border-subtle)] bg-[var(--bg-base)] shrink-0">
            <div className="flex items-center gap-3 min-w-0">
              {category === 'image' && <ImageIcon size={18} className="text-blue-400 shrink-0" />}
              {category === 'text' && <FileText size={18} className="text-emerald-400 shrink-0" />}
              {category === 'zip' && <FileArchive size={18} className="text-amber-400 shrink-0" />}
              {category === 'other' && <Paperclip size={18} className="text-indigo-400 shrink-0" />}

              <div className="flex flex-col min-w-0">
                <span className="truncate text-[14px] font-semibold text-[var(--text-primary)]">
                  {attachment.name}
                </span>
                <span className="text-[11px] text-[var(--text-tertiary)] font-mono">
                  {formatBytes(attachment.size)} • {attachment.mimeType || attachment.mime_type || 'Unknown Type'}
                </span>
              </div>
            </div>

            <div className="flex items-center gap-2 shrink-0">
              {category === 'text' && (
                <ActionTooltip content="Copy text">
                  <button
                    type="button"
                    onClick={handleCopyText}
                    className="flex items-center gap-1.5 rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 py-1 text-[12px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors"
                  >
                    {copied ? <Check size={13} className="text-green-400" /> : <Copy size={13} />}
                    {copied ? 'Copied' : 'Copy'}
                  </button>
                </ActionTooltip>
              )}

              {onDownload && (
                <ActionTooltip content="Download file">
                  <button
                    type="button"
                    onClick={onDownload}
                    className="flex items-center gap-1.5 rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 py-1 text-[12px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors"
                  >
                    <Download size={13} /> Download
                  </button>
                </ActionTooltip>
              )}

              <button
                type="button"
                onClick={onClose}
                className="rounded-md p-1.5 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors"
              >
                <X size={16} />
              </button>
            </div>
          </div>

          {/* Content Preview Area */}
          <div className="flex-1 min-h-0 overflow-auto p-4 bg-[var(--bg-base)]/50">
            {/* Image Preview */}
            {category === 'image' && imageUrl && (
              <div className="flex flex-col h-full items-center justify-center relative group">
                <div className="absolute top-2 right-2 z-10 flex items-center gap-1 bg-[var(--bg-elevated)]/90 backdrop-blur-md rounded-md p-1 border border-[var(--border)] shadow-md">
                  <button
                    type="button"
                    onClick={() => setZoom(prev => Math.min(prev + 0.25, 3))}
                    className="p-1 text-[var(--text-secondary)] hover:text-[var(--text-primary)] rounded"
                  >
                    <ZoomIn size={14} />
                  </button>
                  <span className="text-[11px] font-mono px-1 text-[var(--text-tertiary)]">
                    {Math.round(zoom * 100)}%
                  </span>
                  <button
                    type="button"
                    onClick={() => setZoom(prev => Math.max(prev - 0.25, 0.5))}
                    className="p-1 text-[var(--text-secondary)] hover:text-[var(--text-primary)] rounded"
                  >
                    <ZoomOut size={14} />
                  </button>
                  <button
                    type="button"
                    onClick={() => setZoom(1)}
                    className="p-1 text-[var(--text-secondary)] hover:text-[var(--text-primary)] rounded"
                  >
                    <RotateCcw size={14} />
                  </button>
                </div>

                <div className="flex-1 flex items-center justify-center w-full overflow-auto p-4">
                  <img
                    src={imageUrl}
                    alt={attachment.name}
                    style={{ transform: `scale(${zoom})`, transition: 'transform 0.15s ease-out' }}
                    className="max-h-full max-w-full object-contain rounded-md shadow-lg"
                  />
                </div>
              </div>
            )}

            {/* Text Preview */}
            {category === 'text' && (
              <div className="h-full flex flex-col rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-elevated)] overflow-hidden font-mono text-[12.5px]">
                <div className="overflow-auto p-4 space-y-1 text-[var(--text-primary)] selection:bg-indigo-500/30">
                  <pre className="whitespace-pre-wrap break-words leading-relaxed font-mono">
                    {textContent}
                  </pre>
                </div>
              </div>
            )}

            {/* ZIP Archive Inspector */}
            {category === 'zip' && (
              <div className="h-full flex flex-col gap-3">
                <div className="flex items-center justify-between gap-3 bg-[var(--bg-elevated)] p-2.5 rounded-lg border border-[var(--border)]">
                  <div className="relative flex-1">
                    <Search size={14} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)]" />
                    <input
                      type="text"
                      value={zipSearch}
                      onChange={(e) => setZipSearch(e.target.value)}
                      placeholder="Search files in archive..."
                      className="h-8 w-full rounded-md border border-[var(--border)] bg-[var(--bg-base)] pl-8 pr-3 text-[12px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                    />
                  </div>
                  <span className="text-[11px] text-[var(--text-tertiary)] font-medium shrink-0">
                    {filteredZipEntries.length} items
                  </span>
                </div>

                <div className="flex-1 overflow-y-auto rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-elevated)] divide-y divide-[var(--border-subtle)]">
                  {filteredZipEntries.length > 0 ? (
                    filteredZipEntries.map((entry, idx) => (
                      <div key={idx} className="flex items-center justify-between px-3.5 py-2 hover:bg-[var(--bg-hover)] transition-colors text-[12px]">
                        <div className="flex items-center gap-2.5 min-w-0">
                          {entry.isDir ? (
                            <Folder size={14} className="text-amber-400 shrink-0" />
                          ) : (
                            <File size={14} className="text-[var(--text-tertiary)] shrink-0" />
                          )}
                          <span className="truncate font-mono text-[12px] text-[var(--text-primary)]">
                            {entry.path}
                          </span>
                        </div>
                        {!entry.isDir && (
                          <span className="text-[11px] text-[var(--text-tertiary)] font-mono shrink-0 ml-2">
                            {formatBytes(entry.size)}
                          </span>
                        )}
                      </div>
                    ))
                  ) : (
                    <div className="p-8 text-center text-[12px] text-[var(--text-tertiary)]">
                      {zipSearch ? 'No files match your search filter' : 'Archive is empty or unreadable'}
                    </div>
                  )}
                </div>
              </div>
            )}

            {/* Other / Generic Binary File */}
            {category === 'other' && (
              <div className="flex flex-col items-center justify-center h-full text-center gap-3 p-6">
                <div className="rounded-full bg-indigo-500/10 p-4 text-indigo-400 border border-indigo-500/20">
                  <Paperclip size={32} />
                </div>
                <div className="flex flex-col gap-1 max-w-sm">
                  <span className="text-[14px] font-semibold text-[var(--text-primary)]">
                    Preview Not Available
                  </span>
                  <span className="text-[12px] text-[var(--text-tertiary)]">
                    This file format ({attachment.name.split('.').pop()?.toUpperCase() || 'binary'}) cannot be rendered inline. Download the file to view it on your system.
                  </span>
                </div>
                {onDownload && (
                  <button
                    type="button"
                    onClick={onDownload}
                    className="mt-2 flex items-center gap-2 rounded-md bg-[var(--text-primary)] text-[var(--bg-base)] px-4 py-2 text-[12.5px] font-medium shadow-sm hover:opacity-90 transition-opacity"
                  >
                    <Download size={14} /> Download File ({formatBytes(attachment.size)})
                  </button>
                )}
              </div>
            )}
          </div>
        </motion.div>
      </div>
    </AnimatePresence>
  );
}
