import React from 'react';

export function SettingSection({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="border-b border-[var(--border-subtle)] pb-5">
      <h3 className="mb-3 text-[11px] font-semibold uppercase tracking-[0.04em] text-[var(--text-tertiary)]">
        {label}
      </h3>
      {children}
    </div>
  );
}

export function SettingRow({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between border-b border-[var(--border-subtle)] py-3">
      <div>
        <div className="text-[13px] text-[var(--text-primary)]">{label}</div>
        {description && (
          <div className="mt-0.5 text-[12px] text-[var(--text-secondary)]">{description}</div>
        )}
      </div>
      {children}
    </div>
  );
}

export function Toggle({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      type="button"
      onClick={() => onChange(!checked)}
      role="switch"
      aria-checked={checked}
      className={`relative h-5 w-9 rounded-full transition-colors ${
        checked ? 'bg-[var(--text-primary)]' : 'bg-[var(--border)]'
      }`}
    >
      <div
        className={`absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white transition-transform ${
          checked ? 'translate-x-4' : 'translate-x-0'
        }`}
      />
    </button>
  );
}
