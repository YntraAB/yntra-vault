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
  it('hides desktop automation on Android even on a wide screen and reports failed copies honestly', async () => {
    const { AutotypeButton } = await import('@/features/entries/components/AutotypeButton');
    const { default: SmartLoginButton } = await import('@/features/entries/components/SmartLoginButton');
    const { CopyButton } = await import('@/components/ui/CopyButton');
    const { initializePlatform } = await import('@/lib/platform');
    let os = 'android';
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: async (command: string) => {
      if (command === 'get_runtime_platform') return os;
      if (command === 'copy_to_clipboard') throw new Error('synthetic clipboard failure');
      return null;
    } } });
    await getBackend();
    const container = dom.document.createElement('div'); dom.document.body.append(container);
    await act(async () => {
      root = mount(container as unknown as Element);
      root.render(<ToastProvider><SettingsProvider><AutotypeButton value="test" /><SmartLoginButton entryId="test" entryTitle="Test" hasUrl /><CopyButton value="test" /></SettingsProvider></ToastProvider>);
    });
    expect(container.querySelector('#smart-login-button')).toBeNull();
    expect(container.querySelector('.lucide-keyboard')).toBeNull();
    await act(async () => { container.querySelector<HTMLButtonElement>('button')!.click(); });
    expect(container.querySelector('.lucide-check')).toBeNull();
    expect(container.querySelector('button')?.getAttribute('aria-label')).toContain('failed');
    os = 'windows';
    await act(async () => { await initializePlatform(); });
    expect(container.querySelector('#smart-login-button')).not.toBeNull();
    expect(container.querySelector('.lucide-keyboard')).not.toBeNull();
  });

  it('waits for listener readiness and closes an old listener before restarting', async () => {
    Object.assign(dom, { __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => {} }, __TAURI_INTERNALS__: {
      transformCallback: () => 1, unregisterCallback: () => {},
      invoke: async (command: string) => ['list_entries','get_tags'].includes(command) ? [] : null,
    } });
    const backend = await getBackend();
    let ready = () => {};
    let stop: (() => void) | undefined;
    let starts = 0, active = 0, maximumActive = 0, cancellations = 0;
    Object.assign(backend, {
      onP2pListenerReady: async (callback: () => void) => { ready = callback; return () => { ready = () => {}; }; },
      runP2pSyncListener: async () => {
        starts++; active++; maximumActive = Math.max(maximumActive,active);
        return new Promise((_,reject) => { stop = () => { active--; stop=undefined; reject(new Error('cancelled')); }; });
      },
      cancelP2pSyncListener: async () => { cancellations++; stop?.(); },
    });
    dom.localStorage.setItem('yntra-vault-settings',JSON.stringify({p2pAutoListen:false,p2pAutoSyncWifi:false,disableSkeletonDelays:true}));
    let auth!: ReturnType<typeof useAuth>;
    let entries!: ReturnType<typeof useEntries>;
    function Harness() {
      const a = useAuth(), e = useEntries();
      useEffect(() => { auth=a; entries=e; },[a,e]);
      return null;
    }
    const container=dom.document.createElement('div'); dom.document.body.append(container);
    await act(async()=>{root=mount(container as unknown as Element);root.render(<ToastProvider><SettingsProvider><AuthProvider><EntriesProvider><Harness/></EntriesProvider></AuthProvider></SettingsProvider></ToastProvider>);});
    await act(async()=>{auth.setCurrentVault({id:'test',name:'Test',path:'/test.vdb'});entries.toggleP2pListener(true);});
    expect(starts).toBe(1);
    expect(entries.isP2pListening).toBe(false);
    await act(async()=>ready());
    expect(entries.isP2pListening).toBe(true);
    await act(async()=>auth.setCurrentVault({id:'test',name:'Renamed',path:'/test.vdb'}));
    expect(starts).toBe(1);
    await act(async()=>entries.toggleP2pListener(false));
    expect(cancellations).toBe(1);
    expect(active).toBe(0);
    await act(async()=>entries.toggleP2pListener(true));
    expect(starts).toBe(2);
    expect(maximumActive).toBe(1);
    await act(async()=>auth.setIsLocked(true));
    expect(active).toBe(0);
    expect(entries.isP2pListening).toBe(false);
  });

  it('requires two distinct saved recovery shares and exports only the selected share', async () => {
    const { RecoveryKitCards } = await import('@/features/auth/components/LocalProtection');
    const exports: Array<{ path: string; share: string }> = [];
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: async (command: string) => command === 'plugin:dialog|save' ? 'C:/recovery/share-2.txt' : null } });
    const backend = await getBackend();
    Object.assign(backend, { saveFileDialog: async () => 'C:/recovery/share-2.txt', exportRecoveryShare: async (path: string, share: string) => { exports.push({ path, share }); } });
    const kit = {
      vault_id:'vault-id',vault_name:'Test',created_at:'2026-09-26',generated_at:'2026-09-26',format_version:2,total_entries:1,
      verification_hash:'kit-id',document_markdown:'Store separately.',
      shares:[1,2,3].map(i=>({share_index:i,label:`Share ${i}`,share_data:`test-share-${i}`})),
    };
    let completed = false;
    const container=dom.document.createElement('div');dom.document.body.append(container);
    await act(async()=>{root=mount(container as unknown as Element);root.render(<RecoveryKitCards kit={kit} onDone={()=>{completed=true;}}/>);});
    const button=(label:string)=>Array.from(container.querySelectorAll<HTMLButtonElement>('button')).find(b=>b.textContent?.trim()===label)!;
    await act(async()=>button('Start saving').click());
    expect(button('Continue').disabled).toBe(true);
    expect(container.querySelector('code')).toBeNull();
    await act(async()=>container.querySelector<HTMLInputElement>('input[type=checkbox]')!.click());
    expect(button('Continue').disabled).toBe(true);
    await act(async()=>button('Next share').click());
    await act(async()=>button('Save this share').click());
    expect(exports).toEqual([{path:'C:/recovery/share-2.txt',share:'test-share-2'}]);
    expect(button('Continue').disabled).toBe(true);
    await act(async()=>button('Show for transcription').click());
    expect(container.querySelector('code')?.textContent).toBe('test-share-2');
    await act(async()=>container.querySelector<HTMLInputElement>('input[type=checkbox]')!.click());
    expect(button('Continue').disabled).toBe(false);
    await act(async()=>button('Continue').click());
    expect(container.querySelector('code')).toBeNull();
    expect(completed).toBe(false);
    await act(async()=>button('Done').click());expect(completed).toBe(true);
  });

  it('resizes panels within viewport bounds and persists on release, including settings reload', async()=>{
    const {usePanelResize}=await import('@/hooks/usePanelResize');
    const saves:Array<{sidebarWidth:number;passwordListWidth:number}>=[];
    const save=(sizes:{sidebarWidth:number;passwordListWidth:number})=>{saves.push(sizes);};
    function Panels({list=320}:{list?:number}){const {handleListResizeStart}=usePanelResize(220,list,save);return <button onMouseDown={handleListResizeStart}>resize</button>;}
    const container=dom.document.createElement('div');dom.document.body.append(container);
    await act(async()=>{root=mount(container as unknown as Element);root.render(<Panels/>);});
    expect(dom.document.documentElement.style.getPropertyValue('--passwordlist-width')).toBe('320px');
    await act(async()=>{container.querySelector('button')!.dispatchEvent(new dom.MouseEvent('mousedown',{bubbles:true,clientX:540,button:0}));dom.document.dispatchEvent(new dom.MouseEvent('mousemove',{clientX:450}));});
    expect(dom.document.documentElement.style.getPropertyValue('--passwordlist-width')).toBe('230px');
    expect(saves).toHaveLength(0);
    await act(async()=>dom.document.dispatchEvent(new dom.MouseEvent('mouseup')));
    expect(saves[0]).toEqual({sidebarWidth:220,passwordListWidth:230});
    expect(dom.document.body.style.userSelect).toBe('');
    await act(async()=>root!.render(<Panels list={280}/>));
    expect(dom.document.documentElement.style.getPropertyValue('--passwordlist-width')).toBe('280px');
  });

  it('subscribes before a fast Smart Login result and removes both listeners afterwards', async () => {
    const { default: SmartLoginButton } = await import('@/features/entries/components/SmartLoginButton');
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: async (command: string) => command === 'get_runtime_platform' ? 'windows' : null } });
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
    await render({ ...update, target_platform: 'windows-portable', sha256: 'a'.repeat(64) });
    expect(container.textContent).toContain('checksum is checked before installation');
    await render({ ...update, target_platform: 'android', sha256: null });
    expect([...container.querySelectorAll('button')].find(button => button.textContent === 'Download & Install')?.disabled).toBe(true);
    await render({ ...update, download_url: null });
    expect(container.textContent).toContain('No download is available');
    expect([...container.querySelectorAll('button')].find(button => button.textContent === 'Download')?.disabled).toBe(true);
  });

  it('checks once at app startup when opted in, without opening settings', async () => {
    const { UpdaterProvider, useUpdater } = await import('@/features/updater/useUpdater');
    const backend = await getBackend();
    let checks = 0;
    Object.assign(backend, { getAppVersion: async () => '0.2.3', checkAppUpdate: async () => {
      checks++;
      return { current_version: '0.2.3', latest_version: '0.2.3', has_update: false, target_platform: 'android' };
    } });
    dom.localStorage.setItem('yntra-vault-settings', JSON.stringify({ autoCheckUpdates: true }));
    let updater!: ReturnType<typeof useUpdater>;
    function Probe() { updater = useUpdater(); return null; }
    const container = dom.document.createElement('div'); dom.document.body.append(container);
    const ui = (key: string) => <SettingsProvider><ToastProvider><UpdaterProvider><Probe key={key}/></UpdaterProvider></ToastProvider></SettingsProvider>;
    await act(async () => { root = mount(container as unknown as Element); root.render(ui('login')); });
    await act(async () => { await new Promise(resolve => setTimeout(resolve, 2700)); });
    expect(checks).toBe(1);
    expect(updater.status).toBe('up-to-date');
    await act(async () => { root!.render(ui('settings')); });
    expect(checks).toBe(1);
    expect(updater.status).toBe('up-to-date');
  });

  it('prevents overlapping checks/installations and retains Android installer errors for retry', async () => {
    const { UpdaterProvider, useUpdater } = await import('@/features/updater/useUpdater');
    const backend = await getBackend();
    let checks = 0, installs = 0;
    let finishCheck!: (result: CheckUpdateResult) => void;
    let failInstall!: (error: Error) => void;
    Object.assign(backend, {
      getAppVersion: async () => '0.2.3',
      checkAppUpdate: () => { checks++; return new Promise(resolve => { finishCheck = resolve; }); },
      downloadAndInstallApk: () => { installs++; return new Promise((_, reject) => { failInstall = reject; }); },
    });
    let updater!: ReturnType<typeof useUpdater>;
    function Probe() { updater = useUpdater(); return null; }
    const container = dom.document.createElement('div'); dom.document.body.append(container);
    await act(async () => { root = mount(container as unknown as Element); root.render(<SettingsProvider><ToastProvider><UpdaterProvider><Probe/></UpdaterProvider></ToastProvider></SettingsProvider>); });
    let checking: ReturnType<typeof updater.checkForUpdates>;
    await act(async () => { checking = updater.checkForUpdates(); void updater.checkForUpdates(); });
    expect(checks).toBe(1);
    await act(async () => {
      finishCheck({ current_version: '0.2.3', latest_version: '0.2.4', has_update: true, target_platform: 'android',
        download_url: 'https://github.com/YntraAB/yntra-vault/releases/download/v0.2.4/app.apk', sha256: 'a'.repeat(64), signature: null, release_notes: null, pub_date: null });
      await checking;
    });
    let installing: Promise<void>;
    await act(async () => { installing = updater.installUpdate(); void updater.installUpdate(); void updater.checkForUpdates(); });
    expect(installs).toBe(1);
    expect(checks).toBe(1);
    await act(async () => { failInstall(new Error('Allow updates in Android settings, then try again.')); await installing; });
    expect(updater.status).toBe('error');
    expect(updater.isDownloading).toBe(false);
    expect(container.querySelector('[role="alert"]')?.textContent).toContain('Allow updates');
    Object.assign(backend, { downloadAndInstallApk: async () => { installs++; return '/cache/verified.apk'; } });
    await act(async () => { await updater.installUpdate(); });
    expect(installs).toBe(2);
    expect(updater.status).toBe('ready');
    expect(updater.isModalOpen).toBe(false);
  });

  it('keeps the newest result and performs no automatic network breach check', async () => {
    const pending: Array<(value: string) => void> = [];
    const commands: string[] = [];
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: (command: string) => {
      if (command === 'get_runtime_platform') return Promise.resolve('windows');
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
