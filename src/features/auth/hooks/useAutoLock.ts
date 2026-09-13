import { useEffect, useRef, useCallback } from 'react';

export interface UseAutoLockOptions {
  autoLockMinutes: number;
  isLocked?: boolean;
  onLock: () => void;
}

/**
 * Monitors user activity and automatically triggers a vault lock when inactivity
 * exceeds `autoLockMinutes`.
 *
 * Security & Reliability features:
 * 1. Capture-phase listening on `window` (`capture: true`) to catch user interactions
 *    even when child elements, dialogs, or modal overlays call `stopPropagation()`.
 * 2. Activity events: `mousemove`, `mousedown`, `keydown`, `touchstart`, `scroll`, and `wheel`.
 * 3. High-frequency event throttling: reschedules the timer at most once per second to avoid
 *    churning timers on fast mouse movements (e.g. 1000Hz gaming mice).
 * 4. Timestamp-based inactivity tracking (`Date.now() - lastActivity`) combined with
 *    a low-overhead periodic heartbeat and `visibilitychange` / `focus` listeners.
 *    This ensures that when an OS suspends, sleeps, or hibernates (which freezes JS timers),
 *    the vault locks immediately upon resume/wake if the timeout was reached.
 */
export function useAutoLock({ autoLockMinutes, isLocked = false, onLock }: UseAutoLockOptions) {
  const lastActivityRef = useRef<number>(Date.now());
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const onLockRef = useRef(onLock);
  onLockRef.current = onLock;

  const triggerLock = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    onLockRef.current();
  }, []);

  useEffect(() => {
    // If auto-lock is disabled (<= 0), already locked, or in a non-browser environment, do not schedule.
    if (autoLockMinutes <= 0 || isLocked || typeof window === 'undefined') {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
        timerRef.current = null;
      }
      return;
    }

    const timeoutMs = autoLockMinutes * 60 * 1000;
    lastActivityRef.current = Date.now();

    const scheduleTimeout = () => {
      if (timerRef.current) clearTimeout(timerRef.current);
      const elapsed = Date.now() - lastActivityRef.current;
      const remainingMs = Math.max(0, timeoutMs - elapsed);
      timerRef.current = setTimeout(() => {
        triggerLock();
      }, remainingMs);
    };

    let lastResetTime = 0;
    const handleUserActivity = () => {
      const now = Date.now();
      // Throttle timer rescheduling to at most once per second for performance
      if (now - lastResetTime >= 1000) {
        lastResetTime = now;
        lastActivityRef.current = now;
        scheduleTimeout();
      }
    };

    // Check if inactivity timeout elapsed during system sleep, hibernation, or background suspension
    const checkElapsedInactivity = () => {
      if (Date.now() - lastActivityRef.current >= timeoutMs) {
        triggerLock();
      } else {
        scheduleTimeout();
      }
    };

    scheduleTimeout();

    // Heartbeat check every 10 seconds to catch clock jumps or wake from OS sleep
    const heartbeatInterval = setInterval(checkElapsedInactivity, 10000);

    const activityEvents = [
      'mousemove',
      'mousedown',
      'keydown',
      'touchstart',
      'scroll',
      'wheel',
    ] as const;

    activityEvents.forEach((ev) => {
      window.addEventListener(ev, handleUserActivity, { capture: true, passive: true });
    });

    window.addEventListener('visibilitychange', checkElapsedInactivity);
    window.addEventListener('focus', checkElapsedInactivity);

    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
        timerRef.current = null;
      }
      clearInterval(heartbeatInterval);
      activityEvents.forEach((ev) => {
        window.removeEventListener(ev, handleUserActivity, { capture: true });
      });
      window.removeEventListener('visibilitychange', checkElapsedInactivity);
      window.removeEventListener('focus', checkElapsedInactivity);
    };
  }, [autoLockMinutes, isLocked, triggerLock]);
}
