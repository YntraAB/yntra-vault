import { beforeAll, afterAll, afterEach, describe, expect, it } from 'bun:test';
import { act, useEffect } from 'react';
import { Window } from 'happy-dom';
import type { Root } from 'react-dom/client';
import { AuthProvider, useAuth } from '@/features/auth/context/AuthContext';
import { SettingsProvider } from '@/features/settings/context/SettingsContext';
import { LanguageProvider } from '@/contexts/LanguageContext';
import { ToastProvider } from '@/contexts/ToastContext';
import { getBackend, resetBackend } from '@/lib/backend';
import { SyncReviewHost } from './components/SyncReview';
import type { SyncPreview, SyncDecision } from './types';

const dom = new Window();
const previous = new Map<string, PropertyDescriptor | undefined>();
let root: Root | undefined;
let mount: typeof import('react-dom/client').createRoot;
let auth!: ReturnType<typeof useAuth>;
let publish: (value: SyncPreview | null) => void;
const preview: SyncPreview = {
  id: 'review-1', local_revision: 'left', remote_revision: 'right', changes: [{
    key: 'entry/notes', entry_id: 'entry', title: 'Discord', field: 'notes', conflict: true, suggested: null,
    local_changed_at: '2026-10-20T10:00:00Z', remote_changed_at: '2026-10-21T10:00:00Z',
    local_device: 'desktop', remote_device: 'phone', local_present: true, remote_present: true,
    sensitive: true, can_keep_both: true, can_edit: true,
  }],
};

beforeAll(async () => {
  for (const [key, value] of Object.entries({ window: dom, document: dom.document, HTMLElement: dom.HTMLElement, Element: dom.Element, SVGElement: dom.SVGElement, navigator: dom.navigator, localStorage: dom.localStorage, CustomEvent: dom.CustomEvent, IS_REACT_ACT_ENVIRONMENT: true })) {
    previous.set(key, Object.getOwnPropertyDescriptor(globalThis, key));
    Object.defineProperty(globalThis, key, { configurable: true, writable: true, value });
  }
  mount = (await import('react-dom/client')).createRoot;
});
afterEach(async () => { await act(async () => root?.unmount()); root = undefined; resetBackend(); dom.document.body.innerHTML = ''; dom.localStorage.clear(); });
afterAll(() => { for (const [key, descriptor] of previous) { if (descriptor) Object.defineProperty(globalThis, key, descriptor); else Reflect.deleteProperty(globalThis, key); } dom.happyDOM.abort(); });

async function setup(overrides: Record<string, unknown> = {}) {
  Object.assign(dom, { __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => {} }, __TAURI_INTERNALS__: { transformCallback: () => 1, unregisterCallback: () => {}, invoke: async () => null } });
  const backend = await getBackend();
  Object.assign(backend, { onSyncReview: async (callback: typeof publish) => { publish = callback; return () => {}; }, getSyncReview: async () => preview,
    inspectSyncField: async (_id: string, _key: string, remote: boolean) => remote ? 'PHONE_SYNTHETIC_SECRET' : 'DESKTOP_SYNTHETIC_SECRET',
    approveSyncReview: async () => {}, cancelSyncReview: async () => {}, ...overrides });
  function Harness() { const value = useAuth(); useEffect(() => { auth = value; }, [value]); return <SyncReviewHost />; }
  const container = dom.document.createElement('div'); dom.document.body.append(container);
  await act(async () => { root = mount(container as unknown as Element); root.render(<ToastProvider><SettingsProvider><AuthProvider><LanguageProvider><Harness /></LanguageProvider></AuthProvider></SettingsProvider></ToastProvider>); });
  await act(async () => { auth.setCurrentVault({ id: 'vault', name: 'Fixture', path: '/fixture.vdb' }); auth.setIsLocked(false); });
  return backend;
}

function button(label: string) {
  const found = Array.from(dom.document.querySelectorAll('button')).find(button => button.textContent === label);
  if (!found) throw new Error(`Missing button: ${label}`);
  return found;
}

