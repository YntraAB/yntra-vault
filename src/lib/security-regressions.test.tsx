import { beforeAll, afterEach, afterAll, describe, it, expect } from 'bun:test';
import React, { act, createRef, useEffect } from 'react';
import { Window } from 'happy-dom';
import type { Root } from 'react-dom/client';
import { usePasswordGenerator } from '@/features/generator/hooks/usePasswordGenerator';
import { SecureSecretInput, type SecureSecretInputRef } from '@/components/ui/SecureSecretInput';
import { getBackend, resetBackend } from '@/lib/backend';
import { AuthProvider, useAuth } from '@/features/auth/context/AuthContext';
import { EntriesProvider, useEntries } from '@/features/entries/context/EntriesContext';
import { SettingsProvider, useSettings } from '@/features/settings/context/SettingsContext';
import { ToastProvider } from '@/contexts/ToastContext';
import { UpdateModal } from '@/features/updater/UpdateModal';
import { loadTranslation } from '@/i18n/translations';
import type { CheckUpdateResult } from '@/types/ipc';

const dom = new Window();
const previous = new Map<string, PropertyDescriptor | undefined>();
let root: Root | undefined;
let mount: typeof import('react-dom/client').createRoot;

beforeAll(async () => {
  for (const [key, value] of Object.entries({ window: dom, document: dom.document, navigator: dom.navigator, localStorage: dom.localStorage, CustomEvent: dom.CustomEvent, IS_REACT_ACT_ENVIRONMENT: true })) {
    previous.set(key, Object.getOwnPropertyDescriptor(globalThis, key));
    Object.defineProperty(globalThis, key, { configurable: true, writable: true, value });
  }
  mount = (await import('react-dom/client')).createRoot;
});
afterEach(async () => {
  await act(async () => root?.unmount());
  root = undefined;
  resetBackend();
  dom.document.body.innerHTML = '';
  dom.localStorage.clear();
});
afterAll(() => {
  for (const [key, descriptor] of previous) {
    if (descriptor) Object.defineProperty(globalThis, key, descriptor);
    else Reflect.deleteProperty(globalThis, key);
  }
  dom.happyDOM.abort();
});

