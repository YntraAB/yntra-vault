import type { KeybindShortcut, KeybindsConfig } from '@/types';

export const DEFAULT_KEYBINDS: KeybindsConfig = {
  search: { ctrlKey: true, key: 'k' },
  newEntry: { ctrlKey: true, key: 'n' },
  lockVault: { ctrlKey: true, key: 'l' },
  copyPassword: { ctrlKey: true, key: 'c' },
  copyUsername: { ctrlKey: true, key: 'b' },
  copyUrl: { ctrlKey: true, key: 'u' },
  copyTotp: { ctrlKey: true, key: 't' },
  editEntry: { ctrlKey: true, key: 'e' },
  deleteEntry: { ctrlKey: true, key: 'Delete' },
  openUrl: { ctrlKey: true, key: 'o' },
  autotype: { ctrlKey: true, shiftKey: true, key: 'a' },
};

/**
  Retrieves current keybinds merged with defaults, ensuring no shortcut property is undefined.
 */
export function getKeybinds(userKeybinds?: Partial<KeybindsConfig>): KeybindsConfig {
  return {
    ...DEFAULT_KEYBINDS,
    ...(userKeybinds || {}),
  };
}

/**
  Formats a KeybindShortcut into an array of string tokens suitable for <kbd> rendering.
  e.g. { ctrlKey: true, key: 'k' } => ['Ctrl', 'K']
 */
export function formatShortcutKeys(shortcut: KeybindShortcut | undefined): string[] {
  if (!shortcut || !shortcut.key) return ['None'];
  const isMac = typeof navigator !== 'undefined' && /mac/i.test(navigator.userAgent);
  const keys: string[] = [];

  if (shortcut.ctrlKey) keys.push(isMac ? 'Ctrl' : 'Ctrl');
  if (shortcut.metaKey) keys.push(isMac ? 'Cmd' : 'Win');
  if (shortcut.altKey) keys.push(isMac ? 'Option' : 'Alt');
  if (shortcut.shiftKey) keys.push('Shift');

  let keyDisplay = shortcut.key.toUpperCase();
  if (shortcut.key === ' ') keyDisplay = 'Space';
  else if (shortcut.key === 'ArrowUp') keyDisplay = '↑';
  else if (shortcut.key === 'ArrowDown') keyDisplay = '↓';
  else if (shortcut.key === 'ArrowLeft') keyDisplay = '←';
  else if (shortcut.key === 'ArrowRight') keyDisplay = '→';

  keys.push(keyDisplay);
  return keys;
}

/**
  Formats a KeybindShortcut into a single string.
  e.g. { ctrlKey: true, key: 'k' } => "Ctrl+K"
 */
export function formatShortcut(shortcut: KeybindShortcut | undefined): string {
  return formatShortcutKeys(shortcut).join('+');
}

/**
  Checks if a DOM KeyboardEvent matches a specified KeybindShortcut configuration.
 */
export function matchesShortcut(e: KeyboardEvent, shortcut: KeybindShortcut | undefined): boolean {
  if (!shortcut || !shortcut.key) return false;

  const eventKey = e.key.toLowerCase();
  const targetKey = shortcut.key.toLowerCase();
  const codeKey = e.code.toLowerCase();

  const keyMatch =
    eventKey === targetKey ||
    codeKey === targetKey ||
    codeKey === `key${targetKey}` ||
    codeKey === `digit${targetKey}`;

  const ctrlMatch = Boolean(shortcut.ctrlKey);
  const altMatch = Boolean(shortcut.altKey);
  const shiftMatch = Boolean(shortcut.shiftKey);

  // Allow Ctrl or Cmd for cross-platform convenience
  const isControlPressed = Boolean(e.ctrlKey || e.metaKey);
  const hasControl = ctrlMatch ? isControlPressed : !e.ctrlKey && !e.metaKey;

  const hasAlt = Boolean(e.altKey) === altMatch;
  const hasShift = Boolean(e.shiftKey) === shiftMatch;

  return keyMatch && hasControl && hasAlt && hasShift;
}

/**
  Compares two KeybindShortcut objects for exact equality.
 */
export function shortcutsEqual(a?: KeybindShortcut, b?: KeybindShortcut): boolean {
  if (!a || !b) return a === b;
  return (
    Boolean(a.ctrlKey) === Boolean(b.ctrlKey) &&
    Boolean(a.altKey) === Boolean(b.altKey) &&
    Boolean(a.shiftKey) === Boolean(b.shiftKey) &&
    Boolean(a.metaKey) === Boolean(b.metaKey) &&
    a.key.toLowerCase() === b.key.toLowerCase()
  );
}
