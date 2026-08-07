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
  Globe, User, Mail, Key, FileText, ShieldCheck, Loader2, Fingerprint,
} from 'lucide-react';
import { useAppState } from '@/contexts/AppStateContext';
import { useTranslation } from '@/contexts/LanguageContext';
import { PasswordStrength } from './PasswordStrength';
import { PasswordGenerator } from './PasswordGenerator';
import { BreachIndicator } from './BreachIndicator';
import CreateTagModal from './CreateTagModal';
import type { PasswordEntry, CustomField, Tag, FieldType } from '@/types';
import { getFieldLayout } from '@/lib/utils';
import { ActionTooltip } from './ui/tooltip';
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
  hasPasskey: false,
  passkeyPublicKey: undefined,
  generatePasskey: undefined,
  passkeyAction: undefined,
};

type StandardFieldKey = 'username' | 'password' | 'email' | 'url' | 'notes' | 'totpSecret' | 'passkey';

const PRESETS = [
  { id: 'login-user', nameKey: 'preset.login_user', fields: ['username', 'password', 'url'] as StandardFieldKey[] },
  { id: 'login-email', nameKey: 'preset.login_email', fields: ['email', 'password', 'url'] as StandardFieldKey[] },
  { id: 'note', nameKey: 'preset.secure_note', fields: ['notes'] as StandardFieldKey[] },
  { id: 'password-only', nameKey: 'preset.password_only', fields: ['password'] as StandardFieldKey[] },
  { id: 'custom', nameKey: 'preset.custom', fields: [] as StandardFieldKey[] },
];

