/**
 * EntryModal — Create / Edit password entry
 * 
 * Full form with PasswordGenerator integration,
 * live PasswordStrength, tag selector, custom fields.
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { motion, AnimatePresence, Reorder } from 'framer-motion';
import {
  X, Plus, Eye, EyeOff, Wand2, GripVertical,
  Globe, User, Mail, Key, FileText, ShieldCheck, Loader2,
} from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';
import { PasswordStrength } from './PasswordStrength';
import { PasswordGenerator } from './PasswordGenerator';
import { BreachIndicator } from './BreachIndicator';
import type { PasswordEntry, CustomField, Tag } from '@/types';
import { getFieldLayout } from '@/lib/utils';
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
} from '@/components/ui/dropdown-menu';

interface EntryModalProps {
  open: boolean;
  onClose: () => void;
  /** If provided, opens in edit mode with this entry's data */
  editEntry?: PasswordEntry | null;
}

const EMPTY_ENTRY: Omit<PasswordEntry, 'id' | 'createdAt' | 'updatedAt'> = {
  title: '',
  username: '',
  password: '',
  url: '',
  email: '',
  notes: '',
  tags: [],
  favorite: false,
  pinned: false,
  totpSecret: undefined,
  customFields: [],
};

type StandardFieldKey = 'username' | 'password' | 'email' | 'url' | 'notes' | 'totpSecret';

const PRESETS = [
  { id: 'login-user', name: 'Login (Username)', fields: ['username', 'password', 'url'] as StandardFieldKey[] },
  { id: 'login-email', name: 'Login (Email)', fields: ['email', 'password', 'url'] as StandardFieldKey[] },
  { id: 'note', name: 'Secure Note', fields: ['notes'] as StandardFieldKey[] },
  { id: 'password-only', name: 'Password Only', fields: ['password'] as StandardFieldKey[] },
  { id: 'custom', name: 'Custom', fields: [] as StandardFieldKey[] },
];

const FIELD_METADATA: { key: StandardFieldKey; label: string; icon: React.ReactNode }[] = [
  { key: 'username', label: 'Username', icon: <User size={13} /> },
  { key: 'email', label: 'Email', icon: <Mail size={13} /> },
  { key: 'password', label: 'Password', icon: <Key size={13} /> },
  { key: 'url', label: 'Website (URL)', icon: <Globe size={13} /> },
  { key: 'totpSecret', label: 'Two-Factor Auth (TOTP)', icon: <ShieldCheck size={13} /> },
  { key: 'notes', label: 'Secure Note', icon: <FileText size={13} /> },
];

