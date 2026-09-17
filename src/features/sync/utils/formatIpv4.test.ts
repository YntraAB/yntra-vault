import { describe, it, expect } from 'bun:test';
import { formatIpv4Input } from './formatIpv4';

describe('formatIpv4Input', () => {
  it('automatically adds a dot after 3 digits are entered in an octet', () => {
    expect(formatIpv4Input('192', '')).toBe('192.');
    expect(formatIpv4Input('192.168', '192.')).toBe('192.168.');
    expect(formatIpv4Input('192.168.100', '192.168.')).toBe('192.168.100.');
  });

  it('does not add a trailing dot after 4th octet', () => {
    expect(formatIpv4Input('192.168.100.254', '192.168.100.')).toBe('192.168.100.254');
  });

  it('allows manual dot input for 1 or 2 digit octets without duplicating', () => {
    expect(formatIpv4Input('192.168.1.', '192.168.1')).toBe('192.168.1.');
    expect(formatIpv4Input('192..', '192.')).toBe('192.');
  });

  it('allows backspace without re-inserting deleted dot', () => {
    expect(formatIpv4Input('192', '192.')).toBe('192');
    expect(formatIpv4Input('19', '192')).toBe('19');
  });

  it('preserves and sanitizes port notation', () => {
    expect(formatIpv4Input('192.168.1.50:5322', '')).toBe('192.168.1.50:5322');
    expect(formatIpv4Input('192.168.1.50:5324abc', '')).toBe('192.168.1.50:5324');
  });

  it('clamps octet values exceeding 255 if 3 digits', () => {
    expect(formatIpv4Input('999', '')).toBe('255.');
  });

  it('supports hostnames like localhost and desktop.local', () => {
    expect(formatIpv4Input('localhost', '')).toBe('localhost');
    expect(formatIpv4Input('localhost:5324', '')).toBe('localhost:5324');
    expect(formatIpv4Input('desktop-pc.local:5324', '')).toBe('desktop-pc.local:5324');
  });
});
