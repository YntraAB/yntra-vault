import { beforeAll, afterAll, afterEach, describe, expect, it } from 'bun:test';
import { act, useEffect } from 'react';
import { Window } from 'happy-dom';
import type { Root } from 'react-dom/client';
import { AuthProvider, useAuth } from '@/features/auth/context/AuthContext';
import { SettingsProvider } from '@/features/settings/context/SettingsContext';
import { LanguageProvider } from '@/contexts/LanguageContext';
import { ToastProvider } from '@/contexts/ToastContext';
import { getBackend, resetBackend } from '@/lib/backend';
import { ConflictHistoryDialog } from './components/ConflictHistory';

const dom = new Window();
const previous = new Map<string, PropertyDescriptor | undefined>();
let root: Root | undefined;
let mount: typeof import('react-dom/client').createRoot;
let auth!: ReturnType<typeof useAuth>;
let closed = 0, refreshed = 0;
beforeAll(async () => {
  for (const [key, value] of Object.entries({ window: dom, document: dom.document, HTMLElement: dom.HTMLElement, Element: dom.Element, SVGElement: dom.SVGElement, navigator: dom.navigator, localStorage: dom.localStorage, CustomEvent: dom.CustomEvent, IS_REACT_ACT_ENVIRONMENT: true })) {
    previous.set(key, Object.getOwnPropertyDescriptor(globalThis, key)); Object.defineProperty(globalThis, key, { configurable: true, writable: true, value });
  }
  mount = (await import('react-dom/client')).createRoot;
});
afterEach(async () => { await act(async () => root?.unmount()); root = undefined; resetBackend(); dom.document.body.innerHTML = ''; dom.localStorage.clear(); closed = refreshed = 0; });
afterAll(() => { for (const [key, descriptor] of previous) { if (descriptor) Object.defineProperty(globalThis, key, descriptor); else Reflect.deleteProperty(globalThis, key); } dom.happyDOM.abort(); });
async function setup(overrides: Record<string, unknown> = {}) {
  Object.assign(dom, { __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => {} }, __TAURI_INTERNALS__: { transformCallback: () => 1, unregisterCallback: () => {}, invoke: async () => null } });
  const backend = await getBackend();
  Object.assign(backend, {
    listConflictHistory: async () => [{ id: 'old-version', saved_at: '2026-10-23T12:00:00Z' }],
    previewConflictHistory: async () => ({ id: 'preview', local_revision: 'reviewed-revision', remote_revision: 'historical', changes: [{ key: 'entry/notes', entry_id: 'entry', field: 'notes' }] }),
    inspectConflictHistory: async () => 'SYNTHETIC OLD SECRET', restoreConflictHistory: async () => {}, ...overrides,
  });
  function Harness() { const value = useAuth(); useEffect(() => { auth = value; }, [value]); return value.isLocked ? null : <ConflictHistoryDialog backend={backend} entryId="entry" onClose={() => closed++} onRestored={async () => { refreshed++; }} />; }
  const container = dom.document.createElement('div'); dom.document.body.append(container);
  await act(async () => { root = mount(container as unknown as Element); root.render(<ToastProvider><SettingsProvider><AuthProvider><LanguageProvider><Harness /></LanguageProvider></AuthProvider></SettingsProvider></ToastProvider>); });
  await act(async () => { auth.setCurrentVault({ id: 'vault', name: 'Fixture', path: '/fixture.vdb' }); auth.setIsLocked(false); });
}
function button(text: string) { const value = [...dom.document.querySelectorAll('button')].find(button => button.textContent === text); if (!value) throw Error(`Missing ${text}`); return value; }
async function selectVersion() { await act(async () => (dom.document.querySelector('[aria-pressed]') as unknown as HTMLButtonElement).click()); }
describe('conflict history', () => {
  it('keeps values masked until requested and restores only the reviewed revision', async () => {
    let reveals = 0; const restores: unknown[][] = [];
    await setup({ inspectConflictHistory: async () => { reveals++; return 'SYNTHETIC OLD SECRET'; }, restoreConflictHistory: async (...args: unknown[]) => { restores.push(args); } });
    await selectVersion();
    expect(reveals).toBe(0); expect(dom.document.body.textContent).not.toContain('SYNTHETIC OLD SECRET');
    await act(async () => button('Show values').click());
    expect(reveals).toBe(1); expect(dom.document.body.textContent).toContain('SYNTHETIC OLD SECRET');
    await act(async () => button('Hide values').click());
    expect(dom.document.body.textContent).not.toContain('SYNTHETIC OLD SECRET');
    await act(async () => button('Restore version').click());
    expect(restores).toEqual([['entry', 'old-version', 'reviewed-revision']]); expect(refreshed).toBe(1); expect(closed).toBe(1);
  });
  it('keeps a stale restoration error visible without reporting success', async () => {
    await setup({ restoreConflictHistory: async () => { throw Error('Vault changed; review again'); } }); await selectVersion();
    await act(async () => button('Restore version').click());
    expect(dom.document.querySelector('[role=alert]')?.textContent).toContain('Vault changed'); expect(refreshed).toBe(0); expect(closed).toBe(0);
  });
  it('discards historical secret responses after locking', async () => {
    let resolve!: (secret: string) => void;
    await setup({ inspectConflictHistory: async () => new Promise<string>(done => { resolve = done; }) }); await selectVersion();
    await act(async () => button('Show values').click());
    await act(async () => auth.lockVault());
    await act(async () => resolve('LATE SYNTHETIC SECRET'));
    expect(dom.document.querySelector('dialog')).toBeNull(); expect(dom.document.body.textContent).not.toContain('LATE SYNTHETIC SECRET');
  });
});
