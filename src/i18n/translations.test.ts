import { describe, it, expect } from 'bun:test';
import { loadTranslation, isTranslationLoaded, getTranslation, translations } from './translations';

describe('i18n Dynamic Loader & Translations', () => {
  it('loads English translation chunk dynamically', async () => {
    const dict = await loadTranslation('en');
    expect(dict).toBeDefined();
    expect(dict['app.name']).toBe('Yntra Vault');
    expect(isTranslationLoaded('en')).toBe(true);
  });

  it('loads other language chunks on demand', async () => {
    const esDict = await loadTranslation('es');
    expect(esDict).toBeDefined();
    expect(esDict['common.save']).toBe('Guardar Cambios');
    expect(isTranslationLoaded('es')).toBe(true);

    const zhDict = await loadTranslation('zh-CN');
    expect(zhDict).toBeDefined();
    expect(zhDict['common.save']).toBe('保存更改');
    expect(isTranslationLoaded('zh-CN')).toBe(true);
  });

  it('reuses cached dictionary on subsequent calls', async () => {
    const firstCall = await loadTranslation('de');
    const secondCall = await loadTranslation('de');
    expect(firstCall).toBe(secondCall);
    expect(translations['de']).toBe(firstCall);
  });

  it('interpolates parameters correctly with whitespace tolerance', async () => {
    await loadTranslation('en');
    const res = getTranslation('en', 'toast.export_failed', { err: 'Disk Full' });
    expect(res).toBe('Export failed: Disk Full');
  });

  it('falls back to English when key is missing in target language', async () => {
    await loadTranslation('en');
    // Inject mock translation to test fallback
    translations['mock-lang'] = { 'existing.key': 'Bonjour' };
    const res = getTranslation('mock-lang', 'app.name');
    expect(res).toBe('Yntra Vault');
    delete translations['mock-lang'];
  });

  it('returns raw key if missing in both target and English dictionaries', async () => {
    const res = getTranslation('en', 'completely.nonexistent.key');
    expect(res).toBe('completely.nonexistent.key');
  });

  it('handles unknown language codes safely by falling back to English', async () => {
    const dict = await loadTranslation('invalid-lang-code');
    expect(dict).toBeDefined();
    expect(dict['app.name']).toBe('Yntra Vault');
  });

  it('loads all 24 supported languages correctly with valid dictionaries', async () => {
    const { LANGUAGES } = await import('./languages');
    for (const lang of LANGUAGES) {
      const dict = await loadTranslation(lang.code);
      expect(dict).toBeDefined();
      expect(Object.keys(dict).length).toBeGreaterThan(100);
      expect(isTranslationLoaded(lang.code)).toBe(true);
      const saveText = getTranslation(lang.code, 'common.save');
      expect(saveText).toBeTruthy();
    }
  });
});
