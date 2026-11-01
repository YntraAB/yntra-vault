import { beforeAll, afterAll, afterEach, describe, expect, it } from 'bun:test';
import { act, useEffect } from 'react';
import { Window } from 'happy-dom';
import type { Root } from 'react-dom/client';
import { AuthProvider, useAuth } from '@/features/auth/context/AuthContext';
import { SettingsProvider } from '@/features/settings/context/SettingsContext';
import { LanguageProvider } from '@/contexts/LanguageContext';
import { ToastProvider } from '@/contexts/ToastContext';
import { getBackend, resetBackend } from '@/lib/backend';
import { useTrustedDevices } from './hooks/useTrustedDevices';
import type { SyncOperation } from './types';

const dom = new Window();
const previous = new Map<string, PropertyDescriptor | undefined>();
let root: Root | undefined;
let mount: typeof import('react-dom/client').createRoot;
let auth!: ReturnType<typeof useAuth>;
let emit!: (operation: SyncOperation) => void;
beforeAll(async () => {
  for (const [key, value] of Object.entries({ window: dom, document: dom.document, HTMLElement: dom.HTMLElement, Element: dom.Element, SVGElement: dom.SVGElement, navigator: dom.navigator, localStorage: dom.localStorage, CustomEvent: dom.CustomEvent, IS_REACT_ACT_ENVIRONMENT: true })) {
    previous.set(key, Object.getOwnPropertyDescriptor(globalThis, key)); Object.defineProperty(globalThis, key, { configurable: true, writable: true, value });
  }
  mount = (await import('react-dom/client')).createRoot;
});
afterEach(async () => { await act(async () => root?.unmount()); root = undefined; resetBackend(); dom.document.body.innerHTML = ''; dom.localStorage.clear(); });
afterAll(() => { for (const [key, descriptor] of previous) { if (descriptor) Object.defineProperty(globalThis, key, descriptor); else Reflect.deleteProperty(globalThis, key); } dom.happyDOM.abort(); });
async function setup(getTrustedDevices: () => Promise<unknown[]>) {
  Object.assign(dom, { __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => {} }, __TAURI_INTERNALS__: { transformCallback: () => 1, unregisterCallback: () => {}, invoke: async () => null } });
  const backend = await getBackend();
  Object.assign(backend, { getTrustedDevices, onSyncOperation: async (callback: typeof emit) => { emit = callback; return () => {}; } });
  function Devices() { const { trustedDevices } = useTrustedDevices(); return <output>{trustedDevices.map(device => device.last_sync_at).join(',')}</output>; }
  function Harness() { const value = useAuth(); useEffect(() => { auth = value; }, [value]); return value.isLocked ? null : <Devices />; }
  const container = dom.document.createElement('div'); dom.document.body.append(container);
  await act(async () => { root = mount(container as unknown as Element); root.render(<ToastProvider><SettingsProvider><AuthProvider><LanguageProvider><Harness /></LanguageProvider></AuthProvider></SettingsProvider></ToastProvider>); });
  await act(async () => { auth.setCurrentVault({ id: 'vault', name: 'Fixture', path: '/fixture.vdb' }); auth.setIsLocked(false); });
}
const completed: SyncOperation = { id: 'operation', transport: 'p2p_listener', phase: 'complete', error: null };
describe('paired device updates', () => {
  it('updates last sync after receiving a remote initiated synchronization', async () => {
    let date = 'before';
    await setup(async () => [{ id: 'peer', last_sync_at: date }]);
    expect(dom.document.querySelector('output')?.textContent).toBe('before');
    date = 'confirmed';
    await act(async () => emit(completed));
    expect(dom.document.querySelector('output')?.textContent).toBe('confirmed');
  });
  it('discards late results and errors from a locked session', async () => {
    let pending = false, reject!: (error: Error) => void;
    await setup(async () => pending ? new Promise((_, fail) => { reject = fail; }) : []);
    pending = true;
    await act(async () => emit(completed));
    await act(async () => auth.lockVault());
    await act(async () => reject(Error('STALE DEVICE RESPONSE')));
    expect(dom.document.querySelector('output')).toBeNull();
    expect(dom.document.body.textContent).not.toContain('STALE DEVICE RESPONSE');
  });
});
