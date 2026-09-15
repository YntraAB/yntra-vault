import React from 'react';
import { Info } from 'lucide-react';
import { ActionTooltip } from '@/components/ui/tooltip';

export interface SettingSectionProps {
  label: string;
  tooltip?: string;
  children: React.ReactNode;
}

export function SettingSection({ label, tooltip, children }: SettingSectionProps) {
  return (
    <div className="border-b border-[var(--border-subtle)] pb-5 select-none">
      <h3 className="mb-3 text-[11px] font-semibold uppercase tracking-[0.04em] text-[var(--text-tertiary)] flex items-center">
        <span>{label}</span>
        {tooltip && (
          <ActionTooltip content={tooltip}>
            <Info size={12} className="ml-1.5 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors cursor-help" />
          </ActionTooltip>
        )}
      </h3>
      {children}
    </div>
  );
}

export interface SettingRowProps {
  label: string;
  description?: string;
  tooltip?: string;
  children: React.ReactNode;
}

export function SettingRow({
  label,
  description,
  tooltip,
  children,
}: SettingRowProps) {
  return (
    <div className="flex items-center justify-between gap-6 border-b border-[var(--border-subtle)] py-3 select-none">
      <div className="min-w-0 flex-1">
        <div className="text-[13px] text-[var(--text-primary)] flex items-center">
          <span>{label}</span>
          {tooltip && (
            <ActionTooltip content={tooltip}>
              <Info size={12} className="ml-1.5 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors cursor-help shrink-0" />
            </ActionTooltip>
          )}
        </div>
        {description && (
          <div className="mt-0.5 text-[12px] text-[var(--text-secondary)] leading-relaxed">{description}</div>
        )}
      </div>
      <div className="shrink-0 flex items-center justify-end">
        {children}
      </div>
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
      className={`shrink-0 relative h-5 w-9 rounded-full transition-colors cursor-pointer select-none ${
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
