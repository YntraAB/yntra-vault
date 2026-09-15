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

const faviconCache = new Map<string, string | null>();
const inFlightRequests = new Map<string, Promise<string | null>>();

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
  const domain = getDomain(url);

  const [imgUrl, setImgUrl] = useState<string | null>(() => {
    if (!domain || !isEnabled) return null;
    return faviconCache.get(domain) ?? null;
  });

  useEffect(() => {
    if (!domain || !isEnabled) {
      setImgUrl(null);
      return;
    }

    if (faviconCache.has(domain)) {
      setImgUrl(faviconCache.get(domain) ?? null);
      return;
    }

    if (!backend) {
      return;
    }

    let isMounted = true;
    let promise = inFlightRequests.get(domain);
    if (!promise) {
      promise = backend.getFavicon(domain).catch(() => null);
      inFlightRequests.set(domain, promise);
    }

    promise.then((result) => {
      inFlightRequests.delete(domain);
      faviconCache.set(domain, result);
      if (isMounted) {
        setImgUrl(result);
      }
    });

    return () => {
      isMounted = false;
    };
  }, [domain, backend, isEnabled]);

  if (imgUrl) {
    return (
      <div className={`relative shrink-0 aspect-square flex items-center justify-center rounded-[4px] bg-[var(--bg-elevated)] border border-[var(--border-subtle)] overflow-hidden ${sizeClass}`}>
        <img
          src={imgUrl}
          alt={title}
          onError={() => {
            if (domain) faviconCache.set(domain, null);
            setImgUrl(null);
          }}
          className="h-full w-full object-contain p-0.5"
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
