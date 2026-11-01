import { getDomain } from '@/lib/utils';


export interface FaviconProps {
  url?: string;
  title: string;
  color?: string;
  sizeClass?: string;
  textClass?: string;
}

export const LOCAL_STORAGE_KEY = 'yntra-favicons-cache';

export const COOLDOWN_MS = 30_000;

export const MAX_CONCURRENT_FETCHES = 4;

export function purgeLegacyFaviconCache() {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.removeItem(LOCAL_STORAGE_KEY);
  } catch {
    // Browser storage may be unavailable. Never load or write cached account domains.
  }
}


purgeLegacyFaviconCache();

export const faviconCache = new Map<string, string>();

export const inFlightRequests = new Map<string, Promise<string | null>>();

export const failedCooldowns = new Map<string, number>();

export const updateListeners = new Set<() => void>();

export interface QueueItem {
  domain: string;
  backend: { getFavicon: (d: string) => Promise<string | null> };
  resolve: (val: string | null) => void;
}

export const requestQueue: QueueItem[] = [];

export let activeFetches = 0;

export let cacheGeneration = 0;

export let sessionLocked = false;

export function processQueue() {
  while (!sessionLocked && activeFetches < MAX_CONCURRENT_FETCHES && requestQueue.length > 0) {
    const item = requestQueue.shift();
    if (!item) break;
    activeFetches++;

    item.backend
      .getFavicon(item.domain)
      .then((res) => {
        item.resolve(res);
      })
      .catch(() => {
        item.resolve(null);
      })
      .finally(() => {
        activeFetches--;
        processQueue();
      });
  }
}

export function enqueueFaviconFetch(
  domain: string,
  backend: { getFavicon: (d: string) => Promise<string | null> }
): Promise<string | null> {
  const existing = inFlightRequests.get(domain);
  if (existing) return existing;

  const generation = cacheGeneration;
  const promise = new Promise<string | null>((resolve) => {
    requestQueue.push({
      domain,
      backend,
      resolve: (res) => {
        if (generation !== cacheGeneration) { resolve(null); return; }
        inFlightRequests.delete(domain);
        if (res && res.startsWith('data:image/')) {
          faviconCache.set(domain, res);
          if (faviconCache.size > 250) faviconCache.delete(faviconCache.keys().next().value!);
          failedCooldowns.delete(domain);
          updateListeners.forEach((fn) => fn());
        } else {
          failedCooldowns.set(domain, Date.now() + COOLDOWN_MS);
        }
        resolve(res?.startsWith('data:image/') ? res : null);
      },
    });
    processQueue();
  });

  inFlightRequests.set(domain, promise);
  return promise;
}


export function clearFaviconCache() {
  cacheGeneration++;
  inFlightRequests.clear();
  for (const item of requestQueue.splice(0)) item.resolve(null);
  faviconCache.clear();
  failedCooldowns.clear();
  purgeLegacyFaviconCache();
  updateListeners.forEach((fn) => fn());
}


export function resetFaviconCooldowns() {
  failedCooldowns.clear();
  updateListeners.forEach((fn) => fn());
}

export const lockFavicons = () => { sessionLocked = true; clearFaviconCache(); };

export const unlockFavicons = () => { sessionLocked = false; resetFaviconCooldowns(); };

export let eventWindow: Window | undefined;

export function ensureFaviconListeners() {
  if (typeof window === 'undefined' || eventWindow === window) return;
  const handlers = [
    ['online', resetFaviconCooldowns], ['yntra-favicons-cleared', clearFaviconCache],
    ['yntra-favicons-reset', resetFaviconCooldowns], ['yntra-session-locked', lockFavicons],
    ['yntra-session-unlocked', unlockFavicons],
  ] as const;
  for (const [event, handler] of handlers) {
    eventWindow?.removeEventListener(event, handler);
    window.addEventListener(event, handler);
  }
  eventWindow = window;
  sessionLocked = false;
  purgeLegacyFaviconCache();
}

ensureFaviconListeners();


export function extractDomain(url?: string, title?: string): string | null {
  if (url) {
    const d = getDomain(url);
    if (d && d.includes('.')) return d;
  }
  if (title) {
    const d = getDomain(title);
    if (d && d.includes('.')) return d;
  }
  return null;
}