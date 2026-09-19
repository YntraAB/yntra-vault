import { describe, it, expect } from 'bun:test';
import { DEFAULT_SETTINGS } from './context/SettingsContext';

describe('Settings Slice', () => {
  it('provides complete default settings', () => {
    expect(DEFAULT_SETTINGS.theme).toBe('system');
    expect(DEFAULT_SETTINGS.language).toBe('en');
    expect(DEFAULT_SETTINGS.autoLockMinutes).toBe(15);
    expect(DEFAULT_SETTINGS.clipboardClearSeconds).toBe(30);
    expect(DEFAULT_SETTINGS.windowCaptureProtection).toBe(true);
    expect(DEFAULT_SETTINGS.lockOnSystemLock).toBe(true);
    expect(DEFAULT_SETTINGS.groupByDate).toBe(true);
  });

  it('includes keybind defaults', () => {
    expect(DEFAULT_SETTINGS.keybinds).toBeDefined();
    expect(DEFAULT_SETTINGS.keybinds.search).toBeDefined();
    expect(DEFAULT_SETTINGS.keybinds.newEntry).toBeDefined();
    expect(DEFAULT_SETTINGS.keybinds.lockVault).toBeDefined();
  });

  it('correctly merges partial settings over defaults', () => {
    const custom = {
      ...DEFAULT_SETTINGS,
      theme: 'dark' as const,
      autoLockMinutes: 5,
    };
    expect(custom.theme).toBe('dark');
    expect(custom.autoLockMinutes).toBe(5);
    expect(custom.language).toBe('en');
  });
});
