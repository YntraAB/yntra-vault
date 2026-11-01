import { useTranslation } from '@/contexts/LanguageContext';

export function TagDefaultUrl({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  const { t } = useTranslation();
  return <label className="flex flex-col gap-1.5 text-[12px] text-[var(--text-secondary)]">
    {t('tags.default_url')}
    <input className="h-8 rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 outline-none focus:border-[var(--border-focus)]" value={value} onChange={event => onChange(event.target.value)} placeholder="https://gmail.com" maxLength={2048} autoCapitalize="none" spellCheck={false} />
    <span className="text-[11px] text-[var(--text-tertiary)]">{t('tags.default_url_hint')}</span>
  </label>;
}
