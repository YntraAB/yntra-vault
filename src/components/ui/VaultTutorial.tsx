import { useState, useEffect, useCallback, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Plus, Shield, ArrowRight, ArrowDown,
  Globe, User, Lock, Wand2, Tag, Search,
  Settings, FolderInput, MousePointerClick, CheckCircle2, GripVertical,
} from 'lucide-react';
import { useTranslation } from '@/contexts/LanguageContext';
import { useUi } from '@/contexts/UiContext';
import { useEntries } from '@/features/entries';

const STORAGE_KEY = 'yntra-vault-show-tutorial';
const RESUME_KEY = 'yntra-vault-tutorial-step';

// Visual mock of a form field
function MockField({ icon, label, value, accent }: {
  icon: React.ReactNode;
  label: string;
  value: string;
  accent?: boolean;
}) {
  return (
    <div className="flex items-center gap-2.5 rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] px-3 py-2">
      <span className={accent ? 'text-[var(--text-primary)]' : 'text-[var(--text-tertiary)]'}>{icon}</span>
      <div className="flex flex-col min-w-0">
        <span className="text-[9px] uppercase tracking-wider text-[var(--text-tertiary)] font-medium">{label}</span>
        <span className="text-[12px] text-[var(--text-primary)] truncate">{value}</span>
      </div>
    </div>
  );
}

// Visual mock of a password strength bar
function MockStrengthBar({ label }: { label: string }) {
  return (
    <div className="flex items-center gap-2 mt-1">
      <div className="flex-1 h-1 rounded-full bg-[var(--border)] overflow-hidden">
        <div className="h-full w-[85%] rounded-full bg-[var(--text-primary)] transition-all" />
      </div>
      <span className="text-[9px] font-semibold text-[var(--text-primary)]">{label}</span>
    </div>
  );
}

// Step IDs for tracking
type StepId = 'orientation' | 'create-tag' | 'create-entry' | 'generator' | 'features';
const STEP_ORDER: StepId[] = ['orientation', 'create-tag', 'create-entry', 'generator', 'features'];

