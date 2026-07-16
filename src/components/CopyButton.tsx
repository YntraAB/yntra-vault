import { useState, useCallback } from 'react';
import { Copy, Check } from 'lucide-react';

interface CopyButtonProps {
  value: string;
  className?: string;
  size?: number;
}

export default function CopyButton({ value, className = '', size = 14 }: CopyButtonProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      navigator.clipboard.writeText(value).catch(() => {});
      setCopied(true);
      setTimeout(() => setCopied(false), 800);
    },
    [value]
  );

  return (
    <button
      onClick={handleCopy}
      className={`inline-flex items-center justify-center rounded-[3px] p-1 text-[var(--text-tertiary)] transition-all duration-100 hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)] active:scale-95 ${className}`}
      title="Copy"
    >
      {copied ? (
        <Check size={size} className="text-[var(--success)]" />
      ) : (
        <Copy size={size} />
      )}
    </button>
  );
}



