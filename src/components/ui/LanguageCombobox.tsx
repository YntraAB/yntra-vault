import { useState } from 'react';
import { Check, ChevronsUpDown } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';
import { LANGUAGES } from '@/i18n/languages';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from '@/components/ui/command';
import { cn } from '@/lib/utils';

interface LanguageComboboxProps {
  className?: string;
}

export function LanguageCombobox({ className }: LanguageComboboxProps) {
  const { language, setLanguage, currentLanguage, t } = useTranslation();
  const [open, setOpen] = useState(false);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          role="combobox"
          aria-expanded={open}
          className={cn(
            'flex h-10 w-full items-center justify-between rounded-lg border border-[var(--border)] bg-[var(--bg-surface)] px-3 py-2 text-sm text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-hover)] focus:outline-none focus:ring-0 focus-visible:outline-none focus-visible:ring-0 outline-none ring-0',
            className
          )}
        >
          <div className="flex items-center gap-2 truncate">
            <span className="text-base leading-none">{currentLanguage.flag}</span>
            <span className="font-medium truncate">{currentLanguage.nativeName}</span>
            {currentLanguage.name !== currentLanguage.nativeName && (
              <span className="text-xs text-[var(--text-secondary)] truncate">({currentLanguage.name})</span>
            )}
          </div>
          <ChevronsUpDown className="ml-2 h-4 w-4 shrink-0 opacity-50 text-[var(--text-secondary)]" />
        </button>
      </PopoverTrigger>

      <PopoverContent align="start" className="w-[320px] p-0 bg-[var(--bg-surface)] border-[var(--border)] shadow-xl rounded-lg overflow-hidden focus:outline-none focus:ring-0 outline-none ring-0">
        <Command className="bg-transparent text-[var(--text-primary)]">
          <CommandInput
            placeholder={t('settings.language_search')}
            className="h-10 text-sm text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] border-b border-[var(--border)] focus:outline-none focus:ring-0 focus-visible:outline-none focus-visible:ring-0 outline-none ring-0"
          />
          <CommandList className="max-h-[260px] overflow-y-auto p-1">
            <CommandEmpty className="py-6 text-center text-xs text-[var(--text-secondary)]">
              {t('settings.language_not_found')}
            </CommandEmpty>
            <CommandGroup>
              {LANGUAGES.map((lang) => {
                const isSelected = lang.code.toLowerCase() === language.toLowerCase();
                return (
                  <CommandItem
                    key={lang.code}
                    value={`${lang.name} ${lang.nativeName} ${lang.code}`}
                    onSelect={() => {
                      setLanguage(lang.code);
                      setOpen(false);
                    }}
                    className={cn(
                      'flex items-center justify-between px-3 py-2 text-sm rounded-md cursor-pointer transition-colors text-[var(--text-primary)] hover:bg-[var(--bg-hover)] data-[selected=true]:bg-[var(--bg-hover)]',
                      isSelected && 'bg-[var(--accent-subtle,#3b82f61a)] font-medium text-[var(--accent)]'
                    )}
                  >
                    <div className="flex items-center gap-2.5 truncate">
                      <span className="text-base leading-none">{lang.flag}</span>
                      <div className="flex flex-col truncate">
                        <span className="text-sm leading-tight text-[var(--text-primary)] truncate">{lang.nativeName}</span>
                        {lang.name !== lang.nativeName && (
                          <span className="text-[11px] text-[var(--text-secondary)] truncate">{lang.name}</span>
                        )}
                      </div>
                    </div>
                    {isSelected && <Check className="h-4 w-4 shrink-0 text-[var(--accent)]" />}
                  </CommandItem>
                );
              })}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