export function VaultTutorial() {
  const { t } = useTranslation();
  const { setIsEntryModalOpen, setIsCreateTagOpen } = useUi();
  const { tags, entries } = useEntries();
  const [phase, setPhase] = useState<'hidden' | 'prompt' | 'guide' | 'waiting'>('hidden');
  const [stepIndex, setStepIndex] = useState(0);

  // Track counts to detect when user completes an action
  const prevTagCount = useRef<number>(tags.length);
  const prevEntryCount = useRef<number>(entries.length);
  const waitingForStep = useRef<StepId | null>(null);

  // Initialize from localStorage
  useEffect(() => {
    const resumeStep = localStorage.getItem(RESUME_KEY);
    if (resumeStep !== null) {
      // Resuming after an action
      const idx = parseInt(resumeStep, 10);
      if (!isNaN(idx) && idx < STEP_ORDER.length) {
        setStepIndex(idx);
        waitingForStep.current = STEP_ORDER[idx - 1] || null;
        setPhase('waiting');
      }
    } else if (localStorage.getItem(STORAGE_KEY) === 'true') {
      setPhase('prompt');
    }
  }, []);

  // Watch for tag creation completion
  useEffect(() => {
    if (phase === 'waiting' && waitingForStep.current === 'create-tag') {
      if (tags.length > prevTagCount.current) {
        // Tag was created, resume tutorial
        const resumeIdx = parseInt(localStorage.getItem(RESUME_KEY) || '2', 10);
        localStorage.removeItem(RESUME_KEY);
        waitingForStep.current = null;
        setStepIndex(resumeIdx);
        // Brief delay so user sees the tag created and tag modal closed
        setTimeout(() => setPhase('guide'), 500);
      }
    }
    prevTagCount.current = tags.length;
  }, [tags.length, phase]);

  // Watch for entry creation completion (entry modal closes after adding)
  useEffect(() => {
    if (phase === 'waiting' && waitingForStep.current === 'create-entry') {
      if (entries.length > prevEntryCount.current) {
        // Entry was added, resume tutorial
        const resumeIdx = parseInt(localStorage.getItem(RESUME_KEY) || '3', 10);
        localStorage.removeItem(RESUME_KEY);
        waitingForStep.current = null;
        setStepIndex(resumeIdx);
        setTimeout(() => setPhase('guide'), 500);
      }
    }
    prevEntryCount.current = entries.length;
  }, [entries.length, phase]);

  const dismiss = useCallback(() => {
    localStorage.removeItem(STORAGE_KEY);
    localStorage.removeItem(RESUME_KEY);
    waitingForStep.current = null;
    setPhase('hidden');
  }, []);

  const startGuide = () => {
    setStepIndex(0);
    setPhase('guide');
  };

  const resumeGuideManually = () => {
    waitingForStep.current = null;
    const resumeIdx = parseInt(localStorage.getItem(RESUME_KEY) || String(stepIndex), 10);
    localStorage.removeItem(RESUME_KEY);
    setStepIndex(resumeIdx);
    setPhase('guide');
  };

  // Trigger an action and wait for completion
  const triggerAction = (nextStepIndex: number, waitFor: StepId) => {
    localStorage.setItem(RESUME_KEY, String(nextStepIndex));
    waitingForStep.current = waitFor;
    prevTagCount.current = tags.length;
    prevEntryCount.current = entries.length;
    setPhase('waiting');
  };

  // Build steps with access to current state
  const steps = [
    // Step 0: Orientation
    {
      id: 'orientation' as StepId,
      title: t('tutorial.guide_step1_title'),
      content: (
        <div className="flex flex-col gap-3">
          <p className="text-[13px] leading-relaxed text-[var(--text-secondary)]">
            {t('tutorial.guide_step1_desc')}
          </p>
          <div className="rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] p-3 flex flex-col gap-2">
            {[
              { name: t('tutorial.guide_sidebar'), desc: t('tutorial.guide_sidebar_desc') },
              { name: t('tutorial.guide_list'), desc: t('tutorial.guide_list_desc') },
              { name: t('tutorial.guide_detail'), desc: t('tutorial.guide_detail_desc') },
            ].map((area) => (
              <div key={area.name} className="flex items-center gap-2 text-[11px] text-[var(--text-secondary)]">
                <div className="w-2 h-2 rounded-full bg-[var(--text-tertiary)] shrink-0" />
                <span className="font-medium text-[var(--text-primary)]">{area.name}</span>
                <span>— {area.desc}</span>
              </div>
            ))}
          </div>
        </div>
      ),
    },
    // Step 1: Create a Tag
    {
      id: 'create-tag' as StepId,
      title: t('tutorial.guide_tag_title'),
      content: (
        <div className="flex flex-col gap-3">
          <p className="text-[13px] leading-relaxed text-[var(--text-secondary)]">
            {t('tutorial.guide_tag_desc')}
          </p>

          {/* Visual: Show sidebar tag area */}
          <div className="rounded-md border border-dashed border-[var(--border-focus)] bg-[var(--bg-elevated)] p-3 flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <span className="text-[10px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">{t('sidebar.tags')}</span>
              <div className="flex items-center gap-1.5">
                <div className="flex h-5 w-5 items-center justify-center rounded-md bg-[var(--text-primary)] text-[var(--bg-base)]">
                  <Plus size={11} />
                </div>
                <ArrowDown size={12} className="text-[var(--text-tertiary)] animate-bounce" />
              </div>
            </div>
            <div className="flex items-center gap-2 text-[11px] text-[var(--text-tertiary)] italic">
              {t('tutorial.guide_tag_empty')}
            </div>
          </div>

          <div className="rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] p-2.5 flex flex-col gap-1">
            <span className="text-[11px] font-medium text-[var(--text-primary)]">{t('tutorial.guide_tag_how')}</span>
            <ol className="text-[11px] text-[var(--text-secondary)] list-decimal list-inside space-y-0.5">
              <li>{t('tutorial.guide_tag_step1')}</li>
              <li>{t('tutorial.guide_tag_step2')}</li>
              <li>{t('tutorial.guide_tag_step3')}</li>
            </ol>
          </div>
        </div>
      ),
      action: () => {
        triggerAction(2, 'create-tag');
        setTimeout(() => setIsCreateTagOpen(true), 250);
      },
      actionLabel: t('tutorial.guide_try_it'),
    },
    // Step 2: Add first entry
    {
      id: 'create-entry' as StepId,
      title: t('tutorial.guide_step2_title'),
      content: (
        <div className="flex flex-col gap-3">
          <p className="text-[13px] leading-relaxed text-[var(--text-secondary)]">
            {t('tutorial.guide_step2_desc')}
          </p>

          <div className="flex items-center gap-3 rounded-md border border-dashed border-[var(--border-focus)] bg-[var(--bg-elevated)] p-3">
            <div className="flex h-8 w-8 items-center justify-center rounded-md bg-[var(--text-primary)] text-[var(--bg-base)] shrink-0">
              <Plus size={16} />
            </div>
            <div className="flex flex-col">
              <span className="text-[12px] font-medium text-[var(--text-primary)]">{t('tutorial.guide_click_plus')}</span>
              <span className="text-[10px] text-[var(--text-tertiary)]">{t('tutorial.guide_click_plus_hint')}</span>
            </div>
            <ArrowDown size={14} className="text-[var(--text-tertiary)] shrink-0 ml-auto animate-bounce" />
          </div>

          <div className="flex flex-col gap-1.5">
            <MockField icon={<Globe size={12} />} label={t('entry.title').toUpperCase()} value="Google" accent />
            <MockField icon={<User size={12} />} label={t('entry.username').toUpperCase()} value="your@email.com" />
            <MockField icon={<Lock size={12} />} label={t('entry.password').toUpperCase()} value="••••••••••••••" accent />
            <MockField icon={<Globe size={12} />} label={t('entry.website_url').toUpperCase()} value="https://google.com" />
          </div>
        </div>
      ),
      action: () => {
        triggerAction(3, 'create-entry');
        setTimeout(() => setIsEntryModalOpen(true), 250);
      },
      actionLabel: t('tutorial.guide_try_it'),
    },
    // Step 3: Generator
    {
      id: 'generator' as StepId,
      title: t('tutorial.guide_step3_title'),
      content: (
        <div className="flex flex-col gap-3">
          <p className="text-[13px] leading-relaxed text-[var(--text-secondary)]">
            {t('tutorial.guide_step3_desc')}
          </p>

          <div className="rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] p-3 flex flex-col gap-2.5">
            <div className="flex items-center gap-2">
              <Wand2 size={13} className="text-[var(--text-primary)]" />
              <span className="text-[11px] font-medium text-[var(--text-primary)]">{t('tutorial.guide_generator')}</span>
            </div>
            <div className="rounded-md bg-[var(--bg-surface)] border border-[var(--border)] px-3 py-2 font-mono text-[13px] text-[var(--text-primary)] tracking-wide">
              kX9#mP2$vL7@nQ4
            </div>
            <MockStrengthBar label={t('strength.strong')} />
            <div className="flex gap-2 flex-wrap">
              {['A-Z', 'a-z', '0-9', '#$%'].map((label) => (
                <span key={label} className="rounded border border-[var(--border)] bg-[var(--bg-surface)] px-2 py-0.5 text-[10px] font-medium text-[var(--text-secondary)]">
                  {label}
                </span>
              ))}
              <span className="text-[10px] text-[var(--text-tertiary)] self-center ml-1">{t('tutorial.guide_length')}</span>
            </div>
          </div>

          <p className="text-[11px] text-[var(--text-tertiary)] leading-relaxed">
            {t('tutorial.guide_step3_hint')}
          </p>
        </div>
      ),
    },
    // Step 4: Features
    {
      id: 'features' as StepId,
      title: t('tutorial.guide_step4_title'),
      content: (
        <div className="flex flex-col gap-3">
          <p className="text-[13px] leading-relaxed text-[var(--text-secondary)]">
            {t('tutorial.guide_step4_desc')}
          </p>

          <div className="grid grid-cols-2 gap-2">
            {[
              { icon: <Search size={14} />, label: t('tutorial.tip_search'), hint: 'Ctrl+K' },
              { icon: <Tag size={14} />, label: t('tutorial.tip_tags'), hint: t('tutorial.tip_tags_hint') },
              { icon: <FolderInput size={14} />, label: t('tutorial.tip_import'), hint: t('tutorial.tip_import_hint') },
              { icon: <Settings size={14} />, label: t('tutorial.tip_settings'), hint: t('tutorial.tip_settings_hint') },
            ].map((tip) => (
              <div key={tip.label} className="flex items-start gap-2 rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] p-2.5">
                <span className="text-[var(--text-secondary)] mt-0.5 shrink-0">{tip.icon}</span>
                <div className="flex flex-col min-w-0">
                  <span className="text-[11px] font-medium text-[var(--text-primary)]">{tip.label}</span>
                  <span className="text-[9px] text-[var(--text-tertiary)]">{tip.hint}</span>
                </div>
              </div>
            ))}
          </div>

          <div className="flex items-start gap-2.5 rounded-md border border-[var(--border)] bg-[var(--bg-elevated)] p-3">
            <GripVertical size={16} className="text-[var(--text-primary)] mt-0.5 shrink-0" />
            <div className="flex flex-col min-w-0">
              <span className="text-[12px] font-medium text-[var(--text-primary)]">
                {t('tutorial.tip_drag_tags')}
              </span>
              <span className="text-[11px] text-[var(--text-secondary)] leading-relaxed mt-0.5">
                {t('tutorial.tip_drag_tags_hint')}
              </span>
            </div>
          </div>
        </div>
      ),
    },
  ];

  const totalSteps = steps.length;
  const currentStep = steps[stepIndex];
  const isLast = stepIndex === totalSteps - 1;

  const next = () => {
    if (stepIndex < totalSteps - 1) {
      setStepIndex((s) => s + 1);
    } else {
      dismiss();
    }
  };

  if (phase === 'hidden') return null;

  // Waiting state: show a floating hint with option to open guide or skip
  if (phase === 'waiting') {
    const waitStep = waitingForStep.current;
    const hintText = waitStep === 'create-tag'
      ? t('tutorial.waiting_tag')
      : t('tutorial.waiting_entry');

    return (
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: 20 }}
        className="fixed bottom-6 left-1/2 -translate-x-1/2 z-[100] flex items-center gap-3 rounded-lg border border-[var(--border)] bg-[var(--bg-base)] px-4 py-2.5 shadow-2xl select-none"
      >
        <div className="h-2 w-2 rounded-full bg-[var(--text-primary)] animate-pulse shrink-0" />
        <span className="text-[12px] text-[var(--text-secondary)]">{hintText}</span>
        <button
          onClick={resumeGuideManually}
          className="text-[11px] font-medium text-[var(--text-primary)] hover:underline transition-colors cursor-pointer ml-1"
        >
          {t('tutorial.resume_guide')}
        </button>
        <button
          onClick={dismiss}
          className="text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors cursor-pointer ml-1"
        >
          {t('tutorial.skip')}
        </button>
      </motion.div>
    );
  }

  return (
    <AnimatePresence mode="wait">
      {/* Phase 1: Ask if they want a guide */}
      {phase === 'prompt' && (
        <motion.div
          key="prompt"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 select-none"
        >
          <motion.div
            initial={{ scale: 0.96, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.96, opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="w-full max-w-[380px] mx-3 rounded-lg border border-[var(--border)] bg-[var(--bg-base)] p-6 shadow-2xl text-center"
          >
            <div className="mb-4 mx-auto flex h-12 w-12 items-center justify-center rounded-full border border-[var(--border)] bg-[var(--bg-elevated)] text-[var(--text-primary)]">
              <Shield size={24} />
            </div>
            <h2 className="text-[16px] font-semibold tracking-tight text-[var(--text-primary)]">
              {t('tutorial.prompt_title')}
            </h2>
            <p className="mt-2 text-[13px] leading-relaxed text-[var(--text-secondary)]">
              {t('tutorial.prompt_desc')}
            </p>
            <div className="mt-6 flex flex-col gap-2">
              <button
                onClick={startGuide}
                className="flex h-9 w-full items-center justify-center gap-2 rounded-md bg-[var(--text-primary)] px-4 text-[13px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 cursor-pointer"
              >
                <MousePointerClick size={15} />
                <span>{t('tutorial.prompt_yes')}</span>
              </button>
              <button
                onClick={dismiss}
                className="h-9 w-full rounded-md border border-[var(--border)] px-4 text-[12px] font-medium text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
              >
                {t('tutorial.prompt_no')}
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}

      {/* Phase 2: Interactive step-by-step guide */}
      {phase === 'guide' && (
        <motion.div
          key={`guide-${stepIndex}`}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 select-none"
        >
          <motion.div
            initial={{ scale: 0.96, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.96, opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="w-full max-w-[440px] mx-3 max-h-[calc(100vh-4rem)] rounded-lg border border-[var(--border)] bg-[var(--bg-base)] shadow-2xl flex flex-col overflow-hidden"
          >
            {/* Header with step counter */}
            <div className="flex items-center justify-between px-5 py-3.5 border-b border-[var(--border-subtle)] shrink-0">
              <span className="text-[11px] font-medium text-[var(--text-tertiary)]">
                {t('tutorial.step_counter', { current: String(stepIndex + 1), total: String(totalSteps) })}
              </span>
              <div className="flex items-center gap-1">
                {Array.from({ length: totalSteps }).map((_, i) => (
                  <div
                    key={i}
                    className={`h-1.5 rounded-full transition-all duration-200 ${
                      i === stepIndex
                        ? 'w-5 bg-[var(--text-primary)]'
                        : i < stepIndex
                        ? 'w-1.5 bg-[var(--text-secondary)]'
                        : 'w-1.5 bg-[var(--border)]'
                    }`}
                  />
                ))}
              </div>
              <button
                onClick={dismiss}
                className="text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors cursor-pointer"
              >
                {t('tutorial.skip')}
              </button>
            </div>

            {/* Step content */}
            <div className="flex-1 overflow-y-auto px-5 py-4">
              <AnimatePresence mode="wait">
                <motion.div
                  key={stepIndex}
                  initial={{ opacity: 0, y: 12 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -12 }}
                  transition={{ duration: 0.15 }}
                >
                  <h2 className="text-[16px] font-semibold tracking-tight text-[var(--text-primary)] mb-3">
                    {currentStep.title}
                  </h2>
                  {currentStep.content}
                </motion.div>
              </AnimatePresence>
            </div>

            {/* Footer actions */}
            <div className="flex items-center justify-between px-5 py-3 border-t border-[var(--border-subtle)] shrink-0">
              <button
                onClick={() => stepIndex > 0 && setStepIndex((s) => s - 1)}
                disabled={stepIndex === 0}
                className="h-8 rounded-md px-3 text-[12px] font-medium text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer disabled:opacity-30 disabled:cursor-default disabled:hover:bg-transparent"
              >
                {t('tutorial.back')}
              </button>
              <div className="flex gap-2">
                {currentStep.action && (
                  <button
                    onClick={currentStep.action}
                    className="flex h-8 items-center gap-1.5 rounded-md border border-[var(--border)] px-3 text-[12px] font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-colors cursor-pointer"
                  >
                    <MousePointerClick size={12} />
                    <span>{currentStep.actionLabel}</span>
                  </button>
                )}
                <button
                  onClick={next}
                  className="flex h-8 items-center gap-1.5 rounded-md bg-[var(--text-primary)] px-4 text-[12px] font-semibold text-[var(--bg-base)] transition-all hover:opacity-90 cursor-pointer"
                >
                  {isLast ? (
                    <>
                      <CheckCircle2 size={13} />
                      <span>{t('tutorial.finish')}</span>
                    </>
                  ) : (
                    <>
                      <span>{t('tutorial.next')}</span>
                      <ArrowRight size={13} />
                    </>
                  )}
                </button>
              </div>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

export default VaultTutorial;
