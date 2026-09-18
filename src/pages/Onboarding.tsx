import { useState, useMemo } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { useNavigate } from 'react-router-dom';
import { Moon, Sun, Monitor, Check, ArrowRight, ArrowLeft, Clock, Clipboard, ChevronDown, ChevronUp, Search, X, FolderInput, Globe, Shield } from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';
import { useTheme } from '@/contexts/ThemeContext';
import { useSettings } from '@/features/settings';
import { ImportModal } from '@/features/sync';
import { isTauri, getBackend } from '@/lib/backend';

export default function Onboarding() {
  const navigate = useNavigate();
  const { t, language, setLanguage, languages } = useTranslation();
  const { theme, setTheme } = useTheme();
  const { settings, updateSettings } = useSettings();

  const [step, setStep] = useState(0);
  const [showAllLanguages, setShowAllLanguages] = useState(false);
  const [langSearch, setLangSearch] = useState('');
  const [showImportModal, setShowImportModal] = useState(false);

  const [operationMode, setOperationMode] = useState<'standard' | 'airgap'>(
    settings?.operationMode === 'airgap' ? 'airgap' : 'standard'
  );
  const [externalFaviconsEnabled, setExternalFaviconsEnabled] = useState(
    settings?.externalFaviconsEnabled ?? (settings?.operationMode === 'airgap' ? false : true)
  );

  const [autoLockMinutes, setAutoLockMinutes] = useState(settings?.autoLockMinutes ?? 15);
  const [clipboardClearSeconds, setClipboardClearSeconds] = useState(settings?.clipboardClearSeconds ?? 30);

  const handleSelectMode = (mode: 'standard' | 'airgap') => {
    setOperationMode(mode);
    if (mode === 'standard') {
      setExternalFaviconsEnabled(true);
    } else if (mode === 'airgap') {
      setExternalFaviconsEnabled(false);
    }
  };

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
      operationMode,
      externalFaviconsEnabled,
    });
    if (isTauri()) {
      getBackend().then((b) => {
        b.setExternalFaviconsEnabled(externalFaviconsEnabled).catch(() => {});
      }).catch(() => {});
    }
    localStorage.setItem('yntra-vault-setup-completed', 'true');
    navigate('/');
  };

  const stepTitles = [
    t('onboarding.step_language') || 'Language',
    t('onboarding.step_theme') || 'Theme',
    t('onboarding.step_mode') || 'Mode',
    t('onboarding.step_security') || 'Security',
    t('onboarding.step_import') || 'Import',
    t('onboarding.ready_title') || 'Ready',
  ];

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      className="flex min-h-dvh w-full justify-center bg-[var(--bg-base)] p-4 overflow-y-auto touch-pan-y overscroll-contain"
    >
      <div className="w-full max-w-[420px] py-4 my-auto">
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
            alt="Yntra Vault"
            className="mb-3 h-20 w-20 rounded-[3px] object-cover invert dark:invert-0"
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

                <div className="grid grid-cols-3 gap-2 mt-1">
                  {[
                    { id: 'dark' as const, label: t('settings.theme_dark'), icon: Moon },
                    { id: 'light' as const, label: t('settings.theme_light'), icon: Sun },
                    { id: 'system' as const, label: t('settings.theme_system'), icon: Monitor },
                  ].map((item) => {
                    const isSelected = theme === item.id;
                    const Icon = item.icon;
                    return (
                      <button
                        key={item.id}
                        type="button"
                        onClick={() => setTheme(item.id)}
                        className={`flex h-20 flex-col items-center justify-center gap-2 rounded-[3px] border text-[12px] font-medium transition-colors cursor-pointer select-none ${
                          isSelected
                            ? 'border-[var(--text-primary)] bg-[var(--bg-active)] text-[var(--text-primary)]'
                            : 'border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)] hover:border-[var(--border-focus)] hover:text-[var(--text-primary)]'
                        }`}
                      >
                        <Icon size={18} className={isSelected ? 'text-[var(--text-primary)]' : 'text-[var(--text-secondary)]'} />
                        <span>{item.label}</span>
                      </button>
                    );
                  })}
                </div>
              </motion.div>
            )}

            {/* STEP 2: OPERATION MODE */}
            {step === 2 && (
              <motion.div
                key="step-mode"
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -6 }}
                transition={{ duration: 0.15 }}
                className="flex flex-col gap-3"
              >
                <div>
                  <h2 className="text-[14px] font-medium text-[var(--text-primary)]">
                    {t('onboarding.step_mode')}
                  </h2>
                  <p className="text-[12px] text-[var(--text-secondary)]">
                    {t('onboarding.step_mode_desc')}
                  </p>
                </div>

                <div className="flex flex-col gap-2.5 mt-1">
                  {[
                    {
                      id: 'standard' as const,
                      title: t('onboarding.mode_standard_title') || 'Standard',
                      icon: Globe,
                      desc: t('onboarding.mode_standard_desc') || 'Hämtar automatiskt ikoner för webbplatser så att logotyper visas i valvlistan.',
                    },
                    {
                      id: 'airgap' as const,
                      title: t('onboarding.mode_airgap_title') || 'Slutet system',
                      icon: Shield,
                      desc: t('onboarding.mode_airgap_desc') || 'Blockerar all nätverksåtkomst och ikonhämtning för total offline-isolering.',
                    },
                  ].map((modeItem) => {
                    const isSelected = operationMode === modeItem.id;
                    const Icon = modeItem.icon;
                    return (
                      <button
                        key={modeItem.id}
                        type="button"
                        onClick={() => handleSelectMode(modeItem.id)}
                        className={`group flex items-start gap-3 rounded-[3px] border p-3 text-left transition-colors cursor-pointer select-none ${
                          isSelected
                            ? 'border-[var(--text-primary)] bg-[var(--bg-active)]'
                            : 'border-[var(--border)] bg-[var(--bg-base)] hover:border-[var(--border-focus)]'
                        }`}
                      >
                        {/* White / Neutral Icon Container */}
                        <div
                          className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-[3px] border transition-colors mt-0.5 ${
                            isSelected
                              ? 'border-[var(--border-focus)] bg-[var(--bg-elevated)] text-[var(--text-primary)]'
                              : 'border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)] group-hover:text-[var(--text-primary)]'
                          }`}
                        >
                          <Icon size={18} />
                        </div>

                        <div className="flex-1 min-w-0">
                          <div className="text-[13px] font-medium text-[var(--text-primary)]">
                            {modeItem.title}
                          </div>
                          <p className="mt-1 text-[11px] text-[var(--text-secondary)] leading-relaxed">
                            {modeItem.desc}
                          </p>
                        </div>
                      </button>
                    );
                  })}
                </div>
              </motion.div>
            )}

            {/* STEP 3: SECURITY DEFAULTS */}
            {step === 3 && (
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
                        { val: 0, label: t('time.never') || 'Never' },
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
                        { val: 0, label: t('time.never') || 'Never' },
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

            {/* STEP 4: COMPETITOR IMPORT */}
            {step === 4 && (
              <motion.div
                key="step-import"
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -6 }}
                transition={{ duration: 0.15 }}
                className="flex flex-col gap-3"
              >
                <div>
                  <h2 className="text-[14px] font-medium text-[var(--text-primary)]">
                    {t('onboarding.step_import') || 'Import'}
                  </h2>
                  <p className="text-[12px] text-[var(--text-secondary)]">
                    {t('onboarding.step_import_desc') || 'Migrate logins from Bitwarden, 1Password, KeePass, or Chrome.'}
                  </p>
                </div>

                <div className="flex flex-col items-center justify-center rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] p-4 text-center gap-2 mt-1">
                  <div className="flex h-9 w-9 items-center justify-center rounded-[3px] bg-[var(--bg-elevated)] text-[var(--text-secondary)] border border-[var(--border)]">
                    <FolderInput size={16} />
                  </div>
                  <p className="text-[12px] font-medium text-[var(--text-primary)]">
                    {t('onboarding.import_title') || 'Import Saved Logins'}
                  </p>
                  <p className="text-[11px] text-[var(--text-secondary)] max-w-[280px] leading-snug">
                    {t('onboarding.import_sub') || 'Migrate logins directly from Bitwarden, 1Password, KeePass, or Chrome in RAM.'}
                  </p>
                  <button
                    type="button"
                    onClick={() => setShowImportModal(true)}
                    className="mt-1 flex h-8 items-center gap-1.5 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                  >
                    <FolderInput size={13} className="text-[var(--text-secondary)]" />
                    <span>{t('onboarding.launch_importer') || 'Launch Importer'}</span>
                  </button>
                </div>
              </motion.div>
            )}

            {/* STEP 5: READY */}
            {step === 5 && (
              <motion.div
                key="step-ready"
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -6 }}
                transition={{ duration: 0.15 }}
                className="flex flex-col items-center justify-center text-center gap-2 py-4"
              >
                <div className="flex h-9 w-9 items-center justify-center rounded-[3px] bg-[var(--bg-elevated)] text-[var(--text-primary)] border border-[var(--border)]">
                  <Check size={16} />
                </div>
                <h2 className="text-[15px] font-semibold text-[var(--text-primary)]">
                  {t('onboarding.ready_title')}
                </h2>
                <p className="text-[12px] text-[var(--text-secondary)] leading-relaxed max-w-[280px]">
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
              className="flex h-9 items-center gap-1 rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-3 text-[12px] font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] cursor-pointer"
            >
              <ArrowLeft size={13} />
              {t('onboarding.back')}
            </button>
          ) : <div />}

          {step < 5 ? (
            <button
              type="button"
              onClick={() => setStep(step + 1)}
              className="flex h-9 items-center gap-1 rounded-[3px] bg-[var(--text-primary)] px-4 text-[12px] font-semibold text-[var(--bg-base)] transition-opacity hover:opacity-90 ml-auto cursor-pointer"
            >
              {step === 4 ? (t('onboarding.skip_or_next') || 'Skip / Next') : t('onboarding.next')}
              <ArrowRight size={13} />
            </button>
          ) : (
            <button
              type="button"
              onClick={handleFinish}
              className="flex h-9 items-center justify-center gap-1 rounded-[3px] bg-[var(--text-primary)] px-5 text-[13px] font-semibold text-[var(--bg-base)] transition-opacity hover:opacity-90 ml-auto cursor-pointer"
            >
              {t('onboarding.get_started')}
              <ArrowRight size={14} />
            </button>
          )}
        </div>
      </div>

      <ImportModal
        isOpen={showImportModal}
        onClose={() => setShowImportModal(false)}
        onSuccess={() => setShowImportModal(false)}
      />
    </motion.div>
  );
}
