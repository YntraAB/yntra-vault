import { useState, useEffect, useCallback, Fragment } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Pencil,
  Trash2,
  User,
  Key,
  Link,
  Mail,
  FileText,
  ShieldCheck,
  Star,
  Pin,
  Eye,
  EyeOff,
} from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';
import CopyButton from './CopyButton';
import PasswordInput from './PasswordInput';
import { PasswordStrength } from './PasswordStrength';
import { BreachIndicator } from './BreachIndicator';
import type { BreachStatus } from '@/lib/backend';
import { useTotp } from '@/lib/useBackend';
import EntryModal from './EntryModal';
import Favicon from './Favicon';
import { formatDate, getFieldLayout } from '@/lib/utils';
import type { Tag } from '@/types';
import { Skeleton } from './ui/skeleton';



export default function PasswordDetail() {
  const { selectedEntry, setIsEditing, isEditing, deleteEntry, updateEntry, tags, togglePin, toggleFavorite, isLoadingDetail, addToast } = useAppState();
  const [editData, setEditData] = useState(selectedEntry);
  const [showDelConfirm, setShowDelConfirm] = useState(false);
  const [showEditModal, setShowEditModal] = useState(false);
  const [showRecovery, setShowRecovery] = useState(false);
  const [showTemporaryStats, setShowTemporaryStats] = useState(false);
  const [showPassword, setShowPassword] = useState(false);

  useEffect(() => {
    if (selectedEntry) {
      setEditData({ ...selectedEntry });
      setShowRecovery(false);
      setShowPassword(false);

      // Check if entry was newly created/updated (within 5 seconds)
      const ageMs = Date.now() - new Date(selectedEntry.updatedAt).getTime();
      if (ageMs < 5000) {
        setShowTemporaryStats(true);
        const timer = setTimeout(() => {
          setShowTemporaryStats(false);
        }, 3000);
        return () => clearTimeout(timer);
      } else {
        setShowTemporaryStats(false);
      }
    }
  }, [selectedEntry, isEditing]);

  const handleSave = useCallback(() => {
    if (editData) {
      updateEntry(editData);
      setIsEditing(false);
    }
  }, [editData, setIsEditing, updateEntry]);

  const data = isEditing && editData ? editData : selectedEntry;
  const entryTags = data
    ? (data.tags.map((t) => tags.find((tag) => tag.name === t)).filter(Boolean) as Tag[])
    : [];

  const activeStandard: string[] = [];
  if (data) {
    if (data.username) activeStandard.push('username');
    if (data.password) activeStandard.push('password');
    if (data.url) activeStandard.push('url');
    if (data.email) activeStandard.push('email');
    if (data.notes) activeStandard.push('notes');
    if (data.totpSecret && data.totpSecret !== 'has-totp') activeStandard.push('totpSecret');
  }

  const displayCustomFields = data ? data.customFields.filter(cf => cf.name !== '_field_order') : [];
  const layoutOrder = data ? getFieldLayout(data.customFields, activeStandard) : [];

  return (
    <div className="flex h-full flex-col overflow-y-auto">
      <AnimatePresence mode="wait">
        {isLoadingDetail ? (
          <motion.div
            key="loading-skeleton"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.1, ease: 'easeInOut' }}
            className="flex flex-col"
          >
            {/* Header */}
            <div className="flex items-start justify-between border-b border-[var(--border-subtle)] p-4">
              <div className="flex items-start gap-3 w-full">
                <Skeleton className="h-9 w-9 rounded-[3px] shrink-0" />
                <div className="flex flex-col gap-2 flex-1 min-w-0">
                  <Skeleton className="h-5 w-40 rounded" />
                  <Skeleton className="h-4 w-60 rounded" />
                  <div className="mt-1 flex gap-1">
                    <Skeleton className="h-5 w-14 rounded-[2px]" />
                    <Skeleton className="h-5 w-16 rounded-[2px]" />
                  </div>
                </div>
              </div>
            </div>

            {/* Fields */}
            <div className="flex flex-col gap-[2px] p-4">
              {[...Array(5)].map((_, i) => (
                <div key={i} className="flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5">
                  <Skeleton className="h-6 w-6 shrink-0" />
                  <div className="flex-1 min-w-0">
                    <Skeleton className="h-4 w-32 rounded" />
                  </div>
                </div>
              ))}
            </div>
          </motion.div>
        ) : !selectedEntry ? (
          <motion.div
            key="no-selection"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.1, ease: 'easeInOut' }}
            className="flex h-[80vh] flex-col items-center justify-center gap-4"
          >
            <div className="flex h-12 w-12 items-center justify-center rounded-[3px] border border-[var(--border)] text-[var(--text-tertiary)]">
              <Key size={24} />
            </div>
            <div className="text-center">
              <p className="text-[16px] font-semibold text-[var(--text-tertiary)]">Select an entry</p>
              <p className="mt-1 text-[13px] text-[var(--text-tertiary)]">
                Choose a password from the list to view details
              </p>
            </div>
          </motion.div>
        ) : !data ? null : (
          <motion.div
            key={data.id}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.1, ease: 'easeInOut' }}
            className="flex flex-col"
          >
            {/* Header */}
            <div className="flex items-start justify-between border-b border-[var(--border-subtle)] p-4">
              <div className="flex items-start gap-3">
                {/* Favicon */}
                <Favicon
                  url={data.url}
                  title={data.title}
                  color={entryTags[0]?.color}
                  sizeClass="h-9 w-9"
                  textClass="text-[12px]"
                />

                <div className="min-w-0">
                  <h1 className="text-[20px] font-semibold leading-tight tracking-tight text-[var(--text-primary)]">
                    {isEditing && editData ? (
                      <input
                        type="text"
                        value={editData.title}
                        onChange={(e) => setEditData({ ...editData, title: e.target.value })}
                        className="w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2 py-1 text-[20px] font-semibold outline-none focus:border-[var(--border-focus)]"
                      />
                    ) : (
                      data.title
                    )}
                  </h1>
                  {data.url && !isEditing && (
                    <a
                      href={`https://${data.url}`}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="mt-0.5 block truncate text-[12px] text-[var(--text-secondary)] transition-colors hover:text-[var(--text-primary)]"
                    >
                      {data.url}
                    </a>
                  )}
                  {isEditing && editData && (
                    <input
                      type="text"
                      value={editData.url}
                      onChange={(e) => setEditData({ ...editData, url: e.target.value })}
                      className="mt-1 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2 py-0.5 text-[12px] outline-none focus:border-[var(--border-focus)]"
                    />
                  )}
                  {/* Tags */}
                  <div className="mt-2 flex flex-wrap gap-1">
                    {entryTags.map((tag) => (
                      <span
                        key={tag.id}
                        className="inline-flex items-center gap-1 rounded-[2px] px-1.5 py-0.5 text-[11px]"
                        style={{
                          backgroundColor: `${tag.color}14`,
                          color: tag.color,
                          border: `1px solid ${tag.color}33`,
                        }}
                      >
                        <span className="h-1.5 w-1.5 rounded-full" style={{ backgroundColor: tag.color }} />
                        {tag.name}
                      </span>
                    ))}
                  </div>
                </div>
              </div>

              {/* Actions */}
              <div className="flex items-center gap-1">
                {isEditing ? (
                  <>
                    <button
                      onClick={handleSave}
                      className="h-8 rounded-[3px] bg-[var(--text-primary)] px-3 text-[13px] font-medium text-[var(--bg-base)] transition-colors hover:bg-[var(--accent-hover)]"
                    >
                      Save
                    </button>
                    <button
                      onClick={() => setIsEditing(false)}
                      className="h-8 rounded-[3px] px-3 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                    >
                      Cancel
                    </button>
                  </>
                ) : (
                  <>
                    {/* Pin toggle */}
                    <button
                      onClick={() => togglePin(data.id)}
                      className={`inline-flex h-8 w-8 items-center justify-center rounded-[3px] transition-colors ${data.pinned
                        ? 'text-yellow-500 hover:bg-yellow-500/10'
                        : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]'
                        }`}
                      title={data.pinned ? 'Unpin entry' : 'Pin entry'}
                    >
                      <Pin size={15} className={data.pinned ? 'fill-current' : ''} />
                    </button>

                    {/* Favorite toggle */}
                    <button
                      onClick={() => toggleFavorite(data.id)}
                      className={`inline-flex h-8 w-8 items-center justify-center rounded-[3px] transition-colors ${data.favorite
                        ? 'text-orange-500 hover:bg-orange-500/10'
                        : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]'
                        }`}
                      title={data.favorite ? 'Remove from favorites' : 'Add to favorites'}
                    >
                      <Star size={15} className={data.favorite ? 'fill-current' : ''} />
                    </button>

                    <div className="w-[1px] h-4 bg-[var(--border-subtle)] mx-1" />

                    <button
                      onClick={() => setShowEditModal(true)}
                      className="inline-flex h-8 items-center gap-1.5 rounded-[3px] px-2.5 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                    >
                      <Pencil size={14} />
                      Edit
                    </button>
                    <button
                      onClick={() => setShowDelConfirm(true)}
                      className="inline-flex h-8 items-center gap-1.5 rounded-[3px] px-2.5 text-[13px] font-medium text-[var(--destructive)] transition-colors hover:bg-[var(--destructive)]/8"
                    >
                      <Trash2 size={14} />
                      Delete
                    </button>
                  </>
                )}
              </div>
            </div>

            {/* Fields */}
            <div className="flex flex-col gap-[2px] p-4">
              {layoutOrder.map((id, i) => {
                const isStandard = ['username', 'password', 'url', 'email', 'notes', 'totpSecret'].includes(id);

                if (isStandard) {
                  if (id === 'username') {
                    return (
                      <motion.div
                        key="username"
                        initial={{ opacity: 0, y: 2 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.1, delay: i * 0.02 }}
                        className="flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5 transition-colors hover:bg-[var(--bg-hover)]"
                      >
                        <span className="shrink-0 text-[var(--text-secondary)]">
                          <User size={15} />
                        </span>
                        <div className="min-w-0 flex-1">
                          <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                            Username
                          </div>
                          {isEditing && editData ? (
                            <input
                              type="text"
                              value={editData.username}
                              onChange={(e) => setEditData({ ...editData, username: e.target.value })}
                              className="w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2 py-0.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                            />
                          ) : (
                            <div className="truncate text-[13px] text-[var(--text-primary)]">
                              {data.username}
                            </div>
                          )}
                        </div>
                        {!isEditing && <CopyButton value={data.username} />}
                      </motion.div>
                    );
                  }

                  if (id === 'password') {
                    return (
                      <div key="password-group" className="flex flex-col gap-2">
                        <motion.div
                          initial={{ opacity: 0, y: 2 }}
                          animate={{ opacity: 1, y: 0 }}
                          transition={{ duration: 0.1, delay: i * 0.02 }}
                          className="flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5 transition-colors hover:bg-[var(--bg-hover)]"
                        >
                          <span className="shrink-0 text-[var(--text-secondary)]">
                            <Key size={15} />
                          </span>
                          <div className="min-w-0 flex-1">
                            <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                              Password
                            </div>
                            {isEditing && editData ? (
                              <PasswordInput
                                value={editData.password}
                                onChange={(v) => setEditData({ ...editData, password: v })}
                              />
                            ) : (
                              <span className="font-mono text-[13px] tracking-wider text-[var(--text-primary)]">
                                {showPassword ? data.password : '••••••••'}
                              </span>
                            )}
                          </div>
                          {!isEditing && (
                            <div className="flex items-center gap-1 shrink-0">
                              <button
                                type="button"
                                onClick={() => setShowPassword(!showPassword)}
                                className="rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                                title={showPassword ? 'Hide password' : 'Show password'}
                              >
                                {showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                              </button>
                              <CopyButton value={data.password} />
                            </div>
                          )}
                        </motion.div>
                        {data.password && !isEditing && (
                          <PasswordSafetySection
                            password={data.password}
                            showTemporaryStats={showTemporaryStats}
                          />
                        )}
                      </div>
                    );
                  }

                  if (id === 'url') {
                    return (
                      <motion.div
                        key="url"
                        initial={{ opacity: 0, y: 2 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.1, delay: i * 0.02 }}
                        className="flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5 transition-colors hover:bg-[var(--bg-hover)]"
                      >
                        <span className="shrink-0 text-[var(--text-secondary)]">
                          <Link size={15} />
                        </span>
                        <div className="min-w-0 flex-1">
                          <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                            URL
                          </div>
                          {isEditing && editData ? (
                            <input
                              type="text"
                              value={editData.url}
                              onChange={(e) => setEditData({ ...editData, url: e.target.value })}
                              className="w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2 py-0.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                            />
                          ) : (
                            <div className="truncate text-[13px] text-[var(--text-primary)]">
                              {data.url}
                            </div>
                          )}
                        </div>
                        {!isEditing && <CopyButton value={data.url} />}
                      </motion.div>
                    );
                  }

                  if (id === 'email') {
                    return (
                      <motion.div
                        key="email"
                        initial={{ opacity: 0, y: 2 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.1, delay: i * 0.02 }}
                        className="flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5 transition-colors hover:bg-[var(--bg-hover)]"
                      >
                        <span className="shrink-0 text-[var(--text-secondary)]">
                          <Mail size={15} />
                        </span>
                        <div className="min-w-0 flex-1">
                          <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                            Email
                          </div>
                          {isEditing && editData ? (
                            <input
                              type="text"
                              value={editData.email}
                              onChange={(e) => setEditData({ ...editData, email: e.target.value })}
                              className="w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2 py-0.5 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                            />
                          ) : (
                            <div className="truncate text-[13px] text-[var(--text-primary)]">
                              {data.email}
                            </div>
                          )}
                        </div>
                        {!isEditing && <CopyButton value={data.email} />}
                      </motion.div>
                    );
                  }

                  if (id === 'notes') {
                    return (
                      <motion.div
                        key="notes"
                        initial={{ opacity: 0, y: 2 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.1, delay: i * 0.02 }}
                        className="flex items-start gap-3 rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5 transition-colors hover:bg-[var(--bg-hover)]"
                      >
                        <span className="mt-0.5 shrink-0 text-[var(--text-secondary)]">
                          <FileText size={15} />
                        </span>
                        <div className="min-w-0 flex-1">
                          <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                            Notes
                          </div>
                          {isEditing && editData ? (
                            <textarea
                              value={editData.notes}
                              onChange={(e) => setEditData({ ...editData, notes: e.target.value })}
                              rows={3}
                              className="mt-1 w-full resize-none rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2 py-1 text-[13px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                            />
                          ) : (
                            <p className="whitespace-pre-wrap text-[13px] leading-relaxed text-[var(--text-primary)]">
                              {data.notes}
                            </p>
                          )}
                        </div>
                      </motion.div>
                    );
                  }

                  if (id === 'totpSecret') {
                    return (
                      <Fragment key="totpSecret">
                        <TOTPField
                          secret={data.totpSecret || ''}
                          index={i}
                        />

                        {data.recoveryCodes && (
                          <div className="mt-3 border-t border-[var(--border-subtle)] pt-3">
                            <div className="flex items-center justify-between mb-2">
                              <div className="flex items-center gap-1.5">
                                <span className="text-[11px] font-semibold uppercase tracking-wide text-[var(--text-tertiary)]">
                                  Recovery Codes
                                </span>
                                <span className="text-[10px] text-[var(--text-tertiary)]/70">
                                  ({data.recoveryCodes.split(/[\s,;\n]+/).filter(Boolean).length} keys)
                                </span>
                              </div>
                              <button
                                onClick={() => setShowRecovery(!showRecovery)}
                                className="text-[11px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors"
                              >
                                {showRecovery ? 'Hide Codes' : 'Show Codes'}
                              </button>
                            </div>

                            {showRecovery && (
                              <motion.div
                                initial={{ opacity: 0, y: -4 }}
                                animate={{ opacity: 1, y: 0 }}
                                transition={{ duration: 0.1 }}
                                className="space-y-2"
                              >
                                <div className="grid grid-cols-2 gap-1.5 font-mono text-[12px] max-h-36 overflow-y-auto pr-1">
                                  {data.recoveryCodes.split(/[\s,;\n]+/).filter(Boolean).map((code, idx) => (
                                    <RecoveryCodeItem
                                      key={idx}
                                      code={code}
                                      index={idx}
                                      onCopy={() => addToast({ message: `Recovery code ${idx + 1} copied`, type: 'success' })}
                                    />
                                  ))}
                                </div>
                                <div className="flex justify-end">
                                  <button
                                    onClick={() => {
                                      navigator.clipboard.writeText(data.recoveryCodes || '');
                                      addToast({ message: 'All recovery codes copied', type: 'success' });
                                    }}
                                    className="rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 py-1 text-[11px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors"
                                  >
                                    Copy All
                                  </button>
                                </div>
                              </motion.div>
                            )}
                          </div>
                        )}
                      </Fragment>
                    );
                  }
                } else {
                  const cf = displayCustomFields.find(c => c.id === id);
                  if (!cf) return null;
                  return (
                    <motion.div
                      key={cf.id}
                      initial={{ opacity: 0, y: 2 }}
                      animate={{ opacity: 1, y: 0 }}
                      transition={{ duration: 0.1, delay: i * 0.02 }}
                      className="flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5 transition-colors hover:bg-[var(--bg-hover)]"
                    >
                      <span className="shrink-0 text-[var(--text-secondary)]">
                        <FileText size={15} />
                      </span>
                      <div className="min-w-0 flex-1">
                        <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)]">
                          {cf.name}
                        </div>
                        <div className="truncate text-[13px] text-[var(--text-primary)]">{cf.value}</div>
                      </div>
                      {!isEditing && <CopyButton value={cf.value} />}
                    </motion.div>
                  );
                }
              })}
            </div>

            {/* Footer */}
            <div className="mt-auto flex gap-6 border-t border-[var(--border-subtle)] px-4 py-3">
              <span className="text-[12px] text-[var(--text-tertiary)]">
                Created: {formatDate(data.createdAt)}
              </span>
              <span className="text-[12px] text-[var(--text-tertiary)]">
                Modified: {formatDate(data.updatedAt)}
              </span>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Delete confirmation */}
      <AnimatePresence>
        {showDelConfirm && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
            onClick={() => setShowDelConfirm(false)}
          >
            <motion.div
              initial={{ scale: 0.98, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0.98, opacity: 0 }}
              transition={{ duration: 0.1 }}
              className="w-[360px] rounded-[4px] border border-[var(--border)] bg-[var(--bg-elevated)] p-5"
              onClick={(e) => e.stopPropagation()}
            >
              <h3 className="text-[16px] font-semibold text-[var(--text-primary)]">
                Delete Entry
              </h3>
              <p className="mt-2 text-[13px] text-[var(--text-secondary)]">
                Are you sure you want to delete &ldquo;{selectedEntry?.title}&rdquo;? This action cannot be undone.
              </p>
              <div className="mt-4 flex justify-end gap-2">
                <button
                  onClick={() => setShowDelConfirm(false)}
                  className="h-8 rounded-[3px] px-3 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                >
                  Cancel
                </button>
                <button
                  onClick={() => {
                    if (selectedEntry) {
                      deleteEntry(selectedEntry.id);
                    }
                    setShowDelConfirm(false);
                  }}
                  className="h-8 rounded-[3px] bg-[var(--destructive)] px-3 text-[13px] font-medium text-white transition-colors hover:opacity-90"
                >
                  Delete
                </button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Edit modal */}
      <EntryModal
        open={showEditModal}
        onClose={() => setShowEditModal(false)}
        editEntry={selectedEntry}
      />
    </div>
  );
}

