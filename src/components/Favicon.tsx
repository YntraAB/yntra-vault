import { useState, useEffect } from 'react';
import { getDomain, getInitials } from '@/lib/utils';

interface FaviconProps {
  url?: string;
  title: string;
  color?: string;
  sizeClass?: string;
  textClass?: string;
}

export default function Favicon({
  url = '',
  title,
  color = 'var(--border)',
  sizeClass = 'h-7 w-7',
  textClass = 'text-[11px]',
}: FaviconProps) {
  const [imgUrl, setImgUrl] = useState<string | null>(null);
  const [error, setError] = useState(false);

  useEffect(() => {
    const domain = getDomain(url);
    if (domain) {
      // Fetch a clean favicon with sz=64 using Google's favicon service
      setImgUrl(`https://www.google.com/s2/favicons?domain=${domain}&sz=64`);
      setError(false);
    } else {
      setImgUrl(null);
      setError(true);
    }
  }, [url]);

  if (imgUrl && !error) {
    return (
      <div className={`relative shrink-0 flex items-center justify-center rounded-[4px] bg-[var(--bg-elevated)] border border-[var(--border-subtle)] overflow-hidden ${sizeClass}`}>
        <img
          src={imgUrl}
          alt={title}
          onError={() => setError(true)}
          className="h-full w-full object-cover"
        />
      </div>
    );
  }

  // Fallback placeholder
  return (
    <div
      className={`flex shrink-0 items-center justify-center rounded-[4px] font-semibold text-white uppercase ${sizeClass} ${textClass}`}
      style={{ backgroundColor: color }}
    >
      {getInitials(title)}
    </div>
  );
}



