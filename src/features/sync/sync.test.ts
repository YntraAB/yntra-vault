import { describe, it, expect } from 'bun:test';
import { BRANDS } from './brands';

describe('Sync Feature Slice', () => {
  describe('Competitor Brands Configuration', () => {
    it('contains auto-detect as the first recommended choice', () => {
      expect(BRANDS.length).toBeGreaterThan(0);
      expect(BRANDS[0].id).toBe('auto_detect');
      expect(BRANDS[0].badge).toBe('Recommended');
      expect(BRANDS[0].supportedFormatKey).toBe('auto');
    });

    it('contains configurations for major password managers', () => {
      const brandIds = BRANDS.map((b) => b.id);
      expect(brandIds).toContain('bitwarden');
      expect(brandIds).toContain('onepassword');
      expect(brandIds).toContain('keepass');
      expect(brandIds).toContain('chrome');
      expect(brandIds).toContain('lastpass');
      expect(brandIds).toContain('dashlane');
      expect(brandIds).toContain('protonpass');
      expect(brandIds).toContain('generic');
    });

    it('defines export instructions for each brand', () => {
      for (const brand of BRANDS) {
        expect(brand.instructions.length).toBeGreaterThan(0);
        expect(brand.supportedFormatKey).toBeTruthy();
      }
    });
  });
});
