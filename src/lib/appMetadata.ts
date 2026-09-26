import { invokeIpc } from '@/types/ipc';

const KEYS = ['yntra-vault-recent-vaults', 'yntra-vault-settings', 'yntra-vault-theme', 'yntra-vault-setup-completed'] as const;
type Values = Record<string, string>;
type NativeStore = { load: () => Promise<Values>; save: (values: Values, initialize?: boolean) => Promise<void> };

/** A small, explicit metadata store. Secrets and keyfile locations are excluded. */
export class AppMetadata {
  private values: Values = {};
  private native?: NativeStore;
  private queue = Promise.resolve();
  private failure: unknown;
  private pending: Values = {};
  private browser: () => Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;
  private onFailure: () => void;
  constructor(browser: () => Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>,
    onFailure: () => void = () => {}) {
    this.browser = browser;
    this.onFailure = onFailure;
  }

  async initialize(native: NativeStore) {
    const saved = await native.load(); // A read error must never become an empty database.
    const merged: Values = {};
    for (const key of KEYS) {
      let legacy: string | null = null;
      try { legacy = this.browser().getItem(key); } catch { /* WebView storage may be unavailable. */ }
      const value = saved[key] ?? legacy;
      if (value !== null && value !== undefined) merged[key] = value;
    }
    await native.save(merged, true);
    // Read the validated projection: strips legacy keyfile/password properties.
    this.values = await native.load();
    this.native = native;
    for (const key of KEYS) {
      try {
        if (this.values[key] !== undefined) this.browser().setItem(key, this.values[key]);
        else this.browser().removeItem(key);
      } catch { /* Native metadata remains authoritative. */ }
    }
  }

  getItem(key: string): string | null {
    this.requireKey(key);
    if (this.native) return this.values[key] ?? null;
    try { return this.browser().getItem(key); } catch { return null; }
  }
  setItem(key: string, value: string) {
    this.requireKey(key);
    if (!this.native) { this.browser().setItem(key, value); return; }
    this.values[key] = value;
    try { this.browser().setItem(key, value); } catch { /* Native write below still runs. */ }
    this.persist(key);
  }
  removeItem(key: string) {
    this.requireKey(key);
    if (!this.native) { this.browser().removeItem(key); return; }
    // Explicit reset values prevent migration from reviving stale browser data.
    const reset: Values = { 'yntra-vault-setup-completed': 'false', 'yntra-vault-recent-vaults': '[]',
      'yntra-vault-settings': '{}', 'yntra-vault-theme': 'system' };
    this.values[key] = reset[key];
    try { this.browser().removeItem(key); } catch { /* Native write below still runs. */ }
    this.persist(key);
  }
  private requireKey(key: string) {
    if (!(KEYS as readonly string[]).includes(key)) throw new Error('Unsupported application metadata key');
  }
  private persist(key: string) {
    this.pending[key] = this.values[key];
    this.queue = this.queue.then(async () => {
      const snapshot = { ...this.pending };
      if (Object.keys(snapshot).length === 0) return;
      try {
        await this.native!.save(snapshot);
        for (const [key, value] of Object.entries(snapshot)) {
          if (this.pending[key] === value) delete this.pending[key];
        }
        this.failure = undefined;
      }
      catch (error) { this.failure = error; this.onFailure(); }
    });
  }
  async flush() {
    await this.queue;
    // An explicit retry of the update also retries a previous disk-full/write error.
    const retryKey = Object.keys(this.pending)[0];
    if (this.failure && retryKey) {
      this.persist(retryKey);
      await this.queue;
    }
    if (this.failure) throw new Error('App settings could not be saved. Retry saving before updating.');
  }
}

export const appMetadata = new AppMetadata(() => localStorage, () => {
  window.dispatchEvent(new Event('yntra-metadata-save-failed'));
});
export async function initializeAppMetadata() {
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
    await appMetadata.initialize({
      load: () => invokeIpc('load_ui_metadata'),
      save: (values, initialize) => invokeIpc('save_ui_metadata', { values, initialize }),
    });
  }
}
