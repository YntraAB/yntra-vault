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

function loadPersistedFavicons(): Map<string, string> {
  const map = new Map<string, string>();
  if (typeof window === 'undefined' || !window.localStorage) return map;
  try {
    const raw = localStorage.getItem(LOCAL_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (parsed && typeof parsed === 'object') {
        for (const [k, v] of Object.entries(parsed)) {
          if (typeof v === 'string' && v.startsWith('data:')) {
            map.set(k, v);
          }
        }
      }
    }
  } catch {
    // Ignore parse or storage access errors
  }
  return map;
}

const faviconCache = loadPersistedFavicons();
const inFlightRequests = new Map<string, Promise<string | null>>();
const failedCooldowns = new Map<string, number>();
const updateListeners = new Set<() => void>();

let saveTimer: ReturnType<typeof setTimeout> | null = null;

function schedulePersist() {
  if (typeof window === 'undefined' || !window.localStorage) return;
  if (saveTimer) return;
  saveTimer = setTimeout(() => {
    saveTimer = null;
    try {
      const entries = Array.from(faviconCache.entries()).slice(-250);
      localStorage.setItem(LOCAL_STORAGE_KEY, JSON.stringify(Object.fromEntries(entries)));
    } catch {
      // Ignore quota exceptions
    }
  }, 1000);
}

interface QueueItem {
  domain: string;
  backend: { getFavicon: (d: string) => Promise<string | null> };
  resolve: (val: string | null) => void;
}

const requestQueue: QueueItem[] = [];
let activeFetches = 0;

function processQueue() {
  while (activeFetches < MAX_CONCURRENT_FETCHES && requestQueue.length > 0) {
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

  const promise = new Promise<string | null>((resolve) => {
    requestQueue.push({
      domain,
      backend,
      resolve: (res) => {
        inFlightRequests.delete(domain);
        if (res && res.startsWith('data:')) {
          faviconCache.set(domain, res);
          failedCooldowns.delete(domain);
          schedulePersist();
          updateListeners.forEach((fn) => fn());
        } else {
          failedCooldowns.set(domain, Date.now() + COOLDOWN_MS);
        }
        resolve(res);
      },
    });
    processQueue();
  });

  inFlightRequests.set(domain, promise);
  return promise;
}

export function clearFaviconCache() {
  faviconCache.clear();
  failedCooldowns.clear();
  if (typeof window !== 'undefined' && window.localStorage) {
    try {
      localStorage.removeItem(LOCAL_STORAGE_KEY);
    } catch {}
  }
  updateListeners.forEach((fn) => fn());
}

export function resetFaviconCooldowns() {
  failedCooldowns.clear();
  updateListeners.forEach((fn) => fn());
}

if (typeof window !== 'undefined') {
  window.addEventListener('online', resetFaviconCooldowns);
  window.addEventListener('yntra-favicons-cleared', clearFaviconCache);
  window.addEventListener('yntra-favicons-reset', resetFaviconCooldowns);
}

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
  const { settings } = useSettings();
  const isEnabled = settings.externalFaviconsEnabled !== false;
  const domain = extractDomain(url, title);

  const [imgUrl, setImgUrl] = useState<string | null>(() => {
    if (!domain || !isEnabled) return null;
    return faviconCache.get(domain) ?? null;
  });

  useEffect(() => {
    if (!domain || !isEnabled) {
      setImgUrl(null);
      return;
    }

    const cached = faviconCache.get(domain);
    if (cached) {
      setImgUrl(cached);
      return;
    }

    const cooldownUntil = failedCooldowns.get(domain);
    if (cooldownUntil && Date.now() < cooldownUntil) {
      setImgUrl(null);
      return;
    }

    if (!backend) return;

    let isMounted = true;
    enqueueFaviconFetch(domain, backend).then((res) => {
      if (isMounted) {
        setImgUrl(res);
      }
    });

    return () => {
      isMounted = false;
    };
  }, [domain, backend, isEnabled]);

  useEffect(() => {
    if (!domain || !isEnabled) return;
    let isMounted = true;
    const onUpdate = () => {
      if (!isMounted) return;
      const cached = faviconCache.get(domain) ?? null;
      if (cached) {
        setImgUrl(cached);
      } else if (backend) {
        const cooldownUntil = failedCooldowns.get(domain);
        if (!cooldownUntil || Date.now() >= cooldownUntil) {
          enqueueFaviconFetch(domain, backend).then((res) => {
            if (isMounted) {
              setImgUrl(res);
            }
          });
        }
      }
    };
    updateListeners.add(onUpdate);
    return () => {
      isMounted = false;
      updateListeners.delete(onUpdate);
    };
  }, [domain, backend, isEnabled]);

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
              schedulePersist();
            }
            setImgUrl(null);
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
