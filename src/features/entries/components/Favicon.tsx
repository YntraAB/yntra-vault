import { useState, useEffect } from 'react';
import { getDomain, getInitials } from '@/lib/utils';
import { useBackend } from '@/lib/useBackend';
import { useSettings } from '@/features/settings';

export interface FaviconProps {
  url?: string;
  title: string;
  color?: string;
  sizeClass?: string;
  textClass?: string;
}

const LOCAL_STORAGE_KEY = 'yntra-favicons-cache';
const COOLDOWN_MS = 30_000;
const MAX_CONCURRENT_FETCHES = 4;

function purgeLegacyFaviconCache() {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.removeItem(LOCAL_STORAGE_KEY);
  } catch {
    // Browser storage may be unavailable. Never load or write cached account domains.
  }
}

purgeLegacyFaviconCache();
const faviconCache = new Map<string, string>();
const inFlightRequests = new Map<string, Promise<string | null>>();
const failedCooldowns = new Map<string, number>();
const updateListeners = new Set<() => void>();

interface QueueItem {
  domain: string;
  backend: { getFavicon: (d: string) => Promise<string | null> };
  resolve: (val: string | null) => void;
}

const requestQueue: QueueItem[] = [];
let activeFetches = 0;
let cacheGeneration = 0;
let sessionLocked = false;

function processQueue() {
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

function enqueueFaviconFetch(
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

const lockFavicons = () => { sessionLocked = true; clearFaviconCache(); };
const unlockFavicons = () => { sessionLocked = false; resetFaviconCooldowns(); };
let eventWindow: Window | undefined;
function ensureFaviconListeners() {
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

export function Favicon({
  url = '',
  title,
  color = 'var(--border)',
  sizeClass = 'h-7 w-7',
  textClass = 'text-[11px]',
}: FaviconProps) {
  const { backend } = useBackend();
  const { settings, externalFaviconsReady } = useSettings();
  const isEnabled = settings.operationMode !== 'airgap' && settings.externalFaviconsEnabled === true && externalFaviconsReady;
  const domain = extractDomain(url, title);

  const [image, setImage] = useState<{ domain: string; url: string } | null>(null);
  const [retry, setRetry] = useState(0);
  const imgUrl = isEnabled && image?.domain === domain ? image.url : null;

  useEffect(() => {
    ensureFaviconListeners();
    if (!domain || !isEnabled || !backend) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const load = () => {
      clearTimeout(timer);
      if (sessionLocked) { setImage(null); return; }
      const cached = faviconCache.get(domain);
      if (cached) {
        setImage({ domain, url: cached });
        return;
      }
      setImage(null);
      if (typeof navigator !== 'undefined' && navigator.onLine === false) return;
      const remaining = (failedCooldowns.get(domain) ?? 0) - Date.now();
      if (remaining > 0) {
        timer = setTimeout(load, remaining + 1);
        return;
      }
      enqueueFaviconFetch(domain, backend).then(res => {
        if (cancelled) return;
        if (res) setImage({ domain, url: res });
        else timer = setTimeout(load, COOLDOWN_MS);
      });
    };
    updateListeners.add(load);
    load();
    return () => {
      cancelled = true;
      clearTimeout(timer);
      updateListeners.delete(load);
    };
  }, [domain, backend, isEnabled, retry]);
  if (imgUrl) {
    return (
      <div
        className={`relative shrink-0 aspect-square flex items-center justify-center rounded-[4px] bg-[var(--bg-elevated)] border border-[var(--border-subtle)] overflow-hidden ${sizeClass}`}
      >
        <img
          src={imgUrl}
          alt={title}
          onError={() => {
            if (domain) {
              faviconCache.delete(domain);
              failedCooldowns.set(domain, Date.now() + COOLDOWN_MS);
            }
            setImage(null);
            setRetry(value => value + 1);
          }}
          className="h-full w-full object-contain p-0.5"
          loading="lazy"
        />
      </div>
    );
  }

  // Fallback placeholder
  return (
    <div
      className={`flex shrink-0 aspect-square items-center justify-center rounded-[4px] font-semibold text-white uppercase select-none ${sizeClass} ${textClass}`}
      style={{ backgroundColor: color }}
    >
      {getInitials(title)}
    </div>
  );
}

export default Favicon;
