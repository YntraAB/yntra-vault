import React, { useState, useEffect, useCallback } from 'react';
import { usePasswordGenerator } from '../hooks/usePasswordGenerator';
import type { GeneratorOptions } from '@/lib/backend';
import { PasswordStrength, BreachIndicator } from '@/features/audit';
import { useTranslation } from '@/contexts/LanguageContext';
import { CopyButton } from '@/components/ui';
import { RefreshCw, Shuffle, BookOpen, ChevronDown, ChevronUp } from 'lucide-react';
import { ActionTooltip } from '@/components/ui/tooltip';
import { DOMAIN_RULES, getDomainName } from '../presets';

export interface PasswordGeneratorProps {
  url?: string;
  onSelect?: (password: string) => void;
  onClose?: () => void;
}

export const PasswordGenerator: React.FC<PasswordGeneratorProps> = ({
  url = '',
  onSelect,
  onClose,
}) => {
  const { t } = useTranslation();
  const { password, generate } = usePasswordGenerator();
  const [mode, setMode] = useState<'Random' | 'Diceware'>('Random');
  const [showOptions, setShowOptions] = useState(false);

  // Random options
  const [length, setLength] = useState(20);
  const [uppercase, setUppercase] = useState(true);
  const [lowercase, setLowercase] = useState(true);
  const [digits, setDigits] = useState(true);
  const [symbols, setSymbols] = useState(true);
  const [excludeAmbiguous, setExcludeAmbiguous] = useState(true);

  // Diceware options
  const [wordCount, setWordCount] = useState(5);
  const [separator, setSeparator] = useState('-');
  const [capitalizeWords, setCapitalizeWords] = useState(true);
  const [addNumber, setAddNumber] = useState(true);

  // Find matching preset
  const getPreset = useCallback(() => {
    if (!url) return null;
    const domain = getDomainName(url);
    if (!domain) return null;
    
    for (const rule of DOMAIN_RULES) {
      if (rule.domains.some(d => domain === d || domain.endsWith('.' + d))) {
        return { rule, domain };
      }
    }
    
    if (domain.includes('bank') || domain.includes('finance') || domain.includes('swish')) {
      return {
        rule: {
          domains: [],
          length: 16,
          uppercase: true,
          lowercase: true,
          digits: true,
          symbols: false,
          excludeAmbiguous: true,
          displayName: 'Bank Preset (Alphanumeric max 16)'
        },
        domain
      };
    }
    return null;
  }, [url]);

  const presetInfo = getPreset();

  // Apply preset rule on mount / url update
  useEffect(() => {
    if (presetInfo) {
      setLength(presetInfo.rule.length);
      setUppercase(presetInfo.rule.uppercase);
      setLowercase(presetInfo.rule.lowercase);
      setDigits(presetInfo.rule.digits);
      setSymbols(presetInfo.rule.symbols);
      setExcludeAmbiguous(presetInfo.rule.excludeAmbiguous);
      setMode('Random');
    } else {
      setLength(20);
      setUppercase(true);
      setLowercase(true);
      setDigits(true);
      setSymbols(true);
      setExcludeAmbiguous(true);
      setMode('Random');
    }
  }, [url, getPreset]);

  const buildOptions = useCallback((): GeneratorOptions => ({
    mode,
    length,
    uppercase,
    lowercase,
    digits,
    symbols,
    exclude_ambiguous: excludeAmbiguous,
    custom_symbols: null,
    word_count: wordCount,
    separator,
    capitalize_words: capitalizeWords,
    add_number: addNumber,
  }), [mode, length, uppercase, lowercase, digits, symbols, excludeAmbiguous, wordCount, separator, capitalizeWords, addNumber]);

  // Generate when settings update
  useEffect(() => {
    generate(buildOptions());
  }, [buildOptions, generate]);

  const handleSelect = () => {
    if (onSelect && password) {
      onSelect(password);
      onClose?.();
    }
  };

  return (
    <div className="flex flex-col gap-3 p-3 bg-[var(--bg-elevated)] rounded-[3px] select-none">
      {/* Password Display Box */}
      <div className="flex items-center rounded-[3px] border border-[var(--border-subtle)] bg-[var(--bg-base)] px-3 py-2">
        <code className="flex-1 break-all font-mono text-[13px] text-[var(--text-primary)] select-all tracking-wide">
          {password || '...'}
        </code>
        <div className="flex items-center gap-1 shrink-0 ml-2">
          <ActionTooltip content={t('generator.regenerate_tooltip')}>
            <button
              type="button"
              onClick={() => generate(buildOptions())}
              className="rounded-[3px] p-1 text-[var(--text-secondary)] hover:bg-[var(--bg-active)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
            >
              <RefreshCw size={13} />
            </button>
          </ActionTooltip>
          {password && <CopyButton value={password} size={13} />}
        </div>
      </div>

      {/* Strength & Breach Indicators */}
      {password && (
        <div className="flex flex-col gap-1.5 px-0.5">
          <PasswordStrength password={password} compact />
          <BreachIndicator password={password} compact />
        </div>
      )}

      {/* Domain Preset Info & Options Expand Row */}
      <div className="flex items-center justify-between border-t border-[var(--border-subtle)] pt-2 mt-1 px-0.5">
        {presetInfo ? (
          <div className="text-[11px] text-amber-500 font-medium">
            Smart Preset: {presetInfo.rule.displayName}
          </div>
        ) : (
          <div className="text-[11px] text-[var(--text-tertiary)] font-medium">
            {t('generator.std_preset')}
          </div>
        )}
        <ActionTooltip content={showOptions ? t('login.hide_password') : t('login.show_password')}>
          <button
            type="button"
            onClick={() => setShowOptions(!showOptions)}
            className="flex items-center gap-1 text-[11px] text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors font-medium cursor-pointer"
          >
            <span>{t('generator.options')}</span>
            {showOptions ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
          </button>
        </ActionTooltip>
      </div>

      {/* Collapsable Options Panel */}
      {showOptions && (
        <div className="flex flex-col gap-3 rounded-[3px] border border-[var(--border-subtle)] bg-[var(--bg-base)] p-3 mt-1">
          {/* Mode Tabs */}
          <div className="flex rounded-[3px] border border-[var(--border-subtle)] p-0.5 bg-[var(--bg-elevated)]">
            <button
              type="button"
              onClick={() => setMode('Random')}
              className={`flex flex-1 items-center justify-center gap-1 rounded-[2px] py-1 text-[11px] font-medium transition-colors cursor-pointer ${
                mode === 'Random' ? 'bg-[var(--bg-base)] text-[var(--text-primary)] shadow-xs' : 'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]'
              }`}
            >
              <Shuffle size={11} /> {t('generator.mode_random')}
            </button>
            <button
              type="button"
              onClick={() => setMode('Diceware')}
              className={`flex flex-1 items-center justify-center gap-1 rounded-[2px] py-1 text-[11px] font-medium transition-colors cursor-pointer ${
                mode === 'Diceware' ? 'bg-[var(--bg-base)] text-[var(--text-primary)] shadow-xs' : 'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]'
              }`}
            >
              <BookOpen size={11} /> {t('generator.mode_diceware')}
            </button>
          </div>

          {mode === 'Random' ? (
            <div className="flex flex-col gap-3">
              {/* Length Slider */}
              <div className="flex items-center gap-3">
                <label className="text-[11px] text-[var(--text-secondary)] font-medium min-w-[45px]">{t('generator.length')}</label>
                <input
                  type="range"
                  min={8}
                  max={64}
                  value={length}
                  onChange={(e) => setLength(Number(e.target.value))}
                  className="flex-1 h-2 rounded-full bg-[var(--border)] appearance-none outline-none cursor-pointer accent-[var(--accent)]"
                />
                <span className="text-[11px] text-[var(--text-primary)] font-mono min-w-[20px] text-right font-medium">
                  {length}
                </span>
              </div>

              {/* Toggles */}
              <div className="grid grid-cols-2 gap-2 mt-1">
                <Toggle label={t('generator.uppercase')} checked={uppercase} onChange={setUppercase} />
                <Toggle label={t('generator.lowercase')} checked={lowercase} onChange={setLowercase} />
                <Toggle label={t('generator.numbers')} checked={digits} onChange={setDigits} />
                <Toggle label={t('generator.symbols')} checked={symbols} onChange={setSymbols} />
                <Toggle label={t('generator.exclude_ambiguous')} checked={excludeAmbiguous} onChange={setExcludeAmbiguous} />
              </div>
            </div>
          ) : (
            <div className="flex flex-col gap-3">
              {/* Word Count Slider */}
              <div className="flex items-center gap-3">
                <label className="text-[11px] text-[var(--text-secondary)] font-medium min-w-[45px]">{t('generator.words')}</label>
                <input
                  type="range"
                  min={3}
                  max={10}
                  value={wordCount}
                  onChange={(e) => setWordCount(Number(e.target.value))}
                  className="flex-1 h-2 rounded-full bg-[var(--border)] appearance-none outline-none cursor-pointer accent-[var(--accent)]"
                />
                <span className="text-[11px] text-[var(--text-primary)] font-mono min-w-[20px] text-right font-medium">
                  {wordCount}
                </span>
              </div>

              {/* Separator selection */}
              <div className="flex items-center justify-between mt-1">
                <label className="text-[11px] text-[var(--text-secondary)] font-medium">{t('generator.separator')}</label>
                <div className="flex gap-1">
                  {['-', '.', '_', ' '].map((sep) => (
                    <button
                      type="button"
                      key={sep}
                      onClick={() => setSeparator(sep)}
                      className={`rounded-[3px] px-2 py-0.5 text-[10px] font-mono transition-colors cursor-pointer ${
                        separator === sep
                          ? 'bg-[var(--text-primary)] text-[var(--bg-base)] font-bold'
                          : 'bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
                      }`}
                    >
                      {sep === ' ' ? 'space' : sep}
                    </button>
                  ))}
                </div>
              </div>

              <div className="grid grid-cols-2 gap-2 mt-1">
                <Toggle label={t('generator.capitalize_words')} checked={capitalizeWords} onChange={setCapitalizeWords} />
                <Toggle label={t('generator.include_number')} checked={addNumber} onChange={setAddNumber} />
              </div>
            </div>
          )}
        </div>
      )}

      {/* Select password button */}
      {onSelect && (
        <button
          type="button"
          onClick={handleSelect}
          className="mt-1 rounded-[3px] bg-[var(--text-primary)] py-2 text-[12px] font-medium text-[var(--bg-base)] transition-colors hover:opacity-90 cursor-pointer"
        >
          {t('generator.use_password')}
        </button>
      )}
    </div>
  );
};

const Toggle: React.FC<{
  label: string;
  checked: boolean;
  onChange: (val: boolean) => void;
}> = ({ label, checked, onChange }) => (
  <label className="flex items-center gap-2 cursor-pointer select-none">
    <input
      type="checkbox"
      checked={checked}
      onChange={(e) => onChange(e.target.checked)}
      className="accent-[var(--text-primary)] h-3.5 w-3.5 rounded bg-[var(--bg-elevated)] border-[var(--border)]"
    />
    <span className="text-[11px] text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors">{label}</span>
  </label>
);

export default PasswordGenerator;