describe('synchronization review', () => {
  it('masks values, orders the newest first, and requires a conflict choice', async () => {
    let calls = 0;
    await setup({ inspectSyncField: async () => { calls++; return 'SECRET'; } });
    expect(calls).toBe(0);
    expect(dom.document.body.textContent).not.toContain('SECRET');
    expect(button('Approve and synchronize').disabled).toBe(true);
    expect(dom.document.querySelector<HTMLInputElement>('input[type=radio]')?.value).toBe('remote');
    await act(async () => button('Show values').click());
    expect(calls).toBe(2); expect(dom.document.body.textContent).toContain('SECRET');
    await act(async () => button('Hide values').click());
    expect(dom.document.body.textContent).not.toContain('SECRET');
  });

  it('submits keep both and blocks duplicate approval while awaiting IPC', async () => {
    let submitted: SyncDecision[] = []; let finish!: () => void; let count = 0;
    await setup({ approveSyncReview: async (_id: string, decisions: SyncDecision[]) => { count++; submitted = decisions; await new Promise<void>(resolve => { finish = resolve; }); } });
    const label = Array.from(dom.document.querySelectorAll('label')).find(label => label.textContent?.includes('Keep both'))!;
    await act(async () => label.querySelector('input')!.click());
    expect(button('Approve and synchronize').disabled).toBe(false);
    await act(async () => { button('Approve and synchronize').click(); });
    expect(count).toBe(1); expect(submitted).toEqual([{ key: 'entry/notes', choice: 'both' }]);
    await act(async () => finish());
    expect(dom.document.querySelector('dialog')).toBeNull();
  });

  it('discards a reveal received after locking and does not revive an old review', async () => {
    const reveals: Array<(value: string) => void> = [];
    const backend = await setup({ inspectSyncField: async () => new Promise<string>(resolve => { reveals.push(resolve); }) });
    await act(async () => button('Show values').click());
    await act(async () => { auth.setIsLocked(true); });
    await act(async () => reveals.forEach(resolve => resolve('LATE_SECRET')));
    expect(dom.document.body.textContent).not.toContain('LATE_SECRET'); expect(dom.document.querySelector('dialog')).toBeNull();
    Object.assign(backend, { getSyncReview: async () => null });
    await act(async () => { auth.setCurrentVault({ id: 'other', name: 'Other', path: '/other.vdb' }); auth.setIsLocked(false); });
    expect(dom.document.querySelector('dialog')).toBeNull();
  });

  it('keeps IPC errors visible without treating a failed approval as completion', async () => {
    await setup({ approveSyncReview: async () => { throw new Error('Vault changed during review'); } });
    await act(async () => dom.document.querySelector<HTMLInputElement>('input[type=radio]')!.click());
    await act(async () => button('Approve and synchronize').click());
    expect(dom.document.querySelector('[role=alert]')?.textContent).toContain('Vault changed during review');
    expect(dom.document.querySelector('dialog')).not.toBeNull();
    await act(async () => publish(null));
    expect(dom.document.querySelector('dialog')).toBeNull();
  });

  it.each(['encrypted_password', 'custom/secondary-password'])('submits a coherent keep-both account choice for %s', async field => {
    let submitted: SyncDecision[] = [];
    const notes = preview.changes[0];
    const password = { ...notes, key: `entry/${field}`, field, can_edit: false, keep_both_entries: [{ id: 'entry', title: 'Discord (this device)' }, { id: 'copy', title: 'Discord (other device)' }] };
    await setup({ getSyncReview: async () => ({ ...preview, changes: [notes, password] }), approveSyncReview: async (_id: string, decisions: SyncDecision[]) => { submitted = decisions; } });
    expect(dom.document.querySelectorAll('details').length).toBe(1);
    const rows = dom.document.querySelectorAll('fieldset');
    await act(async () => rows[0].querySelector<HTMLInputElement>('input[value=local]')!.click());
    const keepBoth = Array.from(rows[1].querySelectorAll('label')).find(label => label.textContent?.includes('Keep both'))!;
    await act(async () => keepBoth.querySelector('input')!.click());
    expect(rows[0].disabled).toBe(true);
    expect(dom.document.body.textContent).toContain('Discord (other device)');
    expect(button('Approve and synchronize').disabled).toBe(false);
    await act(async () => button('Approve and synchronize').click());
    expect(submitted).toEqual([{ key: password.key, choice: 'both' }]);
  });
});
