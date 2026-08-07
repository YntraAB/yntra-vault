import { useState, useMemo } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { useNavigate } from 'react-router-dom';
import { Moon, Sun, Monitor, Check, ArrowRight, ArrowLeft, Clock, Clipboard, ChevronDown, ChevronUp, Search, X } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';
import { useTheme } from '@/contexts/ThemeContext';
import { useAppState } from '@/contexts/AppStateContext';

export default function Onboarding() {
  const navigate = useNavigate();
  const { t, language, setLanguage, languages } = useTranslation();
  const { theme, setTheme } = useTheme();
  const { settings, updateSettings } = useAppState();

  const [step, setStep] = useState(0);
  const [showAllLanguages, setShowAllLanguages] = useState(false);
  const [langSearch, setLangSearch] = useState('');

  const [autoLockMinutes, setAutoLockMinutes] = useState(settings?.autoLockMinutes ?? 15);
  const [clipboardClearSeconds, setClipboardClearSeconds] = useState(settings?.clipboardClearSeconds ?? 30);

  const filteredLanguages = useMemo(() => {
    let list = languages;
    if (langSearch.trim()) {
      const query = langSearch.toLowerCase().trim();
      list = languages.filter(
        (l) =>
          l.name.toLowerCase().includes(query) ||
          l.nativeName.toLowerCase().includes(query) ||
          l.code.toLowerCase().includes(query)
      );
    }
    // Pin currently selected language to top of list
    return [...list].sort((a, b) => {
      if (a.code === language) return -1;
      if (b.code === language) return 1;
      return 0;
    });
  }, [languages, langSearch, language]);

  const handleFinish = () => {
    updateSettings({
      autoLockMinutes,
      clipboardClearSeconds,
    });
    localStorage.setItem('yntra-vault-setup-completed', 'true');
    navigate('/');
  };

  const stepTitles = [
    t('onboarding.step_language'),
    t('onboarding.step_theme'),
    t('onboarding.step_security'),
    t('onboarding.ready_title'),
  ];

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      className="flex h-screen w-screen items-center justify-center bg-[var(--bg-base)] select-none"
    >
      <div className="w-[420px] px-6">
        {/* Top Header Row with App Logo & Skip */}
        <div className="relative flex flex-col items-center text-center">
          <div className="absolute right-0 top-0">
            <button
              type="button"
              onClick={handleFinish}
              className="text-[12px] font-medium text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
            >
              {t('onboarding.skip')}
            </button>
          </div>

          <img
            src="/white-logo.png"
            alt="Yntra Vault Logo"
            className="mb-3 h-20 w-20 rounded-xl object-cover shadow-md"
          />
          <h1 className="text-[20px] font-semibold tracking-tight text-[var(--text-primary)]">
            {t('onboarding.welcome_title')}
          </h1>
          <p className="mt-1 text-[13px] text-[var(--text-secondary)] leading-snug">
            {t('onboarding.welcome_sub')}
          </p>
        </div>

        {/* Step Indicator Bar */}
        <div className="mt-6 mb-5 flex items-center justify-between gap-1.5 px-1">
          {stepTitles.map((title, idx) => (
            <div key={idx} className="flex flex-1 flex-col items-center gap-1.5 min-w-0">
              <div
                className={`h-1 w-full rounded-full transition-colors ${
                  idx <= step ? 'bg-[var(--text-primary)]' : 'bg-[var(--border-subtle)]'
                }`}
              />
              <span
                className={`text-[10px] font-medium transition-colors whitespace-nowrap truncate ${
                  idx === step ? 'text-[var(--text-primary)]' : 'text-[var(--text-tertiary)]'
                }`}
              >
                {title}
              </span>
            </div>
          ))}
        </div>

        {/* Card Content Area */}
        <div className="rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] p-4 shadow-sm min-h-[235px] flex flex-col justify-between">
          <AnimatePresence mode="wait">
            {/* STEP 0: LANGUAGE */}
            {step === 0 && (
              <motion.div
                key="step-language"
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -6 }}
                transition={{ duration: 0.15 }}
                className="flex flex-col gap-3"
              >
                <div>
                  <h2 className="text-[14px] font-medium text-[var(--text-primary)]">
                    {t('onboarding.step_language')}
                  </h2>
                  <p className="text-[12px] text-[var(--text-secondary)]">
                    {t('onboarding.step_language_desc')}
                  </p>
                </div>

                {/* Search Input */}
                <div className="relative mt-0.5">
                  <Search size={14} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)]" />
                  <input
                    type="text"
                    value={langSearch}
                    onChange={(e) => setLangSearch(e.target.value)}
                    placeholder={t('onboarding.search_languages_ph')}
                    className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] pl-8 pr-8 text-[12px] text-[var(--text-primary)] outline-none placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] transition-colors"
                  />
                  {langSearch && (
                    <button
                      type="button"
                      onClick={() => setLangSearch('')}
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors"
                    >
                      <X size={13} />
                    </button>
                  )}
                </div>

                {filteredLanguages.length === 0 ? (
                  <div className="py-6 text-center text-[12px] text-[var(--text-tertiary)] font-medium">
                    {t('onboarding.no_languages_found')}
                  </div>
                ) : (
                  <>
                    <div className="relative">
                      <div
                        className={`grid grid-cols-2 gap-2 overflow-y-auto transition-all duration-300 pr-0.5 ${
                          showAllLanguages || langSearch ? 'max-h-[190px]' : 'max-h-[110px]'
                        }`}
                      >
                        {filteredLanguages.map((lang) => {
                          const isSelected = language === lang.code;
                          return (
                            <button
                              key={lang.code}
                              type="button"
                              onClick={() => setLanguage(lang.code)}
                              className={`flex items-center justify-between rounded-[3px] border px-3 py-2 text-left transition-colors ${
                                isSelected
                                  ? 'border-[var(--border-focus)] bg-[var(--bg-active)]'
                                  : 'border-[var(--border)] bg-[var(--bg-base)] hover:bg-[var(--bg-hover)]'
                              }`}
                            >
                              <div className="flex items-center gap-2 min-w-0">
                                <span className="text-[16px]">{lang.flag}</span>
                                <div className="truncate">
                                  <p className="text-[12px] font-medium text-[var(--text-primary)] truncate">
                                    {lang.nativeName}
                                  </p>
                                  <p className="text-[10px] text-[var(--text-tertiary)] truncate">{lang.name}</p>
                                </div>
                              </div>
                              {isSelected && <Check size={14} className="text-[var(--text-primary)] shrink-0 ml-1" />}
                            </button>
                          );
                        })}
                      </div>

                      {!showAllLanguages && !langSearch && languages.length > 4 && (
                        <div className="absolute bottom-0 left-0 right-0 h-6 bg-gradient-to-t from-[var(--bg-elevated)] to-transparent pointer-events-none" />
                      )}
                    </div>

                    {!langSearch && languages.length > 4 && (
                      <button
                        type="button"
                        onClick={() => setShowAllLanguages(!showAllLanguages)}
                        className="flex items-center justify-center gap-1.5 self-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] px-3 py-1 text-[11px] font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors mt-0.5 cursor-pointer"
                      >
                        <span>{showAllLanguages ? t('onboarding.show_less_langs') : t('onboarding.show_more_langs')}</span>
                        {showAllLanguages ? <ChevronUp size={13} /> : <ChevronDown size={13} />}
                      </button>
                    )}
                  </>
                )}
              </motion.div>
            )}

            {/* STEP 1: THEME */}
            {step === 1 && (
              <motion.div
                key="step-theme"
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -6 }}
                transition={{ duration: 0.15 }}
                className="flex flex-col gap-3"
              >
                <div>
                  <h2 className="text-[14px] font-medium text-[var(--text-primary)]">
                    {t('onboarding.step_theme')}
                  </h2>
                  <p className="text-[12px] text-[var(--text-secondary)]">
                    {t('onboarding.step_theme_desc')}
                  </p>
                </div>

                <div className="grid grid-cols-3 gap-2.5 mt-1">
                  {[
                    { id: 'dark', label: t('settings.theme_dark'), icon: Moon, swatchBg: 'bg-[#18181b]', swatchBorder: 'border-zinc-700', textCol: 'text-zinc-300' },
                    { id: 'light', label: t('settings.theme_light'), icon: Sun, swatchBg: 'bg-[#f4f4f5]', swatchBorder: 'border-zinc-300', textCol: 'text-zinc-700' },
                    { id: 'system', label: t('settings.theme_system'), icon: Monitor, swatchBg: 'bg-zinc-500/20 dark:bg-zinc-700/40', swatchBorder: 'border-zinc-500/40', textCol: 'text-[var(--text-secondary)]' },
                  ].map((item) => {
                    const isSelected = theme === item.id;
                    const Icon = item.icon;
                    return (
                      <button
                        key={item.id}
                        type="button"
                        onClick={() => setTheme(item.id as any)}
                        className={`flex flex-col items-center justify-center h-24 rounded-[3px] border p-2 text-center transition-colors ${
                          isSelected
                            ? 'border-[var(--border-focus)] bg-[var(--bg-active)]'
                            : 'border-[var(--border)] bg-[var(--bg-base)] hover:bg-[var(--bg-hover)]'
                        }`}
                      >
                        {/* Mini Window Wireframe Swatch */}
                        <div className={`mb-2 flex h-8 w-12 items-center justify-center rounded border ${item.swatchBg} ${item.swatchBorder} shadow-xs`}>
                          <Icon size={14} className={item.textCol} />
                        </div>
                        <span className="text-[12px] font-medium text-[var(--text-primary)]">{item.label}</span>
                      </button>
                    );
                  })}
                </div>
              </motion.div>
            )}

            {/* STEP 2: SECURITY DEFAULTS */}
            {step === 2 && (
              <motion.div
                key="step-security"
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -6 }}
                transition={{ duration: 0.15 }}
                className="flex flex-col gap-3"
              >
                <div>
                  <h2 className="text-[14px] font-medium text-[var(--text-primary)]">
                    {t('onboarding.step_security')}
                  </h2>
                  <p className="text-[12px] text-[var(--text-secondary)]">
                    {t('onboarding.step_security_desc')}
                  </p>
                </div>

                <div className="flex flex-col gap-3 mt-0.5">
                  {/* Auto Lock */}
                  <div className="flex flex-col gap-1">
                    <div className="flex items-center gap-1.5 text-[12px] font-medium text-[var(--text-secondary)]">
                      <Clock size={13} className="text-[var(--text-tertiary)]" />
                      <span>{t('onboarding.autolock_label')}</span>
                    </div>
                    <p className="text-[10px] text-[var(--text-tertiary)] mb-1">
                      {t('onboarding.autolock_help')}
                    </p>
                    <div className="flex rounded-[3px] border border-[var(--border)]">
                      {[
                        { val: 5, label: '5 min' },
                        { val: 15, label: '15 min' },
                        { val: 30, label: '30 min' },
                        { val: 0, label: 'Never' },
                      ].map((opt) => (
                        <button
                          key={opt.val}
                          type="button"
                          onClick={() => setAutoLockMinutes(opt.val)}
                          className={`flex-1 py-1 text-[11px] font-medium transition-colors ${
                            autoLockMinutes === opt.val
                              ? 'bg-[var(--bg-active)] text-[var(--text-primary)]'
                              : 'bg-[var(--bg-base)] text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
                          }`}
                        >
                          {opt.label}
                        </button>
                      ))}
                    </div>
                  </div>

                  {/* Clipboard Clear */}
                  <div className="flex flex-col gap-1">
                    <div className="flex items-center gap-1.5 text-[12px] font-medium text-[var(--text-secondary)]">
                      <Clipboard size={13} className="text-[var(--text-tertiary)]" />
                      <span>{t('onboarding.clipboard_label')}</span>
                    </div>
                    <p className="text-[10px] text-[var(--text-tertiary)] mb-1">
                      {t('onboarding.clipboard_help')}
                    </p>
                    <div className="flex rounded-[3px] border border-[var(--border)]">
                      {[
                        { val: 15, label: '15 sec' },
                        { val: 30, label: '30 sec' },
                        { val: 60, label: '60 sec' },
                        { val: 0, label: 'Never' },
                      ].map((opt) => (
                        <button
                          key={opt.val}
                          type="button"
                          onClick={() => setClipboardClearSeconds(opt.val)}
                          className={`flex-1 py-1 text-[11px] font-medium transition-colors ${
                            clipboardClearSeconds === opt.val
                              ? 'bg-[var(--bg-active)] text-[var(--text-primary)]'
                              : 'bg-[var(--bg-base)] text-[var(--text-secondary)] hover:text-[var(--text-primary)]'
                          }`}
                        >
                          {opt.label}
                        </button>
                      ))}
                    </div>
                  </div>
                </div>
              </motion.div>
            )}

            {/* STEP 3: READY */}
            {step === 3 && (
              <motion.div
                key="step-ready"
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -6 }}
                transition={{ duration: 0.15 }}
                className="flex flex-col items-center justify-center text-center gap-2 py-4"
              >
                <div className="flex h-10 w-10 items-center justify-center rounded-full bg-green-500/10 text-green-500 border border-green-500/20">
                  <Check size={20} />
                </div>
                <h2 className="text-[15px] font-semibold text-[var(--text-primary)]">
                  {t('onboarding.ready_title')}
                </h2>
                <p className="text-[12px] text-[var(--text-secondary)] leading-relaxed">
                  {t('onboarding.ready_desc')}
                </p>
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        {/* Footer Navigation Buttons */}
        <div className="mt-4 flex items-center justify-between">
          {step > 0 ? (
            <button
              type="button"
              onClick={() => setStep(step - 1)}
              className="flex h-9 items-center gap-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
            >
              <ArrowLeft size={13} />
              {t('onboarding.back')}
            </button>
          ) : <div />}

          {step < 3 ? (
            <button
              type="button"
              onClick={() => setStep(step + 1)}
              className="flex h-9 items-center gap-1 rounded-[3px] bg-[var(--text-primary)] px-4 text-[12px] font-semibold text-[var(--bg-base)] transition-opacity hover:opacity-90 ml-auto"
            >
              {t('onboarding.next')}
              <ArrowRight size={13} />
            </button>
          ) : (
            <button
              type="button"
              onClick={handleFinish}
              className="flex h-9 items-center justify-center gap-1 rounded-[3px] bg-[var(--text-primary)] px-5 text-[13px] font-semibold text-[var(--bg-base)] transition-opacity hover:opacity-90 ml-auto"
            >
              {t('onboarding.get_started')}
              <ArrowRight size={14} />
            </button>
          )}
        </div>
      </div>
    </motion.div>
  );
}
