import { useState, useEffect, useCallback, useRef } from 'react';
import { Zap } from 'lucide-react';
import { getBackend, isTauri } from '@/lib/backend';
import { ActionTooltip } from '@/components/ui/tooltip';
import { useTranslation } from '@/contexts/LanguageContext';
import SmartLoginModal from './SmartLoginModal';

const STORAGE_KEY = 'yntra.smartlogin.skipCloseWarning';

interface SmartLoginEvent {
  timestamp: string;
  state: unknown;
  message: string;
  detail?: unknown;
}

interface SmartLoginButtonProps {
  entryId: string;
  entryTitle: string;
  hasUrl: boolean;
}

export default function SmartLoginButton({ entryId, entryTitle, hasUrl }: SmartLoginButtonProps) {
  const { t } = useTranslation();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [events, setEvents] = useState<SmartLoginEvent[]>([]);
  const [result, setResult] = useState<unknown | null>(null);
  const [phase, setPhase] = useState<'confirm' | 'preparing' | 'running' | 'done'>('confirm');
  const [error, setError] = useState<string | null>(null);
  const [browserName, setBrowserName] = useState('');
  const [browserNeedsClose, setBrowserNeedsClose] = useState(false);
  const [dontAskAgain, setDontAskAgain] = useState(false);
  const attempt = useRef(0);
  const running = useRef(false);
  const openRequested = useRef(false);
  const subscriptions = useRef<Array<() => void>>([]);
  const clearSubscriptions = useCallback(() => {
    subscriptions.current.forEach(stop => stop());
    subscriptions.current = [];
  }, []);

  const runPrecheck = useCallback(async () => {
    const backend = await getBackend();
    const precheck = await backend.smartLoginPrecheck(entryId);

    if (precheck.error || precheck.browsers.length === 0) {
      throw new Error(precheck.error || 'No Chromium browser found');
    }

    const idx = precheck.recommended_index ?? 0;
    const browser = precheck.browsers[idx];
    setBrowserName(browser.name);
    setBrowserNeedsClose(precheck.needs_close);

    return { browser, idx, needsClose: precheck.needs_close };
  }, [entryId]);

  const executeLogin = useCallback(async () => {
    if (!openRequested.current || running.current) return;
    const currentAttempt = ++attempt.current;
    running.current = true;
    clearSubscriptions();
    setPhase('preparing');

    try {
      const backend = await getBackend();

      // Install listeners before starting: fast existing-session results must not be lost.
      const stopProgress = await backend.onSmartLoginProgress((event: SmartLoginEvent) => {
        if (attempt.current === currentAttempt) setEvents(prev => [...prev.slice(-199), event]);
      });
      if (attempt.current !== currentAttempt) { stopProgress(); return; }
      subscriptions.current.push(stopProgress);
      const stopResult = await backend.onSmartLoginResult((res: unknown) => {
        if (attempt.current !== currentAttempt) return;
        running.current = false;
        setResult(res);
        setPhase('done');
        clearSubscriptions();
      });
      if (attempt.current !== currentAttempt) { stopResult(); return; }
      subscriptions.current.push(stopResult);
      // Re-check in case state changed
      const precheck = await backend.smartLoginPrecheck(entryId);
      if (precheck.error || precheck.browsers.length === 0) {
        throw new Error(precheck.error || 'No Chromium browser found');
      }

      const idx = precheck.recommended_index ?? 0;
      const browser = precheck.browsers[idx];
      setBrowserName(browser.name);

      // Start login — Rust core will reuse active CDP session or relaunch cleanly if needed
      setPhase('running');
      setEvents(prev => [...prev, {
        timestamp: new Date().toISOString(),
        state: 'LaunchingBrowser',
        message: `Connecting to ${browser.name}...`,
      }]);

      if (attempt.current !== currentAttempt) return;
      await backend.smartLoginStart(entryId, idx);
    } catch (err) {
      if (attempt.current !== currentAttempt) return;
      running.current = false;
      clearSubscriptions();
      setError(String(err));
      setPhase('done');
    }
  }, [entryId, clearSubscriptions]);

  const handleOpen = useCallback(async () => {
    openRequested.current = true;
    if (running.current) { setIsModalOpen(true); return; }
    setEvents([]);
    setResult(null);
    setError(null);
    setIsModalOpen(true);
    setDontAskAgain(false);

    try {
      const { needsClose } = await runPrecheck();

      // If browser needs closing and user hasn't opted to skip warning
      const skipWarning = localStorage.getItem(STORAGE_KEY) === 'true';
      if (needsClose && !skipWarning) {
        setPhase('confirm');
        return;
      }

      // Either not running or user chose to skip warning
      await executeLogin();
    } catch (err) {
      setError(String(err));
      setPhase('done');
    }
  }, [executeLogin, runPrecheck]);

  const handleConfirmClose = useCallback(async () => {
    if (dontAskAgain) {
      localStorage.setItem(STORAGE_KEY, 'true');
    }
    await executeLogin();
  }, [dontAskAgain, executeLogin]);

  const handleCancel = useCallback(async () => {
    openRequested.current = false;
    attempt.current += 1;
    clearSubscriptions();
    const wasRunning = running.current;
    running.current = false;
    setIsModalOpen(false);
    if (wasRunning) {
      try { await (await getBackend()).smartLoginCancel(); } catch { /* best effort */ }
    }
  }, [clearSubscriptions]);

  useEffect(() => () => {
    attempt.current += 1;
    openRequested.current = false;
    clearSubscriptions();
    if (running.current) {
      running.current = false;
      void getBackend().then(backend => backend.smartLoginCancel()).catch(() => {});
    }
  }, [clearSubscriptions]);
  if (!isTauri()) return null;

  if (!hasUrl) {
    return (
      <ActionTooltip content={t('smart_login.disabled_tooltip')}>
        <button
          type="button"
          disabled
          className="p-1.5 rounded-md text-[var(--text-tertiary)] opacity-40 cursor-not-allowed select-none"
          aria-label={t('detail.smart_login')}
          id="smart-login-button"
        >
          <Zap size={15} />
        </button>
      </ActionTooltip>
    );
  }

  return (
    <>
      <ActionTooltip content={t('detail.smart_login')}>
        <button
          type="button"
          onClick={handleOpen}
          className="p-1.5 rounded-md text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-all duration-150 select-none cursor-pointer"
          aria-label={t('detail.smart_login')}
          id="smart-login-button"
        >
          <Zap size={15} />
        </button>
      </ActionTooltip>

      <SmartLoginModal
        isOpen={isModalOpen}
        onClose={handleCancel}
        entryTitle={entryTitle}
        phase={phase}
        events={events}
        result={result}
        error={error}
        browserName={browserName}
        browserNeedsClose={browserNeedsClose}
        dontAskAgain={dontAskAgain}
        onDontAskAgainChange={setDontAskAgain}
        onConfirmClose={handleConfirmClose}
        onCancel={handleCancel}
      />
    </>
  );
}
