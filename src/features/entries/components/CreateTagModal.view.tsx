import { normalizeTagUrl } from '../tagDefaults';
import { TagDefaultUrl } from './TagDefaultUrl';
import { useMutationGate } from '@/hooks/useMutationGate';
import { useModalFocus } from '@/hooks/useModalFocus';
import { MAX_DISPLAY_NAME_LENGTH } from '@/lib/displayLimits';
/**
 * CreateTagModal — Create a new tag (vault category)
 *
 * Name input, color picker (preset palette), validation.
 * Same modal pattern as CreateVaultModal / EntryModal.
 */

import { ActionTooltip } from '@/components/ui/tooltip';
import { useTranslation } from '@/contexts/LanguageContext';
import { useToast } from '@/contexts/ToastContext';
import { AnimatePresence,motion } from 'framer-motion';
import { Check,Tag as TagIcon,X } from 'lucide-react';
import { useCallback,useEffect,useRef,useState } from 'react';
import { useEntries } from '../context/EntriesContext';
import { type CreateTagModalProps,PRESET_COLORS } from './CreateTagModal.model';


export function CreateTagModal(props: CreateTagModalProps) { return <CreateTagForm key={String(props.open)} {...props} />; }
function CreateTagForm({ open, onClose: requestClose }: CreateTagModalProps) {
  const { t } = useTranslation();
  const { busy, run, close } = useMutationGate();
  const onClose = useCallback(() => close(requestClose), [close, requestClose]);
  const { addTag, tags } = useEntries();
  const { addToast } = useToast();
  const [name, setName] = useState('');
  const [color, setColor] = useState(() => PRESET_COLORS[Math.floor(Math.random() * PRESET_COLORS.length)]);
  const [defaultUrl, setDefaultUrl] = useState('');
  const [error, setError] = useState('');

  const nameRef = useRef<HTMLInputElement>(null);

  // Focus name field on open
  useEffect(() => {
    if (open) {
      const timer = setTimeout(() => nameRef.current?.focus(), 100);
      return () => clearTimeout(timer);
    }
  }, [open]);

  const dialogRef = useRef<HTMLDivElement>(null);
  useModalFocus(open, dialogRef, onClose);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (busy) return;

    let normalizedUrl: string;
    try { normalizedUrl = normalizeTagUrl(defaultUrl); } catch { setError(t('tags.invalid_url')); return; }
    const trimmed = name.trim();
    if (!trimmed) {
      setError(t('tags.err_name_req'));
      return;
    }
    if (trimmed.length < 2) {
      setError(t('tags.err_min_length'));
      return;
    }
    if (tags.some((t) => t.name.toLowerCase() === trimmed.toLowerCase())) {
      setError(t('tags.err_exists'));
      return;
    }

    const saved = await run(() => addTag({
      id: crypto.randomUUID(),
      name: trimmed,
      color,
      icon: 'tag',
      count: 0,
      default_url: normalizedUrl,
    }));
    if (!saved) return;

    addToast({ message: t('tags.toast_created', { name: trimmed }), type: 'success' });
    onClose();
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-50 flex items-start sm:items-center justify-center overflow-y-auto bg-black/50 p-3 sm:p-4 touch-pan-y overscroll-contain"
          onClick={onClose}
        >
          <motion.div
            ref={dialogRef}
            role="dialog"
            aria-modal="true"
            aria-label={t('tags.new_tag')}
            tabIndex={-1}
            initial={{ scale: 0.97, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.97, opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="w-full max-w-[380px] my-auto rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] shadow-2xl overflow-hidden flex flex-col max-h-[calc(100dvh-1.5rem)] sm:max-h-[85vh]"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between border-b border-[var(--border)] px-4 py-3 bg-[var(--bg-surface)]">
              <div className="flex items-center gap-2.5">
                <div className="flex h-7 w-7 items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-primary)]">
                  <TagIcon size={14} />
                </div>
                <h2 className="text-[13px] font-semibold text-[var(--text-primary)]">
                  {t('tags.new_tag')}
                </h2>
              </div>
              <ActionTooltip content={t('common.close')}>
                <button
                  aria-label={t('common.close')} disabled={busy}
                  onClick={onClose}
                  className="rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
                >
                  <X size={15} />
                </button>
              </ActionTooltip>
            </div>

            {/* Form */}
            <form aria-busy={busy} onSubmit={handleSubmit} className="flex flex-col gap-3.5 p-4">
              <fieldset disabled={busy} className="contents">
              {/* Name */}
              <div className="flex flex-col gap-1.5">
                <label className="text-[12px] font-medium text-[var(--text-secondary)]">
                  {t('tags.tag_name')}
                </label>
                <input
                  aria-label={t('tags.tag_name')}
                  ref={nameRef}
                  type="text"
                  maxLength={MAX_DISPLAY_NAME_LENGTH}
                    value={name}
                  onChange={(e) => {
                    setName(e.target.value);
                    setError('');
                  }}
                  placeholder={t('tag.name_ph')}
                  className={`h-8 w-full rounded-[3px] border bg-[var(--bg-base)] px-3 text-[12px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] transition-colors ${
                    error ? 'border-[var(--destructive)]' : 'border-[var(--border)]'
                  }`}
                />
                {error && (
                  <span role="alert" className="text-[11px] text-[var(--destructive)]">{error}</span>
                )}
              </div>

              <TagDefaultUrl value={defaultUrl} onChange={setDefaultUrl} />
              {/* Color */}
              <div className="flex flex-col gap-1.5">
                <label className="text-[12px] font-medium text-[var(--text-secondary)]">
                  {t('tags.color')}
                </label>
                <div className="flex flex-wrap gap-2">
                  {PRESET_COLORS.map((c) => (
                    <button
                      key={c}
                        aria-label={`${t('tags.color')}: ${c}`}
                        aria-pressed={color === c}
                      type="button"
                      onClick={() => setColor(c)}
                      className="flex h-6 w-6 items-center justify-center rounded-[3px] transition-transform hover:scale-105 cursor-pointer"
                      style={{
                        backgroundColor: c,
                        boxShadow: color === c ? '0 0 0 2px var(--bg-base), 0 0 0 3px var(--border-focus)' : 'none',
                      }}
                    >
                      {color === c && <Check size={13} className="text-white" />}
                    </button>
                  ))}
                </div>
              </div>

              {/* Preview */}
              <div className="flex items-center gap-2 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 py-2">
                <span
                  className="h-2 w-2 rounded-[2px]"
                  style={{ backgroundColor: color }}
                />
                <span className="text-[12px] font-medium text-[var(--text-primary)]">
                  {name.trim() || t('tags.tag_name')}
                </span>
                <span className="ml-auto text-[11px] tabular-nums text-[var(--text-tertiary)]">
                  0
                </span>
              </div>

              {/* Actions */}
              <div className="flex justify-end gap-2 pt-1 border-t border-[var(--border-subtle)] mt-1">
                <button
                  type="button"
                  onClick={onClose}
                  className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
                >
                  {t('common.cancel')}
                </button>
                <button
                  type="submit"
                  className="flex h-8 items-center gap-2 rounded-[3px] bg-[var(--text-primary)] px-3.5 text-[12px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 cursor-pointer"
                >
                  {t('sidebar.new_tag')}
                </button>
              </div>
            </fieldset>
            </form>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}


export default CreateTagModal;