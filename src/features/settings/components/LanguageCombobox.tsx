import { useState, useRef, useEffect, useMemo } from 'react';
import { Check, ChevronsUpDown, Search } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';
import { LANGUAGES } from '@/i18n/languages';
import { cn } from '@/lib/utils';

export interface LanguageComboboxProps {
  className?: string;
}

export function LanguageCombobox({ className }: LanguageComboboxProps) {
  const { language, setLanguage, currentLanguage, t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape' && open) {
        setOpen(false);
      }
    }

    if (open) {
      document.addEventListener('mousedown', handleClickOutside);
      document.addEventListener('keydown', handleKeyDown);
      setTimeout(() => inputRef.current?.focus(), 50);
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [open]);

  const filteredLanguages = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return LANGUAGES;
    return LANGUAGES.filter(
      (lang) =>
        lang.name.toLowerCase().includes(q) ||
        lang.nativeName.toLowerCase().includes(q) ||
        lang.code.toLowerCase().includes(q)
    );
  }, [search]);

  return (
    <div ref={containerRef} className={cn('relative w-full', className)}>
      <button
        type="button"
        role="combobox"
        aria-expanded={open}
        onClick={() => {
          setOpen((prev) => !prev);
          setSearch('');
        }}
        className={cn(
          'flex h-8 w-full items-center justify-between rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 py-2 text-[12px] text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] focus:outline-none focus:ring-0 focus-visible:outline-none focus-visible:ring-0 outline-none ring-0 cursor-pointer select-none'
        )}
      >
        <div className="flex items-center gap-2 truncate">
          <span className="text-base leading-none">{currentLanguage.flag}</span>
          <span title={currentLanguage.nativeName} className="font-medium truncate">{currentLanguage.nativeName}</span>
          {currentLanguage.name !== currentLanguage.nativeName && (
            <span className="text-xs text-[var(--text-secondary)] truncate">({currentLanguage.name})</span>
          )}
        </div>
        <ChevronsUpDown className="ml-2 h-4 w-4 shrink-0 opacity-50 text-[var(--text-secondary)]" />
      </button>

      {open && (
        <div className="absolute left-0 top-full z-50 mt-1 w-full overflow-hidden rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-3 shadow-xl outline-none ring-0 animate-in fade-in-0 zoom-in-95 select-none focus:outline-none focus:ring-0">
          <div className="flex items-center px-3 border-b border-[var(--border)]">
            <Search className="mr-2 h-4 w-4 shrink-0 opacity-50 text-[var(--text-secondary)]" />
            <input
              ref={inputRef}
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={t('settings.language_search')}
              className="h-8 w-full rounded-[3px] bg-[var(--bg-base)] text-[12px] text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] focus:outline-none outline-none border-none ring-0"
            />
          </div>

          <div className="max-h-[260px] overflow-y-auto pt-2">
            {filteredLanguages.length === 0 ? (
              <div className="py-6 text-center text-xs text-[var(--text-secondary)]">
                {t('settings.language_not_found')}
              </div>
            ) : (
              <div className="grid grid-cols-2 gap-2">
                {filteredLanguages.map((lang) => {
                  const isSelected = lang.code.toLowerCase() === language.toLowerCase();
                  return (
                    <button
                      key={lang.code}
                      type="button"
                      onClick={() => {
                        setLanguage(lang.code);
                        setOpen(false);
                      }}
                      className={cn(
                        'w-full flex items-center justify-between rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-2.5 py-2 text-left text-[12px] text-[var(--text-primary)] cursor-pointer transition-colors hover:bg-[var(--bg-hover)]',
                        isSelected && 'border-[var(--border-focus)] bg-[var(--bg-active)] font-medium'
                      )}
                    >
                      <div className="flex items-center gap-2.5 truncate">
                        <span className="text-[16px] leading-none">{lang.flag}</span>
                        <div className="flex flex-col truncate">
                            <span title={lang.nativeName} className="text-[12px] leading-tight text-[var(--text-primary)] truncate">
                            {lang.nativeName}
                          </span>
                          {lang.name !== lang.nativeName && (
                            <span title={lang.name} className="text-[10px] text-[var(--text-tertiary)] truncate">
                              {lang.name}
                            </span>
                          )}
                        </div>
                      </div>
                      {isSelected && <Check className="h-4 w-4 shrink-0 text-[var(--text-primary)]" />}
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

export default LanguageCombobox;
