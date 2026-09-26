import { describe, it, expect } from 'bun:test';
import { formatShortcut, getKeybinds, matchesShortcut } from './keybinds';
import { getTranslation, loadTranslation } from '@/i18n/translations';

describe('configured search shortcut', () => {
  it('interpolates the configured combination in both primary languages', async () => {
    const shortcut = getKeybinds({ search: { altKey: true, shiftKey: true, key: 'f' } }).search;
    for (const language of ['en', 'sv']) {
      await loadTranslation(language);
      const text = getTranslation(language, 'app.search_placeholder', { shortcut: formatShortcut(shortcut) });
      expect(text).toContain('Alt+Shift+F');
      expect(text).not.toContain('Ctrl+K');
    }
  });

  it('requires Meta when a Meta-only binding is configured', () => {
    const event = { key: 'f', code: 'KeyF', metaKey: true, ctrlKey: false, altKey: false, shiftKey: false } as KeyboardEvent;
    expect(matchesShortcut(event, { metaKey: true, key: 'f' })).toBe(true);
    expect(matchesShortcut({ ...event, metaKey: false } as KeyboardEvent, { metaKey: true, key: 'f' })).toBe(false);
  });
});
