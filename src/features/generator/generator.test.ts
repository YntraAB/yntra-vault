import { describe, it, expect } from 'bun:test';
import { getDomainName, DOMAIN_RULES } from './presets';

describe('Password Generator Slice', () => {
  describe('getDomainName parser', () => {
    it('handles empty strings gracefully', () => {
      expect(getDomainName('')).toBe('');
    });

    it('extracts domain from full https URL with path', () => {
      expect(getDomainName('https://github.com/settings/tokens')).toBe('github.com');
    });

    it('strips www prefix correctly', () => {
      expect(getDomainName('https://www.google.com/search?q=test')).toBe('google.com');
      expect(getDomainName('www.amazon.se')).toBe('amazon.se');
    });

    it('extracts raw domain input without protocol', () => {
      expect(getDomainName('discord.com/channels')).toBe('discord.com');
    });
  });

  describe('DOMAIN_RULES presets', () => {
    it('contains presets for key services', () => {
      const allDomains = DOMAIN_RULES.flatMap((r) => r.domains);
      expect(allDomains).toContain('google.com');
      expect(allDomains).toContain('github.com');
      expect(allDomains).toContain('apple.com');
      expect(allDomains).toContain('steamcommunity.com');
    });

    it('enforces maximum length restrictions for constrained services', () => {
      const msRule = DOMAIN_RULES.find((r) => r.domains.includes('microsoft.com'));
      expect(msRule).toBeDefined();
      expect(msRule?.length).toBeLessThanOrEqual(16);

      const paypalRule = DOMAIN_RULES.find((r) => r.domains.includes('paypal.com'));
      expect(paypalRule).toBeDefined();
      expect(paypalRule?.length).toBe(20);
    });
  });
});
