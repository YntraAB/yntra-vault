import { useSyncExternalStore } from 'react';
import { invokeIpc } from '@/types/ipc';

export type RuntimePlatform = 'windows' | 'macos' | 'linux' | 'android' | 'ios' | 'unknown' | 'preview';
let platform: RuntimePlatform = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window ? 'unknown' : 'preview';
const listeners = new Set<() => void>();
const subscribe = (fn: () => void) => { listeners.add(fn); return () => { listeners.delete(fn); }; };
export async function initializePlatform() {
  const value = await invokeIpc('get_runtime_platform');
  platform = ['windows','macos','linux','android','ios'].includes(value) ? value as RuntimePlatform : 'unknown';
  listeners.forEach(fn => fn());
}
export function capabilitiesFor(os: RuntimePlatform) {
  const desktop = ['windows', 'macos', 'linux', 'preview'].includes(os);
  return { os, desktop, mobile: os === 'android' || os === 'ios',
    automation: desktop, usbBinding: os === 'windows' || os === 'preview',
    captureProtection: os === 'windows' || os === 'android' || os === 'preview',
    nativeAuthentication: desktop };
}
export function getCapabilities() { return capabilitiesFor(platform); }
export function useCapabilities() { return capabilitiesFor(useSyncExternalStore(subscribe, () => platform, () => 'preview')); }

export async function configureAutoLock(seconds: number) {
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) await invokeIpc('configure_auto_lock', { seconds });
}
export function recordUserActivity() {
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) void invokeIpc('record_user_activity').catch(() => {});
}
