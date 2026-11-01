import { useSettings } from '@/features/settings';
import { useBackend } from '@/lib/useBackend';
import { getInitials } from '@/lib/utils';
import { useEffect,useState } from 'react';
import { COOLDOWN_MS,enqueueFaviconFetch,ensureFaviconListeners,extractDomain,failedCooldowns,faviconCache,type FaviconProps,sessionLocked,updateListeners } from './Favicon.model';


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