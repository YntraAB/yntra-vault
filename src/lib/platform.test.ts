import { describe, it, expect } from 'bun:test';
import { capabilitiesFor } from './platform';
import { TauriBackend } from './tauri-backend';
describe('runtime capabilities', () => {
  it('keeps desktop automation off phones and USB enrollment on Windows only', () => {
    for (const os of ['android','ios','unknown'] as const) {
      expect(capabilitiesFor(os).automation).toBe(false);
      expect(capabilitiesFor(os).usbBinding).toBe(false);
      expect(capabilitiesFor(os).nativeAuthentication).toBe(false);
    }
    expect(capabilitiesFor('windows').automation).toBe(true);
    expect(capabilitiesFor('windows').usbBinding).toBe(true);
    expect(capabilitiesFor('linux').usbBinding).toBe(false);
    expect(capabilitiesFor('android').captureProtection).toBe(true);
  });
  it('propagates a native clipboard failure without falling back to an unprotected copy', async () => {
    const original = Object.getOwnPropertyDescriptor(globalThis, 'window');
    const commands: string[] = [];
    Object.defineProperty(globalThis, 'window', { configurable: true, value: { __TAURI_INTERNALS__: {
      invoke: async (command: string) => { commands.push(command); throw new Error('Native clipboard unavailable'); },
    } } });
    try {
      await expect(new TauriBackend().copyToClipboard('synthetic-test-secret', true, 30)).rejects.toThrow('Native clipboard unavailable');
      expect(commands).toEqual(['copy_to_clipboard']);
    } finally {
      if (original) Object.defineProperty(globalThis, 'window', original);
      else Reflect.deleteProperty(globalThis, 'window');
    }
  });
});
