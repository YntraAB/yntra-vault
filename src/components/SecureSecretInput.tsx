import { useRef, useEffect, useImperativeHandle, forwardRef, useCallback } from 'react';

export interface SecureSecretInputRef {
  getSecretBytes: () => Uint8Array;
  clearSecretBytes: () => void;
  focus: () => void;
}

interface SecureSecretInputProps {
  placeholder?: string;
  className?: string;
  show?: boolean;
  onKeyDown?: (e: React.KeyboardEvent<HTMLInputElement>) => void;
  autoFocus?: boolean;
  mismatch?: boolean;
  disabled?: boolean;
}

export const SecureSecretInput = forwardRef<SecureSecretInputRef, SecureSecretInputProps>(
  ({ placeholder = '', className = '', show = false, onKeyDown, autoFocus = false, mismatch = false, disabled = false }, ref) => {
    const inputRef = useRef<HTMLInputElement>(null);
    const bufferRef = useRef<Uint8Array>(new Uint8Array(256));
    const lengthRef = useRef<number>(0);

    const clearSecretBytes = useCallback(() => {
      bufferRef.current.fill(0);
      lengthRef.current = 0;
      if (inputRef.current) {
        inputRef.current.value = '';
      }
    }, []);

    useImperativeHandle(ref, () => ({
      getSecretBytes: () => {
        return bufferRef.current.slice(0, lengthRef.current);
      },
      clearSecretBytes,
      focus: () => {
        inputRef.current?.focus();
      },
    }));

    useEffect(() => {
      if (autoFocus) {
        inputRef.current?.focus();
      }
      return () => {
        bufferRef.current.fill(0);
        lengthRef.current = 0;
      };
    }, [autoFocus]);

    const handleInput = useCallback((e: React.FormEvent<HTMLInputElement>) => {
      const val = e.currentTarget.value;
      const encoder = new TextEncoder();
      const encoded = encoder.encode(val);
      
      bufferRef.current.fill(0);
      if (encoded.length <= bufferRef.current.length) {
        bufferRef.current.set(encoded);
        lengthRef.current = encoded.length;
      } else {
        const newBuf = new Uint8Array(encoded.length + 64);
        newBuf.set(encoded);
        bufferRef.current = newBuf;
        lengthRef.current = encoded.length;
      }
      encoded.fill(0);
    }, []);

    return (
      <input
        ref={inputRef}
        type={show ? 'text' : 'password'}
        placeholder={placeholder}
        disabled={disabled}
        onInput={handleInput}
        onKeyDown={onKeyDown}
        className={`h-9 rounded-md border bg-[var(--bg-elevated)] px-3 font-mono text-[13px] tracking-wide text-[var(--text-primary)] outline-none placeholder:font-sans placeholder:tracking-normal placeholder:text-[var(--text-tertiary)] focus:border-[var(--border-focus)] disabled:opacity-50 disabled:cursor-not-allowed ${
          mismatch ? 'border-[var(--destructive)]' : 'border-[var(--border)]'
        } ${className}`}
      />
    );
  }
);

SecureSecretInput.displayName = 'SecureSecretInput';

export default SecureSecretInput;
