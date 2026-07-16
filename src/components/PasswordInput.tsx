import { useState } from 'react';
import { Eye, EyeOff } from 'lucide-react';
import { maskPassword } from '@/lib/utils';

interface PasswordInputProps {
  value: string;
  onChange?: (val: string) => void;
  readOnly?: boolean;
  className?: string;
}

export default function PasswordInput({ value, onChange, readOnly = false, className = '' }: PasswordInputProps) {
  const [show, setShow] = useState(false);

  if (readOnly) {
    return (
      <div className={`flex items-center gap-2 ${className}`}>
        <span className="font-mono text-[13px] tracking-wider text-[var(--text-primary)]">
          {show ? value : maskPassword(value)}
        </span>
        <button
          onClick={() => setShow(!show)}
          className="inline-flex items-center justify-center rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:text-[var(--text-primary)]"
          title={show ? 'Hide' : 'Show'}
        >
          {show ? <EyeOff size={14} /> : <Eye size={14} />}
        </button>
      </div>
    );
  }

  return (
    <div className={`relative flex items-center ${className}`}>
      <input
        type={show ? 'text' : 'password'}
        value={value}
        onChange={(e) => onChange?.(e.target.value)}
        className="h-8 w-full rounded-[3px] border border-[var(--border)] bg-[var(--bg-elevated)] px-2.5 pr-8 font-mono text-[13px] text-[var(--text-primary)] outline-none transition-colors placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)]"
      />
      <button
        type="button"
        onClick={() => setShow(!show)}
        className="absolute right-1.5 inline-flex items-center justify-center rounded-[3px] p-1 text-[var(--text-tertiary)] transition-colors hover:text-[var(--text-primary)]"
      >
        {show ? <EyeOff size={14} /> : <Eye size={14} />}
      </button>
    </div>
  );
}



