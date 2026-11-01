import { beforeAll, afterAll, afterEach, describe, expect, it } from 'bun:test';
import { act } from 'react';
import { Window } from 'happy-dom';
import type { Root } from 'react-dom/client';
import { LanguageProvider } from '@/contexts/LanguageContext';
import { SettingsProvider } from '@/features/settings/context/SettingsContext';
import { ToastProvider } from '@/contexts/ToastContext';
import { EntriesContext, type EntriesContextType } from './context/EntriesContext.model';
import { CreateTagModal } from './components/CreateTagModal';
import { DeleteTagModal } from './components/DeleteTagModal';
const dom = new Window();
const previous = new Map<string, PropertyDescriptor | undefined>();
let root: Root | undefined;
let mount: typeof import('react-dom/client').createRoot;
let closed = 0;
beforeAll(async () => {
  for (const [key, value] of Object.entries({ window: dom, document: dom.document, HTMLElement: dom.HTMLElement, Element: dom.Element, SVGElement: dom.SVGElement, navigator: dom.navigator, localStorage: dom.localStorage, CustomEvent: dom.CustomEvent, IS_REACT_ACT_ENVIRONMENT: true })) {
    previous.set(key, Object.getOwnPropertyDescriptor(globalThis, key)); Object.defineProperty(globalThis, key, { configurable: true, writable: true, value });
  }
  mount = (await import('react-dom/client')).createRoot;
});
afterEach(async () => { await act(async () => root?.unmount()); root = undefined;  dom.document.body.innerHTML = ''; dom.localStorage.clear(); closed = 0; });
afterAll(() => { for (const [key, descriptor] of previous) { if (descriptor) Object.defineProperty(globalThis, key, descriptor); else Reflect.deleteProperty(globalThis, key); } dom.happyDOM.abort(); });

async function render(component: React.ReactNode) {
  const container = dom.document.createElement('div'); dom.document.body.append(container);
  await act(async () => { root = mount(container as unknown as Element); root.render(<ToastProvider><SettingsProvider><LanguageProvider>{component}</LanguageProvider></SettingsProvider></ToastProvider>); });
}
function button(label: string) { const result = [...dom.document.querySelectorAll('button')].find(button => button.textContent === label); if (!result) throw Error(label); return result; }
describe('tag dialog storage results', () => {
  it('blocks duplicate submit and dismissal until creation is stored, and stays open on failure', async () => {
    let calls = 0; let complete!: (saved: boolean) => void;
    const context = { tags: [], addTag: async () => { calls++; return new Promise<boolean>(done => { complete = done; }); } } as unknown as EntriesContextType;
    await render(<EntriesContext.Provider value={context}><CreateTagModal open onClose={() => closed++} /></EntriesContext.Provider>);
    const input = dom.document.querySelector('input')!;
    await act(async () => { Object.getOwnPropertyDescriptor(dom.HTMLInputElement.prototype, 'value')!.set!.call(input, 'Synthetic tag'); input.dispatchEvent(new dom.Event('input', { bubbles: true })); input.dispatchEvent(new dom.Event('change', { bubbles: true })); });
    const form = dom.document.querySelector('form')!;
    await act(async () => { form.dispatchEvent(new dom.Event('submit', { bubbles: true, cancelable: true })); form.dispatchEvent(new dom.Event('submit', { bubbles: true, cancelable: true })); });
    expect(calls).toBe(1); expect(closed).toBe(0);
    await act(async () => { dom.document.querySelector('[role=dialog]')!.dispatchEvent(new dom.KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); });
    expect(closed).toBe(0);
    await act(async () => complete(false)); expect(closed).toBe(0);
    await act(async () => { form.dispatchEvent(new dom.Event('submit', { bubbles: true, cancelable: true })); });
    expect(calls).toBe(2);
    await act(async () => complete(true)); expect(closed).toBe(1);
  });
  it('keeps deletion open on backend error and closes only on confirmed retry', async () => {
    let attempt = 0;
    await render(<DeleteTagModal tag={{id:'fixture',name:'Synthetic',color:'#777777',icon:'tag',count:0}} onClose={() => closed++} onConfirm={async () => { if (attempt++ === 0) throw Error('Disk full'); return true; }} />);
    await act(async () => button('Delete').click());
    expect(closed).toBe(0); expect(dom.document.querySelector('[role=alert]')!.textContent).toContain('Disk full');
    await act(async () => button('Delete').click()); expect(closed).toBe(1);
  });
});
