import { describe, expect, it } from 'bun:test';
import { loginFields } from './loginFields';
import { normalizeTagUrl } from './tagDefaults';
import type { CustomField } from '@/types';

const entry = { username: 'first-user', email: 'first@example.test', password: 'first-password', url: 'https://first.test', customFields: [
  { id: 'phone', type: 'phone', name: 'Phone', value: '+46700000000' },
  { id: 'second', type: 'password', name: 'Second password', value: 'second-password' },
  { id: 'email2', type: 'email', name: 'Other email', value: 'other@example.test' },
  { id: 'site', type: 'url', name: 'Other website', value: 'https://second.test' },
] as CustomField[] };
describe('ordered login fields', () => {
  it('uses the visible order across phone, email and username, and within passwords and URLs', () => {
    const fields = loginFields(entry, ['phone', 'email2', 'username', 'second', 'password', 'site', 'url']);
    expect(fields.identifier?.value).toBe('+46700000000');
    expect(fields.password?.value).toBe('second-password');
    expect(fields.url?.value).toBe('https://second.test');
    expect(loginFields(entry, ['email2', 'phone']).identifier?.value).toBe('other@example.test');
  });
  it('reopens persisted order and retains fields absent from stale layout metadata', () => {
    const fields = loginFields({ ...entry, customFields: [...entry.customFields, { id: 'layout', name: '_field_order', type: 'text', value: 'missing,second,phone' }] });
    expect(fields.identifier?.id).toBe('phone');
    expect(fields.password?.id).toBe('second');
    expect(fields.url?.id).toBe('url');
  });
  it('ignores empty fields and recovery codes', () => {
    const fields = loginFields({ ...entry, password: '', customFields: [{ id: 'codes', type: 'password', name: '2FA Recovery Codes', value: 'not-a-password' }, { id: 'empty', type: 'phone', name: 'Phone', value: '  ' }, ...entry.customFields] });
    expect(fields.password?.id).toBe('second');
    expect(fields.identifier?.id).toBe('username');
  });
});
describe('tag website defaults', () => {
  it('normalizes a bare website and permits clearing the suggestion', () => {
    expect(normalizeTagUrl(' gmail.com ')).toBe('https://gmail.com/');
    expect(normalizeTagUrl('')).toBe('');
  });
  it('rejects executable URLs, credentials and control characters', () => {
    for (const value of ['javascript://alert(1)', 'https://user:secret@example.test', 'file:///tmp/test', 'https://example.test/\nnext', 'https://example.test\\evil']) expect(() => normalizeTagUrl(value)).toThrow();
  });
});