function RecoveryCodeItem({ code, index, onCopy }: { code: string; index: number; onCopy: () => void }) {
  const [hovered, setHovered] = useState(false);
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(code);
    setCopied(true);
    onCopy();
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <button
      onClick={handleCopy}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      className="flex items-center justify-between rounded-[3px] border border-[var(--border-subtle)] bg-[var(--bg-elevated)] px-2.5 py-1.5 text-left transition-all hover:border-[var(--border-focus)] hover:bg-[var(--bg-hover)] active:scale-[0.98] group cursor-pointer w-full outline-none"
      title="Click to copy"
    >
      <span className="truncate text-[var(--text-primary)] font-medium font-mono tracking-wide">
        {hovered ? code : '••••••••'}
      </span>
      <span className="text-[9px] text-[var(--text-tertiary)] shrink-0 select-none group-hover:text-[var(--text-secondary)] transition-colors ml-2">
        {copied ? 'Copied' : `#${index + 1}`}
      </span>
    </button>
  );
}

function TOTPField({ secret, index }: { secret: string; index: number }) {
  const code = useTotp(secret);
  const isUrgent = code ? code.seconds_remaining <= 5 : false;

  // Format code with space in middle: "123 456"
  const formattedCode = code
    ? (code.code.length === 6 ? `${code.code.slice(0, 3)} ${code.code.slice(3)}` : code.code)
    : 'Generating...';

  const progress = code ? code.seconds_remaining / code.period : 1;

  return (
    <motion.div
      initial={{ opacity: 0, y: 2 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.1, delay: index * 0.02 }}
      className="flex items-center gap-3 rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5 transition-colors hover:bg-[var(--bg-hover)] mt-4"
    >
      <span className="shrink-0 text-[var(--text-secondary)]">
        <ShieldCheck size={15} />
      </span>
      <div className="min-w-0 flex-1">
        <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-tertiary)] flex items-center gap-1.5">
          <span>2FA Code</span>
          {code && (
            <CountdownRing progress={progress} size={10} urgent={isUrgent} />
          )}
        </div>
        <div className="flex items-baseline gap-1.5">
          <span className={`font-mono text-[13px] font-semibold tracking-wider ${
            isUrgent ? 'text-red-400 animate-pulse' : 'text-[var(--text-primary)]'
          }`}>
            {formattedCode}
          </span>
          {code && (
            <span className="text-[10px] text-[var(--text-tertiary)] select-none">
              ({code.seconds_remaining}s)
            </span>
          )}
        </div>
      </div>
      {code && <CopyButton value={code.code} />}
    </motion.div>
  );
}

