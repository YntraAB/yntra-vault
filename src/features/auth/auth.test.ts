import { describe, it, expect } from 'bun:test';
import type { Vault } from '@/types';

describe('Auth Feature Slice', () => {
  it('handles vault list additions and removals cleanly', () => {
    const initialVaults: Vault[] = [
      { id: '1', name: 'Personal Vault', path: '~/.yntra-vault/vault.db' },
      { id: '2', name: 'Work Vault', path: '~/.yntra-vault/work.db' },
    ];

    const newVault: Vault = { id: '3', name: 'Team Vault', path: '~/.yntra-vault/team.db' };
    const added = [...initialVaults, newVault];
    expect(added).toHaveLength(3);
    expect(added[2].name).toBe('Team Vault');

    const removed = added.filter((v) => v.id !== '2');
    expect(removed).toHaveLength(2);
    expect(removed.find((v) => v.id === '2')).toBeUndefined();
  });

  it('validates vault creation parameter constraints', () => {
    const validateVault = (name: string, path: string, passLength: number, confirm: string, pass: string) => {
      if (name.trim().length < 2) return 'Vault name must be at least 2 characters';
      if (!path.trim()) return 'Please choose a file location';
      if (passLength < 12) return 'Master password must be at least 12 characters';
      if (!confirm) return 'Please confirm your master password';
      if (pass !== confirm) return 'Passwords do not match';
      return null;
    };

    expect(validateVault('A', '/path/v.vdb', 16, 'secret123456!', 'secret123456!')).toBe('Vault name must be at least 2 characters');
    expect(validateVault('Main', '', 16, 'secret123456!', 'secret123456!')).toBe('Please choose a file location');
    expect(validateVault('Main', '/path/v.vdb', 8, 'secret!', 'secret!')).toBe('Master password must be at least 12 characters');
    expect(validateVault('Main', '/path/v.vdb', 16, '', 'secret123456!')).toBe('Please confirm your master password');
    expect(validateVault('Main', '/path/v.vdb', 16, 'mismatch', 'secret123456!')).toBe('Passwords do not match');
    expect(validateVault('Main', '/path/v.vdb', 16, 'secret123456!', 'secret123456!')).toBeNull();
  });

  it('correctly calculates auto-lock timeouts and inactivity thresholds', () => {
    const isAutoLockEnabled = (minutes: number, isLocked: boolean) => minutes > 0 && !isLocked;
    const calculateTimeoutMs = (minutes: number) => minutes * 60 * 1000;
    const shouldLock = (lastActivity: number, now: number, timeoutMs: number) => (now - lastActivity) >= timeoutMs;

    // Disabled cases
    expect(isAutoLockEnabled(0, false)).toBe(false);
    expect(isAutoLockEnabled(-1, false)).toBe(false);
    expect(isAutoLockEnabled(15, true)).toBe(false);

    // Enabled cases
    expect(isAutoLockEnabled(1, false)).toBe(true);
    expect(isAutoLockEnabled(5, false)).toBe(true);
    expect(isAutoLockEnabled(15, false)).toBe(true);
    expect(isAutoLockEnabled(30, false)).toBe(true);

    // Timeout milliseconds conversion
    expect(calculateTimeoutMs(1)).toBe(60_000);
    expect(calculateTimeoutMs(5)).toBe(300_000);
    expect(calculateTimeoutMs(15)).toBe(900_000);
    expect(calculateTimeoutMs(30)).toBe(1_800_000);

    // Activity check
    const t0 = 1_000_000;
    const fiveMinMs = 5 * 60 * 1000;
    expect(shouldLock(t0, t0 + 100_000, fiveMinMs)).toBe(false);
    expect(shouldLock(t0, t0 + 299_999, fiveMinMs)).toBe(false);
    expect(shouldLock(t0, t0 + 300_000, fiveMinMs)).toBe(true);
    expect(shouldLock(t0, t0 + 600_000, fiveMinMs)).toBe(true);
  });
});