export default function EntryModal({ open, onClose, editEntry }: EntryModalProps) {
  const { addEntry, updateEntry, tags: allTags, addToast } = useAppState();
  const isEdit = !!editEntry;

  const [form, setForm] = useState(EMPTY_ENTRY);
  const [showPassword, setShowPassword] = useState(false);
  const [showGenerator, setShowGenerator] = useState(false);
  const [loading, setLoading] = useState(false);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [fieldsOrder, setFieldsOrder] = useState<string[]>(['username', 'password', 'url']);

  const titleRef = useRef<HTMLInputElement>(null);

  const activeFields = fieldsOrder.filter((f): f is StandardFieldKey =>
    ['username', 'password', 'email', 'url', 'notes', 'totpSecret'].includes(f)
  );

  // Populate form on open
  useEffect(() => {
    if (open) {
      if (editEntry) {
        const standardActive: StandardFieldKey[] = [];
        if (editEntry.username) standardActive.push('username');
        if (editEntry.password) standardActive.push('password');
        if (editEntry.url) standardActive.push('url');
        if (editEntry.email) standardActive.push('email');
        if (editEntry.notes) standardActive.push('notes');
        if (editEntry.totpSecret) standardActive.push('totpSecret');
        if (standardActive.length === 0) {
          standardActive.push('username', 'password', 'url');
        }

        const layout = getFieldLayout(editEntry.customFields, standardActive);
        setFieldsOrder(layout);

        setForm({
          title: editEntry.title,
          username: editEntry.username,
          password: editEntry.password,
          url: editEntry.url,
          email: editEntry.email,
          notes: editEntry.notes,
          tags: [...editEntry.tags],
          favorite: editEntry.favorite,
          pinned: editEntry.pinned,
          totpSecret: editEntry.totpSecret,
          customFields: editEntry.customFields.map(f => ({ ...f })),
        });
      } else {
        setFieldsOrder(['username', 'password', 'url']);
        setForm({ ...EMPTY_ENTRY, customFields: [] });
      }
      setErrors({});
      setShowGenerator(false);
      setTimeout(() => titleRef.current?.focus(), 100);
    }
  }, [open, editEntry]);

  // Clear sensitive data on close
  useEffect(() => {
    if (!open) {
      setForm(prev => ({ ...prev, password: '' }));
      setShowPassword(false);
    }
  }, [open]);

  // Esc to close
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open && !showGenerator) onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onClose, showGenerator]);

  const updateField = useCallback(<K extends keyof typeof form>(key: K, value: typeof form[K]) => {
    setForm(prev => ({ ...prev, [key]: value }));
    setErrors(prev => ({ ...prev, [key]: '' }));
  }, []);

  const removeField = useCallback((fieldKey: StandardFieldKey) => {
    setFieldsOrder(prev => prev.filter(f => f !== fieldKey));
  }, []);

  const toggleTag = useCallback((tagName: string) => {
    setForm(prev => ({
      ...prev,
      tags: prev.tags.includes(tagName)
        ? prev.tags.filter(t => t !== tagName)
        : [...prev.tags, tagName],
    }));
  }, []);

  const addCustomField = useCallback(() => {
    const id = crypto.randomUUID();
    setForm(prev => ({
      ...prev,
      customFields: [
        ...prev.customFields,
        { id, name: '', type: 'text' as const, value: '' },
      ],
    }));
    setFieldsOrder(prev => [...prev, id]);
  }, []);

  const updateCustomField = useCallback((id: string, updates: Partial<CustomField>) => {
    setForm(prev => ({
      ...prev,
      customFields: prev.customFields.map(f =>
        f.id === id ? { ...f, ...updates } : f
      ),
    }));
  }, []);

  const removeCustomField = useCallback((id: string) => {
    setForm(prev => ({
      ...prev,
      customFields: prev.customFields.filter(f => f.id !== id),
    }));
    setFieldsOrder(prev => prev.filter(fId => fId !== id));
  }, []);

  const validate = (): boolean => {
    const errs: Record<string, string> = {};
    if (!form.title.trim()) errs.title = 'Title is required';
    if (activeFields.includes('password') && !form.password.trim() && !isEdit) {
      errs.password = 'Password is required';
    }
    setErrors(errs);
    return Object.keys(errs).length === 0;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!validate()) return;

    setLoading(true);
    try {
      const now = new Date().toISOString();
      const cleanedForm = { ...form };
      if (!activeFields.includes('username')) cleanedForm.username = '';
      if (!activeFields.includes('password')) cleanedForm.password = '';
      if (!activeFields.includes('url')) cleanedForm.url = '';
      if (!activeFields.includes('email')) cleanedForm.email = '';
      if (!activeFields.includes('notes')) cleanedForm.notes = '';
      if (!activeFields.includes('totpSecret')) {
        cleanedForm.totpSecret = undefined;
        cleanedForm.recoveryCodes = undefined;
      }

      // Save fields order as metadata
      let customFields = form.customFields.filter(cf => cf.name !== '_field_order');
      customFields.push({
        id: crypto.randomUUID(),
        name: '_field_order',
        type: 'text',
        value: fieldsOrder.join(','),
      });
      cleanedForm.customFields = customFields;

      if (isEdit && editEntry) {
        const updated: PasswordEntry = {
          ...editEntry,
          ...cleanedForm,
          updatedAt: now,
        };
        await updateEntry(updated);
        addToast({ message: 'Entry updated', type: 'success' });
      } else {
        const newEntry: PasswordEntry = {
          ...cleanedForm,
          id: crypto.randomUUID(),
          createdAt: now,
          updatedAt: now,
        };
        await addEntry(newEntry);
        addToast({ message: 'Entry created', type: 'success' });
      }
      onClose();
    } catch (err: any) {
      addToast({ message: `Failed: ${err}`, type: 'error' });
    } finally {
      setLoading(false);
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
            className="flex max-h-[85vh] w-[520px] flex-col rounded-lg border border-[var(--border)] bg-[var(--bg-base)] shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between border-b border-[var(--border-subtle)] px-5 py-3.5">
              <h2 className="text-[16px] font-semibold text-[var(--text-primary)]">
                {isEdit ? 'Edit Entry' : 'New Entry'}
              </h2>
              <button
                onClick={onClose}
                className="rounded-md p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
              >
                <X size={16} />
              </button>
            </div>

            {/* Form */}
            <form onSubmit={handleSubmit} className="flex flex-col gap-3 overflow-y-auto p-5 flex-1 min-h-0">
              {/* Presets / Templates */}
              <div className="flex flex-col gap-1.5 pb-2 border-b border-[var(--border-subtle)] mb-1">
                <label className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-secondary)]">
                  Template
                </label>
                <div className="flex flex-wrap gap-1.5">
                  {PRESETS.map((p) => {
                    const isMatch = p.id === 'custom'
                      ? !PRESETS.filter(x => x.id !== 'custom').some(x => {
                          const standardFields = x.fields;
                          return (
                            fieldsOrder.length === standardFields.length &&
                            standardFields.every(f => fieldsOrder.includes(f))
                          );
                        })
                      : (
                          fieldsOrder.length === p.fields.length &&
                          p.fields.every(f => fieldsOrder.includes(f))
                        );

                    return (
                      <button
                        key={p.id}
                        type="button"
                        onClick={() => {
                          if (p.id === 'custom') {
                            setFieldsOrder([]);
                          } else {
                            setFieldsOrder(p.fields);
                          }
                        }}
                        className={`rounded px-2.5 py-1 text-[11px] font-medium transition-all ${
                          isMatch
                            ? 'bg-[var(--text-primary)] text-[var(--bg-base)] shadow-sm'
                            : 'bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] border border-[var(--border)]'
                        }`}
                      >
                        {p.name}
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* Title (Locked at the top) */}
              <div className="flex flex-col gap-1.5">
                <label className="flex items-center gap-1.5 text-[12px] font-medium text-[var(--text-secondary)]">
                  <Globe size={14} /> Title
                  <span className="text-[var(--destructive)]">*</span>
                </label>
                <input
                  ref={titleRef}
                  type="text"
                  value={form.title}
                  onChange={(e) => updateField('title', e.target.value)}
                  placeholder="GitHub"
                  className={`h-9 w-full rounded-md border bg-[var(--bg-elevated)] px-3 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] ${
                    errors.title ? 'border-[var(--destructive)]' : 'border-[var(--border)]'
                  }`}
                  required
                />
                {errors.title && <span className="text-[11px] text-[var(--destructive)]">{errors.title}</span>}
              </div>

              {/* Reorderable Fields list */}
              <Reorder.Group axis="y" values={fieldsOrder} onReorder={setFieldsOrder} className="flex flex-col gap-3">
                {fieldsOrder.map((id) => {
                  const isStandard = ['username', 'password', 'email', 'url', 'notes', 'totpSecret'].includes(id);
                  let icon = <FileText size={13} />;
                  let label = 'Custom Field';
                  let content = null;
                  let onRemove = () => {};

                  if (isStandard) {
                    onRemove = () => removeField(id as StandardFieldKey);
                    if (id === 'username') {
                      icon = <User size={13} />;
                      label = 'Username';
                      content = (
                        <input
                          type="text"
                          value={form.username}
                          onChange={(e) => updateField('username', e.target.value)}
                          placeholder="john_doe"
                          className="h-9 w-full rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                        />
                      );
                    } else if (id === 'email') {
                      icon = <Mail size={13} />;
                      label = 'Email';
                      content = (
                        <input
                          type="text"
                          value={form.email}
                          onChange={(e) => updateField('email', e.target.value)}
                          placeholder="john@example.com"
                          className="h-9 w-full rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                        />
                      );
                    } else if (id === 'url') {
                      icon = <Globe size={13} />;
                      label = 'Website (URL)';
                      content = (
                        <input
                          type="text"
                          value={form.url}
                          onChange={(e) => updateField('url', e.target.value)}
                          placeholder="github.com"
                          className="h-9 w-full rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                        />
                      );
                    } else if (id === 'totpSecret') {
                      icon = <ShieldCheck size={13} />;
                      label = 'Two-Factor Auth (TOTP)';
                      content = (
                        <div className="flex flex-col gap-3">
                          <input
                            type="text"
                            value={form.totpSecret || ''}
                            onChange={(e) => updateField('totpSecret', e.target.value || undefined)}
                            placeholder="TOTP secret or otpauth:// URI"
                            className="h-9 w-full rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 font-mono text-[13px] text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                          />
                          <div className="flex flex-col gap-1">
                            <span className="text-[11px] font-medium text-[var(--text-secondary)]">
                              Recovery Codes (Secure Keys / Backup Codes)
                            </span>
                            <textarea
                              value={form.recoveryCodes || ''}
                              onChange={(e) => updateField('recoveryCodes', e.target.value || undefined)}
                              placeholder="Enter recovery codes (one per line, space or comma separated)..."
                              rows={3}
                              className="w-full resize-none rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 py-2 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                            />
                          </div>
                        </div>
                      );
                    } else if (id === 'notes') {
                      icon = <FileText size={13} />;
                      label = 'Secure Note';
                      content = (
                        <textarea
                          value={form.notes}
                          onChange={(e) => updateField('notes', e.target.value)}
                          placeholder="Optional notes..."
                          rows={2}
                          className="w-full resize-none rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 py-2 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                        />
                      );
                    } else if (id === 'password') {
                      icon = <Key size={13} />;
                      label = 'Password';
                      content = (
                        <div className="flex flex-col gap-1.5">
                          <div className="flex gap-1.5">
                            <div className="relative flex-1">
                              <input
                                type={showPassword ? 'text' : 'password'}
                                value={form.password}
                                onChange={(e) => updateField('password', e.target.value)}
                                placeholder="Enter or generate a password"
                                className={`h-9 w-full rounded-md border bg-[var(--bg-elevated)] px-3 pr-9 font-mono text-[13px] tracking-wide text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:tracking-normal placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] ${
                                  errors.password ? 'border-[var(--destructive)]' : 'border-[var(--border)]'
                                }`}
                              />
                              <button
                                type="button"
                                onClick={() => setShowPassword(!showPassword)}
                                className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
                              >
                                {showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                              </button>
                            </div>
                            <button
                              type="button"
                              onClick={() => setShowGenerator(!showGenerator)}
                              className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-md border transition-colors ${
                                showGenerator
                                  ? 'border-[var(--text-primary)] bg-[var(--bg-active)] text-[var(--text-primary)]'
                                  : 'border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]'
                              }`}
                              title="Generate password"
                            >
                              <Wand2 size={15} />
                            </button>
                          </div>
                          {errors.password && (
                            <span className="text-[11px] text-[var(--destructive)]">{errors.password}</span>
                          )}
                          {form.password && !showGenerator && (
                            <div className="flex flex-col gap-1.5 mt-1.5 px-0.5">
                              <PasswordStrength password={form.password} compact />
                              <BreachIndicator password={form.password} compact />
                            </div>
                          )}
                          <AnimatePresence>
                            {showGenerator && (
                              <motion.div
                                initial={{ height: 0, opacity: 0 }}
                                animate={{ height: 'auto', opacity: 1 }}
                                exit={{ height: 0, opacity: 0 }}
                                transition={{ duration: 0.2 }}
                                className="overflow-hidden rounded-md border border-[var(--border)] mt-1.5"
                              >
                                <PasswordGenerator
                                  url={form.url}
                                  onSelect={(pw) => {
                                    updateField('password', pw);
                                    setShowGenerator(false);
                                  }}
                                  onClose={() => setShowGenerator(false)}
                                />
                              </motion.div>
                            )}
                          </AnimatePresence>
                        </div>
                      );
                    }
                  } else {
                    const cf = form.customFields.find(f => f.id === id);
                    if (!cf) return null;
                    onRemove = () => removeCustomField(id);
                    content = (
                      <div className="flex gap-1.5">
                        <input
                          type="text"
                          value={cf.name}
                          onChange={(e) => updateCustomField(cf.id, { name: e.target.value })}
                          placeholder="Field name"
                          className="h-9 w-[140px] rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)] font-medium"
                        />
                        <input
                          type="text"
                          value={cf.value}
                          onChange={(e) => updateCustomField(cf.id, { value: e.target.value })}
                          placeholder="Value"
                          className="h-9 flex-1 rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                        />
                      </div>
                    );
                  }

                  return (
                    <Reorder.Item
                      key={id}
                      value={id}
                      className="rounded-md border border-[var(--border-subtle)] bg-[var(--bg-elevated)] p-3.5 shadow-sm hover:border-[var(--border)] transition-colors cursor-default select-none"
                    >
                      <div className="flex items-center justify-between mb-2">
                        <div className="flex items-center gap-2 text-[12px] font-medium text-[var(--text-secondary)]">
                          <span className="cursor-grab text-[var(--text-tertiary)] active:cursor-grabbing hover:text-[var(--text-primary)] transition-colors p-0.5">
                            <GripVertical size={13} />
                          </span>
                          <span className="flex items-center gap-1.5">
                            {icon} {label}
                          </span>
                        </div>
                        <button
                          type="button"
                          onClick={onRemove}
                          className="rounded p-0.5 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-red-400 transition-colors"
                          title={`Remove ${label}`}
                        >
                          <X size={12} />
                        </button>
                      </div>

                      {content}
                    </Reorder.Item>
                  );
                })}
              </Reorder.Group>

              {/* Tags */}
              <div className="flex flex-col gap-1.5 mt-1">
                <label className="text-[12px] font-medium text-[var(--text-secondary)]">Tags</label>
                <div className="flex flex-wrap gap-1.5">
                  {allTags.map((tag: Tag) => {
                    const active = form.tags.includes(tag.name);
                    return (
                      <button
                        key={tag.id}
                        type="button"
                        onClick={() => toggleTag(tag.name)}
                        className={`flex items-center gap-1.5 rounded-md px-2.5 py-1 text-[12px] font-medium transition-all ${
                          active
                            ? 'text-white'
                            : 'bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]'
                        }`}
                        style={active ? { backgroundColor: tag.color } : {}}
                      >
                        <span
                          className="h-2 w-2 rounded-full"
                          style={{ backgroundColor: active ? '#fff' : tag.color }}
                        />
                        {tag.name}
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* Add Field Button & Dropdown */}
              <div className="relative mt-1 self-start">
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <button
                      type="button"
                      className="flex items-center gap-1.5 rounded-md px-2 py-1 text-[12px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-elevated)] hover:text-[var(--text-primary)]"
                    >
                      <Plus size={13} /> Add field
                    </button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="start" className="w-[200px] max-h-[300px] overflow-y-auto z-50">
                    {FIELD_METADATA.filter(f => !fieldsOrder.includes(f.key)).map((field) => (
                      <DropdownMenuItem
                        key={field.key}
                        onSelect={() => {
                          setFieldsOrder(prev => [...prev, field.key]);
                        }}
                        className="flex items-center gap-2 text-[12px] cursor-pointer"
                      >
                        {field.icon}
                        <span>{field.label}</span>
                      </DropdownMenuItem>
                    ))}

                    {FIELD_METADATA.filter(f => !fieldsOrder.includes(f.key)).length > 0 && (
                      <DropdownMenuSeparator />
                    )}

                    <DropdownMenuItem
                      key="custom-field"
                      onSelect={() => {
                        addCustomField();
                      }}
                      className="flex items-center gap-2 text-[12px] cursor-pointer"
                    >
                      <Plus size={13} />
                      <span>Custom Field</span>
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>

              {/* Actions */}
              <div className="flex justify-between border-t border-[var(--border-subtle)] pt-4 mt-2">
                <label className="flex items-center gap-2 text-[12px] text-[var(--text-secondary)]">
                  <input
                    type="checkbox"
                    checked={form.favorite}
                    onChange={(e) => updateField('favorite', e.target.checked)}
                    className="accent-[var(--accent)] h-3.5 w-3.5"
                  />
                  Favorite
                </label>

                <div className="flex gap-2">
                  <button
                    type="button"
                    onClick={onClose}
                    className="h-9 rounded-md px-4 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)]"
                  >
                    Cancel
                  </button>
                  <button
                    type="submit"
                    disabled={loading}
                    className="flex h-9 items-center gap-2 rounded-md bg-[var(--text-primary)] px-5 text-[13px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 disabled:opacity-50"
                  >
                    {loading ? (
                      <>
                        <Loader2 size={14} className="animate-spin" />
                        Saving...
                      </>
                    ) : isEdit ? (
                      'Save Changes'
                    ) : (
                      'Create Entry'
                    )}
                  </button>
                </div>
              </div>
            </form>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

// ─── Field Component (Only used for Title input compatibility if needed) ───

import React from 'react';

interface FieldProps {
  icon?: React.ReactNode;
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  error?: string;
  required?: boolean;
  type?: string;
  onRemove?: () => void;
}

const Field = React.forwardRef<HTMLInputElement, FieldProps>(
  ({ icon, label, value, onChange, placeholder, error, required, type = 'text', onRemove }, ref) => (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between text-[12px] font-medium text-[var(--text-secondary)]">
        <span className="flex items-center gap-1.5">
          {icon} {label}
          {required && <span className="text-[var(--destructive)]">*</span>}
        </span>
        {onRemove && (
          <button
            type="button"
            onClick={onRemove}
            className="rounded p-0.5 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-red-400 transition-colors"
            title={`Remove ${label}`}
          >
            <X size={12} />
          </button>
        )}
      </div>
      <input
        ref={ref}
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className={`h-9 rounded-md border bg-[var(--bg-elevated)] px-3 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] ${
          error ? 'border-[var(--destructive)]' : 'border-[var(--border)]'
        }`}
      />
      {error && <span className="text-[11px] text-[var(--destructive)]">{error}</span>}
    </div>
  )
);
Field.displayName = 'Field';

