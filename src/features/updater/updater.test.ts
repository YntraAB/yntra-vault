import { describe, it, expect } from 'bun:test';
import { DEFAULT_SETTINGS } from '../settings/context/SettingsContext';
import type { CheckUpdateResult } from '@/types/ipc';

describe('Updater Feature Slice', () => {
  it('correctly models CheckUpdateResult when newer version is available', () => {
    const result: CheckUpdateResult = {
      current_version: '0.2.2',
      latest_version: '0.2.3',
      has_update: true,
      release_notes: '## Improvements\n- Faster vault unlocking\n- Enhanced security',
      pub_date: '2026-09-22T08:00:00Z',
      download_url: 'https://github.com/YntraAB/yntra-vault/releases/download/v0.2.3/Yntra.Vault_0.2.3_x64-setup.exe',
      sha256: 'a1b2c3d4e5f67890123456789abcdef0123456789abcdef0123456789abcdef0',
      signature: null,
      target_platform: 'windows-x86_64',
    };

    expect(result.has_update).toBe(true);
    expect(result.current_version).toBe('0.2.2');
    expect(result.latest_version).toBe('0.2.3');
    expect(result.sha256).toHaveLength(64);
    expect(result.target_platform).toBe('windows-x86_64');
  });

  it('correctly models CheckUpdateResult when application is up to date', () => {
    const result: CheckUpdateResult = {
      current_version: '0.2.2',
      latest_version: '0.2.2',
      has_update: false,
      release_notes: null,
      pub_date: null,
      download_url: null,
      sha256: null,
      signature: null,
      target_platform: 'windows-x86_64',
    };

    expect(result.has_update).toBe(false);
    expect(result.current_version).toBe(result.latest_version);
    expect(result.download_url).toBeNull();
  });

  it('correctly models Android APK update asset', () => {
    const androidResult: CheckUpdateResult = {
      current_version: '0.2.2',
      latest_version: '0.2.3',
      has_update: true,
      release_notes: 'Mobile release notes',
      pub_date: '2026-09-22T08:00:00Z',
      download_url: 'https://github.com/YntraAB/yntra-vault/releases/download/v0.2.3/Yntra.Vault_0.2.3_universal.apk',
      sha256: 'feedbeef1234567890123456789abcdef0123456789abcdef0123456789abcdef0',
      signature: null,
      target_platform: 'android',
    };

    expect(androidResult.target_platform).toBe('android');
    expect(androidResult.download_url).toEndWith('.apk');
    expect(androidResult.sha256).toBeDefined();
  });

  it('correctly models Windows Portable binary update asset', () => {
    const portableResult: CheckUpdateResult = {
      current_version: '0.2.2',
      latest_version: '0.2.3',
      has_update: true,
      release_notes: 'Portable update notes',
      pub_date: '2026-09-22T08:00:00Z',
      download_url: 'https://github.com/YntraAB/yntra-vault/releases/download/v0.2.3/Yntra.Vault_0.2.3_portable.exe',
      sha256: 'deadbeef1234567890123456789abcdef0123456789abcdef0123456789abcdef0',
      signature: null,
      target_platform: 'windows-portable',
    };

    expect(portableResult.target_platform).toBe('windows-portable');
    expect(portableResult.download_url).toEndWith('_portable.exe');
  });

  it('keeps update and breach checks opt-in while enabling website icons', () => {
    expect(DEFAULT_SETTINGS.autoCheckUpdates).toBe(false);
    expect(DEFAULT_SETTINGS.externalFaviconsEnabled).toBe(true);
    expect(DEFAULT_SETTINGS.autoBreachCheck).toBe(false);
  });
});
