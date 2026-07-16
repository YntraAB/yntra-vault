/**
 * BreachIndicator — Shows if a password has been found in data breaches
 * 
 * States: unknown → checking → safe/breached/error
 * Uses HIBP k-anonymity API via Rust backend.
 * Automatically runs checks on password change with debouncing.
 */

import React, { useEffect, useState, useCallback, useRef } from 'react';
import { useBackend } from '../lib/useBackend';
import { useAppState } from '../contexts/AppStateContext';
import type { BreachStatus } from '../lib/backend';

interface BreachIndicatorProps {
  /** Pre-fetched breach status from entry data */
  status?: BreachStatus;
  /** Password to check (triggers API call) */
  password?: string;
  /** Show as compact text or full detail */
  compact?: boolean;
  /** Callback when status changes */
  onStatusChange?: (status: BreachStatus) => void;
  /** Hide indicator if password is safe or unknown */
  hideIfSafe?: boolean;
}

export const BreachIndicator: React.FC<BreachIndicatorProps> = ({
  status: initialStatus,
  password,
  compact = false,
  onStatusChange,
  hideIfSafe = false,
}) => {
  const { backend } = useBackend();
  const { settings } = useAppState();
  const [status, setStatus] = useState<BreachStatus>(
    initialStatus || { type: 'Unknown' }
  );
  const lastCheckedPasswordRef = useRef<string | null>(null);

  // Update status if initialStatus prop changes
  useEffect(() => {
    if (initialStatus) {
      setStatus(initialStatus);
      if (initialStatus.type !== 'Unknown') {
        lastCheckedPasswordRef.current = password || null;
      }
    }
  }, [initialStatus, password]);

  // Check breach logic
  const checkBreach = useCallback(async (targetPassword: string) => {
    if (!backend || !targetPassword || targetPassword.length < 4) {
      setStatus({ type: 'Unknown' });
      return;
    }

    setStatus({ type: 'Checking' });

    try {
      const result = await backend.checkPasswordBreach(targetPassword);
      const newStatus: BreachStatus = result.is_breached
        ? { type: 'Breached', breach_count: result.breach_count, checked_at: result.checked_at }
        : { type: 'Safe', checked_at: result.checked_at };

      setStatus(newStatus);
      onStatusChange?.(newStatus);
      lastCheckedPasswordRef.current = targetPassword;
    } catch (e: any) {
      setStatus({ type: 'Error', message: e.toString() });
    }
  }, [backend, onStatusChange]);

  // Auto-trigger check with a 500ms debounce when password changes
  useEffect(() => {
    if (!settings.autoBreachCheck) {
      return;
    }

    if (!password || password.trim() === '') {
      setStatus({ type: 'Unknown' });
      return;
    }

    if (password === lastCheckedPasswordRef.current) {
      return;
    }

    const timer = setTimeout(() => {
      checkBreach(password);
    }, 500);

    return () => clearTimeout(timer);
  }, [password, checkBreach, settings.autoBreachCheck]);

  const config = getStatusConfig(status);

  if (hideIfSafe && (status.type === 'Safe' || status.type === 'Unknown')) {
    return null;
  }

  if (status.type === 'Checking' && !compact) {
    return null;
  }

  if (compact) {
    return (
      <span className={`text-[10px] font-medium tracking-wide ${config.textColor}`} title={config.tooltip}>
        {config.shortLabel}
      </span>
    );
  }

  return (
    <div className="flex items-center gap-1.5 text-[11px] font-medium py-0.5 select-none">
      <span className={config.textColor}>{config.label}</span>
      {config.detail && (
        <span className="text-[10px] text-[var(--text-tertiary)] font-normal">({config.detail})</span>
      )}
    </div>
  );
};

// ─── Config Helper ──────────────────────────────────────────────────────

interface StatusConfig {
  shortLabel: string;
  label: string;
  detail?: string;
  tooltip: string;
  textColor: string;
}

function getStatusConfig(status: BreachStatus): StatusConfig {
  switch (status.type) {
    case 'Unknown':
      return {
        shortLabel: 'Not checked',
        label: 'Password breach status not checked yet',
        tooltip: 'This password has not been checked against the HIBP database',
        textColor: 'text-[var(--text-tertiary)]',
      };

    case 'Checking':
      return {
        shortLabel: 'Checking...',
        label: 'Checking password safety...',
        tooltip: 'Comparing password hash against data breach records',
        textColor: 'text-[var(--text-secondary)] animate-pulse',
      };

    case 'Safe':
      return {
        shortLabel: 'Safe',
        label: 'No data breaches found',
        tooltip: 'This password was not found in any known public data leaks',
        textColor: 'text-green-500',
      };

    case 'Breached':
      const formatted = formatCount(status.breach_count);
      return {
        shortLabel: `${formatted} breaches`,
        label: `Found in ${status.breach_count.toLocaleString()} data breaches!`,
        tooltip: `⚠ Warning: This password was leaked in ${status.breach_count.toLocaleString()} public breaches! Change it immediately.`,
        textColor: 'text-red-500 font-semibold',
      };

    case 'Error':
      return {
        shortLabel: 'Check failed',
        label: 'Failed to verify breach status',
        detail: 'offline',
        tooltip: `Error details: ${status.message}`,
        textColor: 'text-amber-500',
      };
  }
}

function formatCount(count: number): string {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M`;
  if (count >= 1_000) return `${(count / 1_000).toFixed(1)}K`;
  return count.toString();
}

export default BreachIndicator;



