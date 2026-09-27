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

async function fillInput(input: HTMLInputElement, value: string) {
  await act(async () => {
    input.focus();
    Object.getOwnPropertyDescriptor(dom.HTMLInputElement.prototype, 'value')!.set!.call(input, value);
    input.dispatchEvent(new dom.Event('input', { bubbles: true }));
    input.dispatchEvent(new dom.KeyboardEvent('keyup', { bubbles: true, key: 'a' }));
  });
}

beforeAll(async () => {
  for (const [key, value] of Object.entries({ window: dom, document: dom.document, HTMLElement: dom.HTMLElement, Element: dom.Element, SVGElement: dom.SVGElement, navigator: dom.navigator, localStorage: dom.localStorage, CustomEvent: dom.CustomEvent, IS_REACT_ACT_ENVIRONMENT: true })) {
    previous.set(key, Object.getOwnPropertyDescriptor(globalThis, key));
    Object.defineProperty(globalThis, key, { configurable: true, writable: true, value });
  }
  mount = (await import('react-dom/client')).createRoot;
});
afterEach(async () => {
  (await import('framer-motion')).MotionGlobalConfig.skipAnimations = false;
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
  it('starts locked and selecting a vault alone cannot unlock it', async () => {
    let auth!: ReturnType<typeof useAuth>;
    function Harness() { const value = useAuth(); useEffect(() => { auth = value; }, [value]); return null; }
    const container = dom.document.createElement('div'); dom.document.body.append(container);
    await act(async () => { root = mount(container as unknown as Element); root.render(<ToastProvider><AuthProvider><Harness /></AuthProvider></ToastProvider>); });
    expect(auth.isLocked).toBe(true);
    await act(async () => auth.setCurrentVault({ id: 'fixture', name: 'Fixture', path: '/fixture.vdb' }));
    expect(auth.isLocked).toBe(true);
  });

  it('prevents empty advancement and short Unicode passwords during rekey, and blocks duplicate submissions', async () => {
    const { ChangeMasterPasswordModal } = await import('@/features/auth/components/ChangeMasterPasswordModal');
    const { frameSteps, frameData } = await import('motion-dom');
    (await import('framer-motion')).MotionGlobalConfig.skipAnimations = true;
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: async () => null } });
    const backend = await getBackend(); let calls = 0, closed = 0;
    let finish!: () => void;
    Object.assign(backend, { changeMasterPasswordBytes: async () => { calls++; return new Promise<void>(resolve => { finish = resolve; }); } });
    const container = dom.document.createElement('div'); dom.document.body.append(container);
    await act(async () => { root = mount(container as unknown as Element); root.render(<ToastProvider><ChangeMasterPasswordModal open onClose={() => { closed++; }} /></ToastProvider>); });
    const button = (text: string) => [...container.querySelectorAll('button')].find(b => b.textContent?.trim() === text)!;
    expect(button('Next').disabled).toBe(true);
    const current = container.querySelector<HTMLInputElement>('input[type=password]')!;
    await act(async () => current.dispatchEvent(new dom.KeyboardEvent('keydown', { bubbles: true, key: 'Enter' })));
    expect(button('Next').disabled).toBe(true);
    await fillInput(current, 'legacy-pass');
    await act(async () => button('Next').click());
    for (let tick = 0; tick < 4; tick++) await act(async () => { frameData.timestamp = Math.max(frameData.timestamp, performance.now()) + 500; for (const step of Object.values(frameSteps)) step.process(frameData); });
    const passwords = container.querySelectorAll<HTMLInputElement>('input[type=password]');
    expect(passwords.length).toBe(2);
    for (const password of ['            ', '🔐🔐🔐🔐🔐🔐']) {
      await fillInput(passwords[0], password); await fillInput(passwords[1], password);
      expect(button('Change Master Password').disabled).toBe(true);
      await act(async () => passwords[1].dispatchEvent(new dom.KeyboardEvent('keydown', { bubbles: true, key: 'Enter' })));
      expect(calls).toBe(0);
    }
    await fillInput(passwords[0], 'valid-fixture-password'); await fillInput(passwords[1], 'valid-fixture-password');
    await act(async () => { button('Change Master Password').click(); button('Change Master Password').click(); });
    expect(calls).toBe(1);
    await act(async () => { container.querySelector<HTMLDivElement>('.fixed')!.click(); dom.dispatchEvent(new dom.KeyboardEvent('keydown', { key: 'Escape' })); });
    expect(closed).toBe(0);
    await act(async () => finish());
    expect(closed).toBe(1);
  });

  it('rejects invalid new-vault credentials before any path dialog or backend creation', async () => {
    const { CreateVaultModal } = await import('@/features/auth/components/CreateVaultModal');
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: async () => null } });
    const backend = await getBackend();
    let paths = 0, creates = 0;
    Object.assign(backend, { getMobileVaultPath: async () => { paths++; return '/fixture.vdb'; }, createVaultBytes: async () => { creates++; } });
    const container = dom.document.createElement('div'); dom.document.body.append(container);
    await act(async () => { root = mount(container as unknown as Element); root.render(<CreateVaultModal open onClose={() => {}} onCreated={() => {}} />); });
    await fillInput(container.querySelector<HTMLInputElement>('input[type=text]')!, 'Fixture');
    const passwords = container.querySelectorAll<HTMLInputElement>('input[type=password]');
    for (const password of ['', 'short', '            ', '🔐🔐🔐🔐🔐🔐']) {
      await fillInput(passwords[0], password);
      await fillInput(passwords[1], password);
      expect(container.querySelector<HTMLButtonElement>('button[type=submit]')!.disabled).toBe(true);
      await act(async () => container.querySelector('form')!.dispatchEvent(new dom.Event('submit', { bubbles: true, cancelable: true })));
      expect(paths).toBe(0); expect(creates).toBe(0);
    }
    await fillInput(passwords[0], 'valid-fixture-password');
    await fillInput(passwords[1], 'different-fixture-password');
    expect(container.querySelector<HTMLButtonElement>('button[type=submit]')!.disabled).toBe(true);
    await fillInput(passwords[1], 'valid-fixture-password');
    expect(container.querySelector<HTMLButtonElement>('button[type=submit]')!.disabled).toBe(false);
  });

  it('blocks invalid recovery credentials even on direct form submission', async () => {
    const { RecoveryForm } = await import('@/features/auth/components/LocalProtection');
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: async () => null } });
    const backend = await getBackend(); let calls = 0;
    Object.assign(backend, { recoverVault: async () => { calls++; } });
    const container = dom.document.createElement('div'); dom.document.body.append(container);
    await act(async () => { root = mount(container as unknown as Element); root.render(<RecoveryForm path="/fixture.vdb" onBack={() => {}} onRecovered={() => {}} />); });
    const fields = container.querySelectorAll<HTMLInputElement>('input');
    await fillInput(fields[0], 'fixture-share-a'); await fillInput(fields[1], 'fixture-share-b');
    for (const password of ['', 'short', '            ', '🔐🔐🔐🔐🔐🔐']) {
      await fillInput(fields[2], password); await fillInput(fields[3], password);
      expect(container.querySelector<HTMLButtonElement>('button[type=submit]')!.disabled).toBe(true);
      await act(async () => container.querySelector('form')!.dispatchEvent(new dom.Event('submit', { bubbles: true, cancelable: true })));
      expect(calls).toBe(0);
    }
    await fillInput(fields[2], 'valid-fixture-password'); await fillInput(fields[3], 'valid-fixture-password');
    await fillInput(fields[1], 'fixture-share-a');
    expect(container.querySelector<HTMLButtonElement>('button[type=submit]')!.disabled).toBe(true);
  });

  it('clears disconnected USB selections and failed scans instead of retaining a stale binding', async () => {
    const { UsbPicker } = await import('@/features/auth/components/LocalProtection');
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: async () => null } });
    const backend = await getBackend();
    let devices = [{ id: 'drive-one', name: 'USB' }], fail = false;
    Object.assign(backend, { listUsbStorageDevices: async () => { if (fail) throw new Error('Scan failed'); return devices; } });
    const values: string[] = [];
    function Harness() {
      const [value, setValue] = React.useState('drive-one');
      return <UsbPicker value={value} onChange={next => { values.push(next); setValue(next); }} />;
    }
    const container = dom.document.createElement('div'); dom.document.body.append(container);
    await act(async () => { root = mount(container as unknown as Element); root.render(<Harness />); });
    expect(container.querySelector('select')?.value).toBe('drive-one');
    devices = [];
    await act(async () => container.querySelector('button')!.click());
    expect(values).toEqual(['']);
    expect(container.querySelector('select')?.value).toBe('');
    expect(container.textContent).toContain('Connect a USB drive');
    fail = true;
    await act(async () => container.querySelector('button')!.click());
    expect(container.querySelector('[role=alert]')?.textContent).toContain('Scan failed');
    expect(container.textContent).not.toContain('Connect a USB drive');
  });

  it('blocks duplicate creation and dismissal while choosing a vault path, then recovers after cancellation', async () => {
    const { CreateVaultModal } = await import('@/features/auth/components/CreateVaultModal');
    let paths = 0, closes = 0, creates = 0;
    let finish!: (value: string | null) => void;
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: async () => null } });
    const backend = await getBackend();
    Object.assign(backend, {
      getMobileVaultPath: () => { paths++; return new Promise<string | null>(resolve => { finish = resolve; }); },
      createVaultBytes: async () => { creates++; },
    });
    const container = dom.document.createElement('div'); dom.document.body.append(container);
    await act(async () => { root = mount(container as unknown as Element); root.render(<CreateVaultModal open onClose={() => { closes++; }} onCreated={() => {}} />); });
    await fillInput(container.querySelector<HTMLInputElement>('input[type=text]')!, 'Fixture');
    const passwords = container.querySelectorAll<HTMLInputElement>('input[type=password]');
    await fillInput(passwords[0], 'fixture-password');
    await fillInput(passwords[1], 'fixture-password');
    const form = container.querySelector('form')!;
    await act(async () => {
      form.dispatchEvent(new dom.Event('submit', { bubbles: true, cancelable: true }));
      form.dispatchEvent(new dom.Event('submit', { bubbles: true, cancelable: true }));
    });
    expect(paths).toBe(1);
    expect(container.querySelector('fieldset')?.disabled).toBe(true);
    await act(async () => {
      container.querySelector<HTMLDivElement>('.fixed')!.click();
      dom.dispatchEvent(new dom.KeyboardEvent('keydown', { key: 'Escape' }));
    });
    expect(closes).toBe(0);
    await act(async () => finish(null));
    await act(async () => { await new Promise(resolve => setTimeout(resolve, 20)); });
    expect(creates).toBe(0);
    expect(container.querySelector('fieldset')?.disabled).toBe(false);
    await act(async () => container.querySelector<HTMLDivElement>('.fixed')!.click());
    expect(closes).toBe(1);
  });

  it('keeps recovery actions unavailable when protection status fails to load', async () => {
    const { LocalProtectionSettings } = await import('@/features/auth/components/LocalProtection');
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: async () => null } });
    const backend = await getBackend();
    let fails = true;
    Object.assign(backend, { getLocalProtection: async () => { if (fails) throw new Error('Status failed'); return { protected: false, usb_bound: false, recovery_enabled: false }; } });
    const container = dom.document.createElement('div'); dom.document.body.append(container);
    await act(async () => { root = mount(container as unknown as Element); root.render(<LocalProtectionSettings />); });
    const create = () => [...container.querySelectorAll('button')].find(button => button.textContent === 'Create')!;
    expect(create().disabled).toBe(true);
    expect(container.querySelector('[role=alert]')?.textContent).toContain('Status failed');
    fails = false;
    await act(async () => [...container.querySelectorAll('button')].find(button => button.textContent === 'Retry')!.click());
    expect(create().disabled).toBe(false);
  });

  it('still shows a newly created vault recovery kit when recent-vault history is damaged', async () => {
    const { CreateVaultModal } = await import('@/features/auth/components/CreateVaultModal');
    const { initializePlatform } = await import('@/lib/platform');
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: async (command: string) => command === 'get_runtime_platform' ? 'windows' : null } });
    await initializePlatform();
    const backend = await getBackend();
    const kit = { vault_id: 'fixture', vault_name: 'Fixture', created_at: '', generated_at: '', format_version: 2, total_entries: 0, verification_hash: 'fixture', document_markdown: '', shares: [1, 2, 3].map(i => ({ share_index: i, label: `Share ${i}`, share_data: `fixture-${i}` })) };
    let creates = 0;
    Object.assign(backend, {
      getMobileVaultPath: async () => '/fixture.vdb',
      listUsbStorageDevices: async () => [{ id: 'fixture-usb', name: 'Fixture USB' }],
      createProtectedVault: async () => { creates++; return { info: { id: 'fixture', name: 'Fixture', path: '/fixture.vdb' }, kit }; },
    });
    dom.localStorage.setItem('yntra-vault-recent-vaults', '{invalid');
    const container = dom.document.createElement('div'); dom.document.body.append(container);
    await act(async () => { root = mount(container as unknown as Element); root.render(<CreateVaultModal open onClose={() => {}} onCreated={() => {}} />); });
    const fill = async (input: HTMLInputElement, value: string) => {
      await act(async () => {
        input.focus();
        Object.getOwnPropertyDescriptor(dom.HTMLInputElement.prototype, 'value')!.set!.call(input, value);
        input.dispatchEvent(new dom.Event('input', { bubbles: true }));
        input.dispatchEvent(new dom.KeyboardEvent('keyup', { bubbles: true, key: 'a' }));
      });
    };
    await fill(container.querySelector<HTMLInputElement>('input[type=text]')!, 'Fixture');
    const passwords = container.querySelectorAll<HTMLInputElement>('input[type=password]');
    await fill(passwords[0], 'synthetic-fixture-password');
    await fill(passwords[1], 'synthetic-fixture-password');
    const usbToggle = [...container.querySelectorAll('label')].find(label => label.textContent?.includes('USB protection'))!.querySelector('input')!;
    await act(async () => usbToggle.click());
    await act(async () => { const select = container.querySelector('select')!; select.value = 'fixture-usb'; select.dispatchEvent(new dom.Event('change', { bubbles: true })); });
    await act(async () => container.querySelector('form')!.dispatchEvent(new dom.Event('submit', { bubbles: true, cancelable: true })));
    expect(creates).toBe(1);
    expect(dom.document.querySelector('[role=dialog]')?.textContent).toContain('Start saving');
  });

  it('shows imported tags immediately without reopening the vault', async () => {
    const { ImportModal } = await import('@/features/sync/components/ImportModal');
    const { frameSteps, frameData } = await import('motion-dom');
    (await import('framer-motion')).MotionGlobalConfig.skipAnimations = true;
    Object.assign(dom, { __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => {} }, __TAURI_INTERNALS__: { transformCallback: () => 1, unregisterCallback: () => {}, invoke: async (command: string) => command === 'plugin:dialog|open' ? '/fixture.csv' : null } });
    const backend = await getBackend();
    let imported = false;
    Object.assign(backend, {
      listEntries: async () => [],
      getTags: async () => imported ? [{ id: 'work', name: 'Work', color: '', icon: 'tag' }] : [],
      parseImportFile: async () => ({ total_found: 1, duplicates_count: 0, format_detected: 'CSV', entries: [{ title: 'GitHub', username: 'fixture', url: 'https://github.com', tags: ['Work'], is_duplicate: false }] }),
      importEntries: async () => { imported = true; return 1; },
    });
    dom.localStorage.setItem('yntra-vault-settings', JSON.stringify({ disableSkeletonDelays: true }));
    let auth!: ReturnType<typeof useAuth>;
    let entries!: ReturnType<typeof useEntries>;
    function Harness() {
      const a = useAuth(), e = useEntries();
      useEffect(() => { auth = a; entries = e; }, [a, e]);
      return <ImportModal isOpen onClose={() => {}} />;
    }
    const container = dom.document.createElement('div'); dom.document.body.append(container);
    await act(async () => { root = mount(container as unknown as Element); root.render(<ToastProvider><SettingsProvider><AuthProvider><EntriesProvider><Harness /></EntriesProvider></AuthProvider></SettingsProvider></ToastProvider>); });
    await act(async () => { auth.setCurrentVault({ id: 'test', name: 'Test', path: '/test.vdb' }); auth.setIsLocked(false); });
    const click = async (label: string) => {
      const button = [...container.querySelectorAll('button')].find(button => button.textContent?.includes(label));
      expect(button).toBeDefined();
      await act(async () => { button!.click(); });
      await act(async () => { await new Promise(resolve => setTimeout(resolve, 20)); });
      for (let tick = 0; tick < 4; tick++) {
        await act(async () => {
          frameData.timestamp = Math.max(frameData.timestamp, performance.now()) + 500;
          for (const step of Object.values(frameSteps)) step.process(frameData);
        });
      }
    };
    await click('Recommended');
    await click('Browse File');
    await click('Import 1 Items');
    expect(imported).toBe(true);
    expect(entries.tags.some(tag => tag.name === 'Work')).toBe(true);
  });

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
    await act(async()=>{auth.setCurrentVault({id:'test',name:'Test',path:'/test.vdb'});auth.setIsLocked(false);entries.toggleP2pListener(true);});
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

  it('opens only one recovery export dialog and does not mark a cancelled export as saved', async () => {
    const { RecoveryKitCards } = await import('@/features/auth/components/RecoveryKitWizard');
    Object.assign(dom, { __TAURI_INTERNALS__: { invoke: async () => null } });
    const backend = await getBackend();
    let dialogs = 0, exports = 0;
    let finish!: (path: string | null) => void;
    Object.assign(backend, {
      saveFileDialog: () => { dialogs++; return new Promise<string | null>(resolve => { finish = resolve; }); },
      exportRecoveryShare: async () => { exports++; },
    });
    const kit = { vault_id: 'fixture', vault_name: 'Fixture', created_at: '', generated_at: '', format_version: 2, total_entries: 0, verification_hash: 'fixture', document_markdown: '', shares: [1, 2, 3].map(i => ({ share_index: i, label: `Share ${i}`, share_data: `fixture-${i}` })) };
    const container = dom.document.createElement('div'); dom.document.body.append(container);
    await act(async () => { root = mount(container as unknown as Element); root.render(<RecoveryKitCards kit={kit} />); });
    const button = (label: string) => [...container.querySelectorAll('button')].find(b => b.textContent?.trim() === label)!;
    await act(async () => button('Start saving').click());
    await act(async () => { button('Save this share').click(); button('Save this share').click(); });
    expect(dialogs).toBe(1);
    expect(container.querySelector<HTMLInputElement>('input[type=checkbox]')?.disabled).toBe(true);
    await act(async () => finish(null));
    expect(exports).toBe(0);
    expect(container.querySelector('[role=status]')).toBeNull();
    expect(button('Continue').disabled).toBe(true);
    expect(button('Save this share').disabled).toBe(false);
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
    await act(async () => { auth.setCurrentVault({ id: 'vault-1', name: 'Test', path: '/test.vdb' }); auth.setIsLocked(false); });
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