const CountdownRing: React.FC<{
  progress: number;
  size: number;
  urgent: boolean;
}> = ({ progress, size, urgent }) => {
  const r = size / 2 - 1.5;
  const circumference = 2 * Math.PI * r;
  const strokeDashoffset = circumference * (1 - progress);
  const color = urgent ? '#ef4444' : 'var(--accent, #3b82f6)';

  return (
    <svg width={size} height={size} className="shrink-0 -rotate-90">
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        stroke="var(--bg-hover)"
        strokeWidth={1.5}
      />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        stroke={color}
        strokeWidth={1.5}
        strokeDasharray={circumference}
        strokeDashoffset={strokeDashoffset}
        strokeLinecap="round"
        className="transition-all duration-1000 linear"
      />
    </svg>
  );
};

const PasswordSafetySection: React.FC<{
  password?: string;
  showTemporaryStats: boolean;
}> = ({ password, showTemporaryStats }) => {
  const [breachStatus, setBreachStatus] = useState<BreachStatus | null>(null);

  if (!password) return null;

  const isSafe = breachStatus?.type === 'Safe' || breachStatus?.type === 'Unknown';
  const shouldShowContainer = showTemporaryStats || !isSafe;

  if (!shouldShowContainer) return null;

  return (
    <motion.div
      initial={{ opacity: 0, y: 2 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -2 }}
      className="flex flex-col gap-2 rounded-[3px] bg-[var(--bg-elevated)] px-3 py-2.5 mt-2"
    >
      {showTemporaryStats && <PasswordStrength password={password} />}
      <BreachIndicator
        password={password}
        onStatusChange={(status) => setBreachStatus(status)}
        hideIfSafe={!showTemporaryStats}
      />
    </motion.div>
  );
};



