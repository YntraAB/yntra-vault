import { type ClassValue, clsx } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function getInitials(title: string): string {
  return title
    .split(/\s+/)
    .map((w) => w[0])
    .join('')
    .toUpperCase()
    .slice(0, 2);
}

export function formatDate(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
}

export function formatTime(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
}

export function isToday(iso: string): boolean {
  const d = new Date(iso);
  const now = new Date();
  return d.toDateString() === now.toDateString();
}

export function isYesterday(iso: string): boolean {
  const d = new Date(iso);
  const yesterday = new Date();
  yesterday.setDate(yesterday.getDate() - 1);
  return d.toDateString() === yesterday.toDateString();
}

export function maskPassword(pw: string): string {
  return '\u2022'.repeat(Math.min(pw.length, 16));
}

// Simple TOTP generation for demo
export function generateTOTP(_secret: string, period: number = 30, digits: number = 6): string {
  // This is a simplified mock - real implementation needs proper base32 decode + HMAC
  const now = Math.floor(Date.now() / 1000);
  const counter = Math.floor(now / period);
  // Deterministic pseudo-random based on counter for demo
  const hash = counter.toString(16).padStart(16, '0');
  let code = '';
  for (let i = 0; i < digits; i++) {
    code += parseInt(hash[i % hash.length], 16) % 10;
  }
  return code;
}

export function getTOTPRemainingSeconds(period: number = 30): number {
  const now = Math.floor(Date.now() / 1000);
  const remaining = period - (now % period);
  return remaining === period ? period : remaining;
}

export function getDomain(url: string): string | null {
  if (!url) return null;
  let clean = url.trim().toLowerCase();
  if (!/^https?:\/\//i.test(clean)) {
    clean = 'https://' + clean;
  }
  try {
    const parsed = new URL(clean);
    return parsed.hostname.replace(/^www\./, '');
  } catch {
    return null;
  }
}

interface MiniCustomField {
  id: string;
  name: string;
  value: string;
}

export function getFieldLayout(customFields: MiniCustomField[] = [], activeStandardFields: string[]): string[] {
  const layoutCf = customFields.find(cf => cf.name === '_field_order');
  const activeCustomFieldIds = customFields.filter(cf => cf.name !== '_field_order').map(cf => cf.id);
  const allActive = [...activeStandardFields, ...activeCustomFieldIds];

  if (layoutCf && layoutCf.value) {
    const savedOrder = layoutCf.value.split(',');
    const ordered = savedOrder.filter(id => allActive.includes(id));
    const missing = allActive.filter(id => !ordered.includes(id));
    return [...ordered, ...missing];
  }
  
  return allActive;
}


