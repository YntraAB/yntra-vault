import { describe, expect, it } from 'bun:test';
import { AppMetadata } from './appMetadata';

function browserStore(initial: Record<string, string> = {}) {
  const values = new Map(Object.entries(initial));
  return { getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => { values.set(key, value); },
    removeItem: (key: string) => { values.delete(key); } };
}
const recent = 'yntra-vault-recent-vaults';
const settings = 'yntra-vault-settings';

describe('application metadata across updates', () => {
  it('boots a clean install and restores the first saved preferences after restart', async () => {
    let disk: Record<string, string> = {};
    const native = { load: async () => ({ ...disk }), save: async (values: Record<string, string>) => { disk = { ...disk, ...values }; } };
    const first = new AppMetadata(() => browserStore());
    await first.initialize(native);
    expect(first.getItem(recent)).toBeNull();
    first.setItem('yntra-vault-setup-completed', 'true');
    first.setItem('yntra-vault-theme', 'dark');
    await first.flush();
    const restarted = new AppMetadata(() => browserStore());
    await restarted.initialize(native);
    expect(restarted.getItem('yntra-vault-setup-completed')).toBe('true');
    expect(restarted.getItem('yntra-vault-theme')).toBe('dark');
  });
  it('migrates old paths/preferences and restores them with an empty WebView', async () => {
    let disk: Record<string, string> = {};
    const native = { load: async () => ({ ...disk }), save: async (value: Record<string, string>) => { disk = { ...disk, ...value }; } };
    const first = new AppMetadata(() => browserStore({ [recent]: '[{"id":"1","name":"Vault","path":"E:\\\\vault.vdb"}]', [settings]: '{"language":"sv"}', 'yntra-vault-keyfiles': 'private.key' }));
    await first.initialize(native);
    expect(disk['yntra-vault-keyfiles']).toBeUndefined();
    const next = new AppMetadata(() => browserStore());
    await next.initialize(native);
    expect(next.getItem(recent)).toBe(first.getItem(recent));
    expect(next.getItem(settings)).toBe('{"language":"sv"}');
    expect(() => next.setItem('password', 'secret')).toThrow();
  });

  it('serializes changes and never resurrects removed vaults from an older WebView', async () => {
    let disk: Record<string, string> = { [recent]: '[{"path":"old"}]' };
    const browser = browserStore();
    const native = { load: async () => ({ ...disk }), save: async (value: Record<string, string>) => {
      await new Promise(resolve => setTimeout(resolve, 2)); disk = { ...disk, ...value };
    } };
    const metadata = new AppMetadata(() => browser);
    await metadata.initialize(native);
    metadata.setItem(recent, '[{"path":"new"}]');
    metadata.setItem(recent, '[]');
    await metadata.flush();
    const next = new AppMetadata(() => browserStore({ [recent]: '[{"path":"old"}]' }));
    await next.initialize(native);
    expect(next.getItem(recent)).toBe('[]');
  });

  it('does not overwrite native data after a failed read and blocks updates after failed writes', async () => {
    let writes = 0;
    const metadata = new AppMetadata(() => browserStore());
    await expect(metadata.initialize({ load: async () => { throw new Error('disk error'); }, save: async () => { writes++; } })).rejects.toThrow();
    expect(writes).toBe(0);
    let fail = false;
    let notified = false;
    const active = new AppMetadata(() => browserStore(), () => { notified = true; });
    await active.initialize({ load: async () => ({}), save: async () => { if (fail) throw new Error('full'); } });
    fail = true;
    active.setItem(settings, '{}');
    await expect(active.flush()).rejects.toThrow('could not be saved');
    expect(notified).toBe(true);
    fail = false;
    await active.flush();
  });

  it('retries all unsaved keys without reverting other instances metadata', async () => {
    let disk: Record<string, string> = {};
    let fail = false;
    const metadata = new AppMetadata(() => browserStore());
    await metadata.initialize({ load: async () => ({ ...disk }), save: async values => {
      if (fail) throw new Error('disk full');
      disk = { ...disk, ...values };
    } });
    fail = true;
    metadata.setItem(settings, '{"language":"sv"}');
    await expect(metadata.flush()).rejects.toThrow();
    disk['yntra-vault-theme'] = 'dark'; // A different app instance updated this key.
    fail = false;
    metadata.setItem(recent, '[]');
    await metadata.flush();
    expect(disk[settings]).toBe('{"language":"sv"}');
    expect(disk[recent]).toBe('[]');
    expect(disk['yntra-vault-theme']).toBe('dark');
  });

  it('keeps working when browser storage is unavailable', async () => {
    const metadata = new AppMetadata(() => { throw new Error('blocked'); });
    let disk: Record<string, string> = { [settings]: '{"language":"sv"}' };
    await metadata.initialize({ load: async () => disk, save: async values => { disk = { ...disk, ...values }; } });
    metadata.setItem(recent, '[]');
    await metadata.flush();
    expect(disk[recent]).toBe('[]');
    expect(metadata.getItem(settings)).toBe('{"language":"sv"}');
  });
});
