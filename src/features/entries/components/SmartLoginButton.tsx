import { useState, useEffect, useCallback } from 'react';
import { Zap } from 'lucide-react';
import { getBackend, isTauri } from '@/lib/backend';
import { ActionTooltip } from '@/components/ui/tooltip';
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
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [events, setEvents] = useState<SmartLoginEvent[]>([]);
  const [result, setResult] = useState<unknown | null>(null);
  const [phase, setPhase] = useState<'confirm' | 'preparing' | 'running' | 'done'>('confirm');
  const [error, setError] = useState<string | null>(null);
  const [browserName, setBrowserName] = useState('');
  const [browserNeedsClose, setBrowserNeedsClose] = useState(false);
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const [, setBrowserIndex] = useState(0);
  const [dontAskAgain, setDontAskAgain] = useState(false);

  const runPrecheck = useCallback(async () => {
    const backend = await getBackend();
    const precheck = await backend.smartLoginPrecheck();

    if (precheck.error || precheck.browsers.length === 0) {
      throw new Error(precheck.error || 'No Chromium browser found');
    }

    const idx = precheck.recommended_index ?? 0;
    const browser = precheck.browsers[idx];
    setBrowserName(browser.name);
    setBrowserNeedsClose(browser.is_running);
    setBrowserIndex(idx);

    return { browser, idx };
  }, []);

  const executeLogin = useCallback(async () => {
    setPhase('preparing');

    try {
      const backend = await getBackend();

      // Re-check in case state changed
      const precheck = await backend.smartLoginPrecheck();
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

      await backend.smartLoginStart(entryId, idx);
    } catch (err) {
      setError(String(err));
      setPhase('done');
    }
  }, [entryId]);

  const handleOpen = useCallback(async () => {
    setEvents([]);
    setResult(null);
    setError(null);
    setIsModalOpen(true);
    setDontAskAgain(false);

    try {
      const { browser } = await runPrecheck();

      // If browser needs closing and user hasn't opted to skip warning
      const skipWarning = localStorage.getItem(STORAGE_KEY) === 'true';
      if (browser.is_running && !skipWarning) {
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
    try {
      const backend = await getBackend();
      await backend.smartLoginCancel();
    } catch { /* ignore */ }
    setIsModalOpen(false);
  }, []);

  useEffect(() => {
    if (!isModalOpen) return;

    let unlisten: (() => void) | undefined;
    let unlistenResult: (() => void) | undefined;

    const setup = async () => {
      try {
        const backend = await getBackend();
        unlisten = await backend.onSmartLoginProgress((event: SmartLoginEvent) => {
          setEvents(prev => [...prev, event]);
        });
        unlistenResult = await backend.onSmartLoginResult((res: unknown) => {
          setResult(res);
          setPhase('done');
        });
      } catch { /* not in Tauri */ }
    };

    setup();
    return () => {
      unlisten?.();
      unlistenResult?.();
    };
  }, [isModalOpen]);

  if (!isTauri() || !hasUrl) return null;

  return (
    <>
      <ActionTooltip content="Smart Login">
        <button
          onClick={handleOpen}
          className="p-1.5 rounded-md text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] transition-all duration-150 select-none cursor-pointer"
          aria-label="Smart Login"
          id="smart-login-button"
        >
          <Zap size={15} />
        </button>
      </ActionTooltip>

      <SmartLoginModal
        isOpen={isModalOpen}
        onClose={() => setIsModalOpen(false)}
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
