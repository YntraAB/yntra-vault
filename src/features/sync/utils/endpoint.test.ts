import { describe, expect, it } from 'bun:test';
import { syncEndpoint } from './endpoint';

describe('sync endpoint routing', () => {
  it('keeps the port advertised by discovery and handles IPv6', () => {
    expect(syncEndpoint('192.168.1.20:58123')).toBe('192.168.1.20:58123');
    expect(syncEndpoint('desktop.local')).toBe('desktop.local:5322');
    expect(syncEndpoint('2001:db8::4')).toBe('[2001:db8::4]:5322');
    expect(syncEndpoint('[2001:db8::4]:5678')).toBe('[2001:db8::4]:5678');
    expect(() => syncEndpoint('host:65536')).toThrow();
    expect(() => syncEndpoint('https://host')).toThrow();
  });
});