export default function EntryModal({ open, onClose, editEntry }: EntryModalProps) {
  const { t } = useTranslation();
  const { addEntry, updateEntry, tags: allTags, filterCategory, addToast } = useAppState();
  const isEdit = !!editEntry;

  const [form, setForm] = useState(EMPTY_ENTRY);
  const [showPassword, setShowPassword] = useState(false);
  const [showCustomPasswords, setShowCustomPasswords] = useState<Record<string, boolean>>({});
  const [showGenerator, setShowGenerator] = useState(false);
  const [loading, setLoading] = useState(false);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [fieldsOrder, setFieldsOrder] = useState<string[]>(['username', 'password', 'url']);
  const [showCreateTagModal, setShowCreateTagModal] = useState(false);

  const prevTagsRef = useRef<Tag[]>(allTags);
  useEffect(() => {
    if (allTags.length > prevTagsRef.current.length) {
      const newTag = allTags.find(t => !prevTagsRef.current.some(pt => pt.id === t.id));
      if (newTag && !form.tags.includes(newTag.name)) {
        setForm(prev => ({ ...prev, tags: [...prev.tags, newTag.name] }));
      }
    }
    prevTagsRef.current = allTags;
  }, [allTags, form.tags]);

  const titleRef = useRef<HTMLInputElement>(null);

  const activeFields = fieldsOrder.filter((f): f is StandardFieldKey =>
    ['username', 'password', 'email', 'url', 'notes', 'totpSecret', 'passkey'].includes(f)
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
        if (editEntry.hasPasskey) standardActive.push('passkey');
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
          hasPasskey: editEntry.hasPasskey,
          passkeyPublicKey: editEntry.passkeyPublicKey,
          generatePasskey: undefined,
          passkeyAction: undefined,
        });
      } else {
        setFieldsOrder(['username', 'password', 'url']);
        const initialTags: string[] = [];
        let initialFavorite = false;

        if (filterCategory === 'favorites') {
          initialFavorite = true;
        } else if (filterCategory && filterCategory !== 'all') {
          initialTags.push(filterCategory);
        }

        setForm({
          ...EMPTY_ENTRY,
          tags: initialTags,
          favorite: initialFavorite,
          customFields: [],
        });
      }
      setErrors({});
      setShowGenerator(false);
      setTimeout(() => titleRef.current?.focus(), 100);
    }
  }, [open, editEntry, filterCategory]);

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

  const getFieldSuffix = useCallback((type: FieldType): string => {
    switch (type) {
      case 'password': return t('detail.password');
      case 'email': return t('detail.email');
      case 'username': return t('detail.username');
      case 'url': return t('detail.url');
      case 'totp': return t('detail.totp');
      case 'notes': return t('preset.secure_note');
      default: return t('entry.custom_field');
    }
  }, [t]);

  const getOrdinal = useCallback((count: number): string => {
    const ordinalKeys = [
      'ordinal.1', 'ordinal.2', 'ordinal.3', 'ordinal.4', 'ordinal.5',
      'ordinal.6', 'ordinal.7', 'ordinal.8', 'ordinal.9'
    ];
    if (count <= 0) return '';
    if (count - 1 < ordinalKeys.length) return t(ordinalKeys[count - 1]);
    return `${count + 1}`;
  }, [t]);

  const getFieldCount = useCallback((type: FieldType | StandardFieldKey): number => {
    let count = 0;
    if (type === 'username' && fieldsOrder.includes('username')) count++;
    if (type === 'password' && fieldsOrder.includes('password')) count++;
    if (type === 'email' && fieldsOrder.includes('email')) count++;
    if (type === 'url' && fieldsOrder.includes('url')) count++;
    if ((type === 'totpSecret' || type === 'totp') && fieldsOrder.includes('totpSecret')) count++;
    if (type === 'notes' && fieldsOrder.includes('notes')) count++;

    const targetType = type === 'totpSecret' ? 'totp' : type;
    count += form.customFields.filter(f => f.type === targetType).length;
    return count;
  }, [fieldsOrder, form.customFields]);

  const getPrefix = useCallback((name: string, suffix: string): string => {
    if (!suffix) return name;
    const regex = new RegExp(`\\s+${suffix}$`, 'i');
    return name.replace(regex, '');
  }, []);

  const formatFullName = useCallback((prefix: string, suffix: string, defaultOrdinal: string): string => {
    const p = prefix.trim();
    if (p) {
      const regex = new RegExp(`\\s+${suffix}$`, 'i');
      if (regex.test(p)) return p;
      return `${p} ${suffix}`;
    }
    return defaultOrdinal ? `${defaultOrdinal} ${suffix}` : suffix;
  }, []);

  const getDropdownLabel = useCallback((key: StandardFieldKey | FieldType, baseLabel: string): string => {
    const count = getFieldCount(key);
    if (count === 0) return baseLabel;
    const ordinal = getOrdinal(count);
    return `${ordinal} ${baseLabel}`;
  }, [getFieldCount, getOrdinal]);

  const applyTemplatePreset = useCallback((presetFields: StandardFieldKey[]) => {
    setFieldsOrder(presetFields);

    setForm(prev => {
      let updatedEmail = prev.email;

      // Transfer username -> email if user entered an email in username before switching to Email template
      if (presetFields.includes('email') && !updatedEmail.trim() && prev.username.includes('@')) {
        updatedEmail = prev.username;
      }

      return {
        ...prev,
        email: updatedEmail,
      };
    });
  }, []);

  const addCustomField = useCallback((type: FieldType = 'text', defaultName: string = '') => {
    const id = crypto.randomUUID();
    setForm(prev => ({
      ...prev,
      customFields: [
        ...prev.customFields,
        { id, name: defaultName, type, value: '' },
      ],
    }));
    setFieldsOrder(prev => [...prev, id]);
  }, []);

  const handleAddField = useCallback((key: StandardFieldKey | FieldType | 'custom') => {
    const isStandardKey = ['username', 'password', 'email', 'url', 'notes', 'totpSecret', 'passkey'].includes(key);

    if (isStandardKey && !fieldsOrder.includes(key)) {
      setFieldsOrder(prev => [...prev, key]);

      if (key === 'email') {
        setForm(prev => {
          if (!prev.email.trim() && prev.username.includes('@')) {
            return { ...prev, email: prev.username };
          }
          return prev;
        });
      } else if (key === 'username') {
        setForm(prev => {
          if (!prev.username.trim() && prev.email.trim()) {
            return { ...prev, username: prev.email };
          }
          return prev;
        });
      }
    } else {
      let type: FieldType = 'text';

      if (key === 'password') type = 'password';
      else if (key === 'email') type = 'email';
      else if (key === 'username') type = 'username';
      else if (key === 'url') type = 'url';
      else if (key === 'totpSecret' || key === 'totp') type = 'totp';
      else if (key === 'notes') type = 'notes';
      else if (key === 'text') type = 'text';

      const count = getFieldCount(type);
      const ordinal = getOrdinal(count);
      const suffix = getFieldSuffix(type);
      const defaultName = formatFullName(ordinal, suffix, ordinal);

      addCustomField(type, defaultName);
    }
  }, [fieldsOrder, addCustomField, getFieldCount, getOrdinal, getFieldSuffix, formatFullName]);

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

      // Passkey creation/destruction logic based on active fields list
      if (isEdit && editEntry) {
        if (activeFields.includes('passkey')) {
          if (!editEntry.hasPasskey) {
            cleanedForm.passkeyAction = 'generate';
          }
        } else {
          if (editEntry.hasPasskey) {
            cleanedForm.passkeyAction = 'remove';
          }
        }
      } else {
        if (activeFields.includes('passkey')) {
          cleanedForm.generatePasskey = true;
        }
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
                {isEdit ? t('common.edit') : t('list.new_entry')}
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
                  {t('entry.template')}
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
                            applyTemplatePreset(p.fields);
                          }
                        }}
                        className={`rounded px-2.5 py-1 text-[11px] font-medium transition-all ${
                          isMatch
                            ? 'bg-[var(--text-primary)] text-[var(--bg-base)] shadow-sm'
                            : 'bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] border border-[var(--border)]'
                        }`}
                      >
                        {t(p.nameKey)}
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* Title (Locked at the top) */}
              <div className="flex flex-col gap-1.5">
                <label className="flex items-center gap-1.5 text-[12px] font-medium text-[var(--text-secondary)]">
                  <Globe size={14} /> {t('entry.title')}
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
                      label = t('detail.username');
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
                      label = t('detail.email');
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
                      label = t('detail.url');
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
                      label = t('detail.totp');
                      content = (
                        <div className="flex flex-col gap-3">
                          <input
                            type="text"
                            value={form.totpSecret || ''}
                            onChange={(e) => updateField('totpSecret', e.target.value || undefined)}
                            placeholder={t('entry.totp_placeholder')}
                            className="h-9 w-full rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 font-mono text-[13px] text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                          />
                          <div className="flex flex-col gap-1">
                            <span className="text-[11px] font-medium text-[var(--text-secondary)]">
                              Recovery Codes (Secure Keys / Backup Codes)
                            </span>
                            <textarea
                              value={form.recoveryCodes || ''}
                              onChange={(e) => updateField('recoveryCodes', e.target.value || undefined)}
                              placeholder={t('entry.recovery_placeholder')}
                              rows={3}
                              className="w-full resize-none rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 py-2 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                            />
                          </div>
                        </div>
                      );
                    } else if (id === 'notes') {
                      icon = <FileText size={13} />;
                      label = t('preset.secure_note');
                      content = (
                        <textarea
                          value={form.notes}
                          onChange={(e) => updateField('notes', e.target.value)}
                          placeholder={t('entry.notes_placeholder')}
                          rows={2}
                          className="w-full resize-none rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 py-2 text-[13px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
                        />
                      );
                    } else if (id === 'password') {
                      icon = <Key size={13} />;
                      label = t('detail.password');
                      content = (
                        <div className="flex flex-col gap-1.5">
                          <div className="flex gap-1.5">
                            <div className="relative flex-1">
                              <input
                                type={showPassword ? 'text' : 'password'}
                                value={form.password}
                                onChange={(e) => updateField('password', e.target.value)}
                                placeholder={t('entry.password_placeholder')}
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
                            <ActionTooltip content="Generate random password">
                              <button
                                type="button"
                                onClick={() => setShowGenerator(!showGenerator)}
                                className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-md border transition-colors ${
                                  showGenerator
                                    ? 'border-[var(--text-primary)] bg-[var(--bg-active)] text-[var(--text-primary)]'
                                    : 'border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]'
                                }`}
                              >
                                <Wand2 size={15} />
                              </button>
                            </ActionTooltip>
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
                    } else if (id === 'passkey') {
                      icon = <Fingerprint size={13} />;
                      label = 'Passkey (ES256)';
                      content = (
                        <div className="rounded-md border border-[var(--border-subtle)] bg-[var(--bg-base)] p-3 text-[12px] text-[var(--text-secondary)]">
                          {isEdit && form.hasPasskey ? (
                            <div className="flex items-center gap-2">
                              <div className="h-2 w-2 rounded-full bg-green-500 animate-pulse" />
                              <span>{t('entry.passkey_active_desc')}</span>
                            </div>
                          ) : (
                            <div className="flex items-center gap-2">
                              <div className="h-2 w-2 rounded-full bg-amber-500 animate-pulse" />
                              <span>{t('entry.passkey_new_desc')}</span>
                            </div>
                          )}
                        </div>
                      );
                    }
                  } else {
                    const cf = form.customFields.find(f => f.id === id);
                    if (!cf) return null;
                    onRemove = () => removeCustomField(id);

                    let cfIcon = <FileText size={13} />;
                    if (cf.type === 'password') cfIcon = <Key size={13} />;
                    else if (cf.type === 'email') cfIcon = <Mail size={13} />;
                    else if (cf.type === 'url') cfIcon = <Globe size={13} />;
                    else if (cf.type === 'username') cfIcon = <User size={13} />;
                    else if (cf.type === 'totp') cfIcon = <ShieldCheck size={13} />;

                    icon = cfIcon;
                    label = cf.name || t('entry.custom_field');

                    const suffix = getFieldSuffix(cf.type);
                    const currentPrefix = getPrefix(cf.name, suffix);
                    const isPasswordType = cf.type === 'password';

                    content = (
                      <div className="flex gap-1.5 items-center w-full">
                        <input
                          type="text"
                          value={currentPrefix}
                          onChange={(e) => {
                            const newPrefix = e.target.value;
                            const otherFields = form.customFields.filter(f => f.type === cf.type && f.id !== cf.id);
                            const count = otherFields.length + (fieldsOrder.includes(cf.type as any) ? 1 : 0);
                            const defaultOrd = getOrdinal(count);
                            const updatedName = formatFullName(newPrefix, suffix, defaultOrd);
                            updateCustomField(cf.id, { name: updatedName });
                          }}
                          placeholder={currentPrefix || suffix}
                          className="h-9 w-[120px] rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)] font-medium shrink-0"
                        />
                        <select
                          value={cf.type || 'text'}
                          onChange={(e) => {
                            const newType = e.target.value as FieldType;
                            const newSuffix = getFieldSuffix(newType);
                            const otherFields = form.customFields.filter(f => f.type === newType && f.id !== cf.id);
                            const count = otherFields.length + (fieldsOrder.includes(newType as any) ? 1 : 0);
                            const defaultOrd = getOrdinal(count);
                            const updatedName = formatFullName(currentPrefix, newSuffix, defaultOrd);
                            updateCustomField(cf.id, { type: newType, name: updatedName });
                          }}
                          className="h-9 rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-2 text-[11px] text-[var(--text-secondary)] outline-none focus:border-[var(--border-focus)] shrink-0"
                        >
                          <option value="password">{t('detail.password')}</option>
                          <option value="email">{t('detail.email')}</option>
                          <option value="username">{t('detail.username')}</option>
                          <option value="url">{t('detail.url')}</option>
                          <option value="notes">{t('preset.secure_note')}</option>
                          <option value="totp">{t('detail.totp')}</option>
                          <option value="text">{t('entry.custom_field')}</option>
                        </select>
                        <div className="relative flex-1 min-w-0">
                          <input
                            type={isPasswordType && !showCustomPasswords[cf.id] ? 'password' : 'text'}
                            value={cf.value}
                            onChange={(e) => updateCustomField(cf.id, { value: e.target.value })}
                            placeholder={isPasswordType ? '••••••••' : 'Value'}
                            className="h-9 w-full rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] pl-2.5 pr-8 text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--border-focus)]"
                          />
                          {isPasswordType && (
                            <ActionTooltip content={showCustomPasswords[cf.id] ? t('login.hide_password') : t('login.show_password')}>
                              <button
                                type="button"
                                onClick={() => setShowCustomPasswords(prev => ({ ...prev, [cf.id]: !prev[cf.id] }))}
                                className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors p-0.5"
                              >
                                {showCustomPasswords[cf.id] ? <EyeOff size={14} /> : <Eye size={14} />}
                              </button>
                            </ActionTooltip>
                          )}
                        </div>
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
                        <ActionTooltip content={t('common.delete')}>
                          <button
                            type="button"
                            onClick={onRemove}
                            className="rounded p-0.5 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-red-400 transition-colors"
                          >
                            <X size={12} />
                          </button>
                        </ActionTooltip>
                      </div>

                      {content}
                    </Reorder.Item>
                  );
                })}
              </Reorder.Group>

              {/* Tags */}
              <div className="flex flex-col gap-1.5 mt-1">
                <label className="text-[12px] font-medium text-[var(--text-secondary)]">{t('sidebar.tags')}</label>
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
                            ? 'text-white shadow-sm opacity-100'
                            : 'bg-[var(--bg-elevated)] text-[var(--text-tertiary)] opacity-40 border border-[var(--border-subtle)] hover:opacity-80 hover:text-[var(--text-secondary)]'
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

                  <ActionTooltip content="Create new tag">
                    <button
                      type="button"
                      onClick={() => setShowCreateTagModal(true)}
                      className="flex items-center justify-center rounded-md border border-dashed border-[var(--border)] px-2 py-1 text-[12px] font-medium text-[var(--text-tertiary)] opacity-60 transition-all hover:opacity-100 hover:border-[var(--border-focus)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                    >
                      <Plus size={13} />
                    </button>
                  </ActionTooltip>
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
                      <Plus size={13} /> {t('entry.add_field')}
                    </button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="start" className="w-[220px] max-h-[340px] overflow-y-auto z-50">
                    <DropdownMenuItem
                      onSelect={() => handleAddField('username')}
                      className="flex items-center gap-2 text-[12px] cursor-pointer"
                    >
                      <User size={13} />
                      <span>{getDropdownLabel('username', t('detail.username'))}</span>
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onSelect={() => handleAddField('password')}
                      className="flex items-center gap-2 text-[12px] cursor-pointer"
                    >
                      <Key size={13} />
                      <span>{getDropdownLabel('password', t('detail.password'))}</span>
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onSelect={() => handleAddField('email')}
                      className="flex items-center gap-2 text-[12px] cursor-pointer"
                    >
                      <Mail size={13} />
                      <span>{getDropdownLabel('email', t('detail.email'))}</span>
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onSelect={() => handleAddField('url')}
                      className="flex items-center gap-2 text-[12px] cursor-pointer"
                    >
                      <Globe size={13} />
                      <span>{getDropdownLabel('url', t('detail.url'))}</span>
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onSelect={() => handleAddField('totpSecret')}
                      className="flex items-center gap-2 text-[12px] cursor-pointer"
                    >
                      <ShieldCheck size={13} />
                      <span>{getDropdownLabel('totpSecret', t('detail.totp'))}</span>
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onSelect={() => handleAddField('notes')}
                      className="flex items-center gap-2 text-[12px] cursor-pointer"
                    >
                      <FileText size={13} />
                      <span>{getDropdownLabel('notes', t('preset.secure_note'))}</span>
                    </DropdownMenuItem>
                    {!fieldsOrder.includes('passkey') && (
                      <DropdownMenuItem
                        onSelect={() => handleAddField('passkey')}
                        className="flex items-center gap-2 text-[12px] cursor-pointer"
                      >
                        <Fingerprint size={13} />
                        <span>Passkey (ES256)</span>
                      </DropdownMenuItem>
                    )}

                    <DropdownMenuSeparator />

                    <DropdownMenuItem
                      onSelect={() => handleAddField('text')}
                      className="flex items-center gap-2 text-[12px] cursor-pointer"
                    >
                      <Plus size={13} />
                      <span>{t('entry.custom_field')}</span>
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>

              {/* Actions */}
              <div className="flex justify-between border-t border-[var(--border-subtle)] pt-4 mt-2">
                <label className="flex items-center gap-2 text-[12px] text-[var(--text-secondary)] cursor-pointer select-none hover:text-[var(--text-primary)] transition-colors">
                  <input
                    type="checkbox"
                    checked={form.favorite}
                    onChange={(e) => updateField('favorite', e.target.checked)}
                    className="accent-[var(--accent)] h-3.5 w-3.5 cursor-pointer"
                  />
                  {t('detail.favorite')}
                </label>

                <div className="flex gap-2">
                  <button
                    type="button"
                    onClick={onClose}
                    className="h-9 rounded-md px-4 text-[13px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)]"
                  >
                    {t('common.cancel')}
                  </button>
                  <button
                    type="submit"
                    disabled={loading}
                    className="flex h-9 items-center gap-2 rounded-md bg-[var(--text-primary)] px-5 text-[13px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 disabled:opacity-50"
                  >
                    {loading ? (
                      <>
                        <Loader2 size={14} className="animate-spin" />
                        {t('common.loading')}
                      </>
                    ) : isEdit ? (
                      t('common.save')
                    ) : (
                      t('list.new_entry')
                    )}
                  </button>
                </div>
              </div>
            </form>
          </motion.div>
        </motion.div>
      )}
      <CreateTagModal
        open={showCreateTagModal}
        onClose={() => setShowCreateTagModal(false)}
      />
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
          <ActionTooltip content={`Remove ${label}`}>
            <button
              type="button"
              onClick={onRemove}
              className="rounded p-0.5 text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-red-400 transition-colors"
            >
              <X size={12} />
            </button>
          </ActionTooltip>
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