describe('secret and synchronization lifecycle', () => {
  it('subscribes before a fast Smart Login result and removes both listeners afterwards', async () => {
    const { default: SmartLoginButton } = await import('@/features/entries/components/SmartLoginButton');
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: async () => null } });
    const backend = await getBackend();
    let resultCallback: ((value: unknown) => void) | undefined;
    let progressReady = false;
    let removed = 0;
    Object.assign(backend, {
      smartLoginPrecheck: async () => ({ browsers: [{ name: 'Test browser', is_running: true }], recommended_index: 0, needs_close: false, error: null }),
      onSmartLoginProgress: async () => { progressReady = true; return () => { removed++; }; },
      onSmartLoginResult: async (callback: (value: unknown) => void) => { resultCallback = callback; return () => { removed++; }; },
      smartLoginStart: async () => {
        expect(progressReady).toBe(true);
        expect(resultCallback).toBeDefined();
        resultCallback?.({ AlreadySignedIn: { final_url: 'https://example.test/' } });
      },
    });
    const container = dom.document.createElement('div');
    dom.document.body.append(container);
    await act(async () => {
      root = mount(container as unknown as Element);
      root.render(<SmartLoginButton entryId="test" entryTitle="Test" hasUrl />);
    });
    await act(async () => { container.querySelector<HTMLButtonElement>('#smart-login-button')!.click(); });
    expect(container.textContent).toContain('This account is already signed in.');
    expect(removed).toBe(2);
  });

  it('shares favicon requests and recovers from a failed download without remounting', async () => {
    const { Favicon, clearFaviconCache, resetFaviconCooldowns } = await import('@/features/entries/components/Favicon');
    clearFaviconCache();
    let requests = 0;
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: async (command: string) => {
      if (command === 'get_favicon') return ++requests === 1 ? null : 'data:image/png;base64,test';
      return null;
    } } });
    await getBackend();
    const container = dom.document.createElement('div');
    dom.document.body.append(container);
    await act(async () => {
      root = mount(container as unknown as Element);
      root.render(<SettingsProvider><Favicon title="One" url="https://example.com" /><Favicon title="Two" url="https://example.com" /></SettingsProvider>);
    });
    expect(requests).toBe(1);
    expect(container.querySelectorAll('img').length).toBe(0);
    await act(async () => { resetFaviconCooldowns(); });
    expect(requests).toBe(2);
    expect(container.querySelectorAll('img').length).toBe(2);
    await act(async () => { root?.unmount(); root = undefined; });
    clearFaviconCache();
  });

  it('waits for the native favicon setting and discards downloads after disabling', async () => {
    const { Favicon, clearFaviconCache } = await import('@/features/entries/components/Favicon');
    clearFaviconCache();
    let enable!: () => void;
    let finishIcon!: (value: string | null) => void;
    let requests = 0;
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: async (command: string, args?: { enabled?: boolean }) => {
      if (command === 'set_external_favicons_enabled' && args?.enabled) return new Promise<void>(resolve => { enable = resolve; });
      if (command === 'get_favicon') { requests++; return new Promise<string | null>(resolve => { finishIcon = resolve; }); }
      return null;
    } } });
    await getBackend();
    let settings!: ReturnType<typeof useSettings>;
    function Harness() {
      const current = useSettings();
      useEffect(() => { settings = current; }, [current]);
      return <Favicon title="Example" url="https://example.com" />;
    }
    const container = dom.document.createElement('div');
    dom.document.body.append(container);
    await act(async () => { root = mount(container as unknown as Element); root.render(<SettingsProvider><Harness /></SettingsProvider>); });
    expect(settings.settings.externalFaviconsEnabled).toBe(true);
    expect(requests).toBe(0);
    await act(async () => { enable(); });
    expect(requests).toBe(1);
    await act(async () => { settings.updateSettings({ externalFaviconsEnabled: false }); });
    await act(async () => { finishIcon('data:image/png;base64,test'); });
    expect(container.querySelector('img')).toBeNull();
    expect(dom.localStorage.getItem('yntra-favicons-cache')).toBeNull();
    await act(async () => { root?.unmount(); root = undefined; });
    await act(async () => { root = mount(container as unknown as Element); root.render(<SettingsProvider><Harness /></SettingsProvider>); });
    expect(settings.settings.externalFaviconsEnabled).toBe(false);
    expect(requests).toBe(1);
    clearFaviconCache();
  });

  it('describes the update path honestly and disables unavailable downloads', async () => {
    await loadTranslation('en');
    const update: CheckUpdateResult = {
      current_version: '0.2.2', latest_version: '0.2.3', has_update: true,
      target_platform: 'windows-x86_64', download_url: 'https://example.com/update.exe',
      sha256: null, signature: null, release_notes: null, pub_date: null,
    };
    const container = dom.document.createElement('div');
    dom.document.body.append(container);
    const render = async (info: CheckUpdateResult) => act(async () => {
      root ??= mount(container as unknown as Element);
      root.render(<UpdateModal isOpen onClose={() => {}} updateInfo={info} currentVersion="0.2.2" isDownloading={false} onInstall={() => {}} />);
    });
    await render(update);
    expect(container.textContent).toContain('download opens in your browser');
    expect(container.textContent).not.toContain('Ed25519');
    await render({ ...update, target_platform: 'windows-portable' });
    expect(container.textContent).toContain('checksum is checked before installation');
    await render({ ...update, download_url: null });
    expect(container.textContent).toContain('No download is available');
    expect([...container.querySelectorAll('button')].find(button => button.textContent === 'Download')?.disabled).toBe(true);
  });

  it('keeps the newest result and performs no automatic network breach check', async () => {
    const pending: Array<(value: string) => void> = [];
    const commands: string[] = [];
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: (command: string) => {
      commands.push(command);
      if (command === 'generate_password_default') return new Promise<string>(resolve => pending.push(resolve));
      if (command === 'analyze_password_strength') return Promise.resolve({ score: 4 });
      throw new Error(`Unexpected command: ${command}`);
    } } });
    await getBackend();
    let generator!: ReturnType<typeof usePasswordGenerator>;
    function Harness() {
      const current = usePasswordGenerator();
      useEffect(() => { generator = current; }, [current]);
      return <span>{current.password}</span>;
    }
    const container = dom.document.createElement('div');
    dom.document.body.append(container);
    await act(async () => { root = mount(container as unknown as Element); root.render(<Harness />); });
    let first!: Promise<string>;
    let second!: Promise<string>;
    await act(async () => { first = generator.generate(); second = generator.generate(); });
    await act(async () => { pending[1]('new-result'); await second; });
    await act(async () => { pending[0]('old-result'); await first; });
    expect(container.textContent).toBe('new-result');
    expect(commands.filter(c => c.includes('breach'))).toEqual([]);
  });

  it('submits a generated replacement instead of the previously typed secret', async () => {
    const reference = createRef<SecureSecretInputRef>();
    const container = dom.document.createElement('div');
    dom.document.body.append(container);
    await act(async () => {
      root = mount(container as unknown as Element);
      root.render(<SecureSecretInput ref={reference} value="old-password" onChange={() => {}} />);
    });
    await act(async () => { root!.render(<SecureSecretInput ref={reference} value="generated-password" onChange={() => {}} />); });
    expect(new TextDecoder().decode(reference.current!.getSecretBytes())).toBe('generated-password');
    reference.current!.clearSecretBytes();
    expect(reference.current!.getSecretBytes().length).toBe(0);
  });

  it('refreshes an existing selected secret even with the same timestamp and rejects a refresh after lock', async () => {
    const entry = {
      id: 'entry-1', title: 'Account', username: 'user', password: 'old', url: '', email: '', notes: '',
      tags: [], favorite: false, pinned: false, custom_fields: [], created_at: '2026-09-01T00:00:00Z',
      updated_at: '2026-09-01T00:00:00Z', breach_status: { type: 'Unknown' }, attachments: [],
    };
    let pauseList = false;
    let finishList!: (value: unknown) => void;
    Object.assign(dom, { __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => {} }, __TAURI_INTERNALS__: {
      transformCallback: () => 1,
      unregisterCallback: () => {},
      invoke: async (command: string) => {
        if (command === 'list_entries') return pauseList ? new Promise(resolve => { finishList = resolve; }) : [{ ...entry }];
        if (command === 'get_entry') return { ...entry };
        if (command === 'get_tags') return [];
        return null;
      },
    } });
    await getBackend();
    dom.localStorage.setItem('yntra-vault-settings', JSON.stringify({ disableSkeletonDelays: true }));
    let auth!: ReturnType<typeof useAuth>;
    let entries!: ReturnType<typeof useEntries>;
    function Harness() {
      const currentAuth = useAuth();
      const currentEntries = useEntries();
      useEffect(() => { auth = currentAuth; entries = currentEntries; }, [currentAuth, currentEntries]);
      return null;
    }
    const container = dom.document.createElement('div');
    dom.document.body.append(container);
    await act(async () => {
      root = mount(container as unknown as Element);
      root.render(<ToastProvider><SettingsProvider><AuthProvider><EntriesProvider><Harness /></EntriesProvider></AuthProvider></SettingsProvider></ToastProvider>);
    });
    await act(async () => { auth.setCurrentVault({ id: 'vault-1', name: 'Test', path: '/test.vdb' }); });
    await act(async () => { await entries.selectEntryById(entry.id); });
    expect(entries.selectedEntry?.password).toBe('old');
    entry.password = 'updated';
    await act(async () => { await entries.refreshEntries(); });
    expect(entries.selectedEntry?.password).toBe('updated');
    pauseList = true;
    let pending!: Promise<void>;
    await act(async () => { pending = entries.refreshEntries(); });
    await act(async () => { auth.setIsLocked(true); });
    await act(async () => { finishList([{ ...entry }]); await pending; });
    expect(entries.entries).toEqual([]);
    expect(entries.selectedEntry).toBeNull();
  });
});
