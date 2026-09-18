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

export { formatBytes } from './formatters';

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

/**
 * Determines whether a string target represents a desktop application path or application protocol,
 * rather than a web URL or domain.
 */
export function isAppPath(target?: string | null): boolean {
  if (!target || !target.trim()) return false;
  const clean = target.trim();

  // Explicit web schemes are always web URLs, never application paths
  if (/^https?:\/\//i.test(clean)) {
    return false;
  }

  // Custom application protocol schemes (e.g. steam://, discord://, spotify://)
  if (/^[a-zA-Z0-9_-]+:\/\//i.test(clean)) {
    return !/^(https?|ftp|file|javascript|data|blob):\/\//i.test(clean);
  }

  // Local Windows file/drive paths or UNC network shares
  if (/^[a-zA-Z]:[\\/]|^\\\\/i.test(clean)) {
    return true;
  }

  // Executable file extensions (Windows & cross-platform)
  if (/\.(exe|bat|cmd|msi|lnk)$/i.test(clean)) {
    return true;
  }

  // Unix/macOS application bundles and binary paths
  if (clean.startsWith('/') && (clean.includes('.app') || /^\/(Applications|usr|opt|bin|sbin)/i.test(clean))) {
    return true;
  }

  return false;
}

/**
 * Determines whether a string target represents a web URL or domain.
 */
export function isWebUrl(target?: string | null): boolean {
  if (!target || !target.trim()) return false;
  return !isAppPath(target);
}

export function getDomain(url: string): string | null {
  if (!url || isAppPath(url)) return null;
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

export function deriveTitle(title: string, url?: string, email?: string, username?: string): string {
  if (title && title.trim().length > 0) {
    return title.trim();
  }
  if (url && isAppPath(url)) {
    const clean = url.trim();
    if (/^[a-zA-Z0-9_-]+:\/\//.test(clean)) {
      const scheme = clean.split('://')[0];
      return scheme.charAt(0).toUpperCase() + scheme.slice(1);
    }
    const name = clean.split(/[\\/]/).pop()?.replace(/\.(exe|app|bat|cmd|msi|lnk)$/i, '');
    if (name) {
      return name.charAt(0).toUpperCase() + name.slice(1);
    }
  }
  const domain = getDomain(url || '');
  if (domain) {
    const parts = domain.split('.');
    const tldList = ['com', 'org', 'net', 'io', 'co', 'uk', 'cz', 'de', 'fr', 'app', 'dev', 'ai', 'me', 'edu', 'gov'];
    const nonTldParts = parts.filter(p => !tldList.includes(p));
    const mainName = nonTldParts[nonTldParts.length - 1] || parts[0] || domain;
    return mainName.charAt(0).toUpperCase() + mainName.slice(1);
  }
  if (email && email.trim().length > 0) {
    return email.trim();
  }
  if (username && username.trim().length > 0) {
    return username.trim();
  }
  return 'New Entry';
}

interface MiniCustomField {
  id: string;
  name: string;
  value: string;
}

export function getFieldLayout(customFields: MiniCustomField[] = [], activeStandardFields: string[]): string[] {
  const layoutCf = customFields.find(cf => cf.name === '_field_order');
  const activeCustomFieldIds = customFields.filter(cf => cf.name !== '_field_order' && Boolean(cf.id)).map(cf => cf.id);
  const validActiveStandard = activeStandardFields.filter(Boolean);
  const allActive = Array.from(new Set([...validActiveStandard, ...activeCustomFieldIds]));

  if (layoutCf && layoutCf.value) {
    const savedOrder = layoutCf.value.split(',').map(s => s.trim()).filter(Boolean);
    const ordered = Array.from(new Set(savedOrder.filter(id => allActive.includes(id))));
    const missing = allActive.filter(id => !ordered.includes(id));
    return Array.from(new Set([...ordered, ...missing]));
  }
  
  return allActive;
}
export async function openExternalUrl(target: string): Promise<void> {
  if (!target || !target.trim()) return;
  const cleanTarget = target.trim();

  // If it's an application path or custom protocol (e.g. steam://, spotify://, C:\Program Files\...)
  if (isAppPath(cleanTarget)) {
    try {
      const { open } = await import('@tauri-apps/plugin-shell');
      await open(cleanTarget);
    } catch {
      // Ignore failure if running outside desktop shell
    }
    return;
  }

  // Reject local file/drive paths, UNC network shares, and root paths
  if (
    /^[a-zA-Z]:[\\/]/.test(cleanTarget) ||
    cleanTarget.startsWith('\\\\') ||
    cleanTarget.startsWith('/')
  ) {
    return;
  }

  let formatted = cleanTarget;
  if (!/^https?:\/\//i.test(cleanTarget)) {
    // Reject any dangerous custom protocol schemes (e.g. file:, javascript:, ms-msdt:)
    if (/^[a-z0-9+.-]+:/i.test(cleanTarget)) {
      return;
    }
    formatted = `https://${cleanTarget}`;
  }

  try {
    const parsed = new URL(formatted);
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
      return;
    }
    const { open } = await import('@tauri-apps/plugin-shell');
    await open(formatted);
  } catch {
    try {
      window.open(formatted, '_blank', 'noopener,noreferrer');
    } catch {
      // Ignore failure
    }
  }
}

