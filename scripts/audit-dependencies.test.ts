import { expect, test } from 'bun:test';
import { classifyAdvisory } from './audit-dependencies.mjs';

test('maintenance notices remain visible without disguising crypto or unsoundness advisories', () => {
    const affected = (informational: string | null) => [{ database_specific: { informational } }];
    expect(classifyAdvisory({ affected: affected('unmaintained') })).toBe('maintenance');
    expect(classifyAdvisory({ affected: affected(null) })).toBe('vulnerability');
    expect(classifyAdvisory({ affected: affected('unsound') })).toBe('vulnerability');
    expect(classifyAdvisory({ affected: [...affected('unmaintained'), ...affected(null)] })).toBe('vulnerability');
    expect(classifyAdvisory({ withdrawn: '2026-11-01', affected: affected(null) })).toBe('withdrawn');
    expect(() => classifyAdvisory({ id: 'incomplete', affected: [] })).toThrow();
});
