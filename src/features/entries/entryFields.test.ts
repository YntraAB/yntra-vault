import { describe, expect, it } from 'bun:test';
import type { PasswordEntry } from '@/types';
import { applyFieldPreset, entryStandardFields, fieldOrderMetadata } from './entryFields';

const entry: PasswordEntry = {
  id: 'entry', title: 'Gmail', username: 'user', password: 'fixture', email: '', url: '', notes: '',
  tags: [], favorite: false, pinned: false, hasPasskey: false, createdAt: '', updatedAt: '',
  customFields: [{ id: 'layout', name: '_field_order', type: 'text', value: 'username,password' }],
};

describe('editing synchronized entries', () => {
  it('switches templates without hiding stored values or extra credentials', () => {
    const existing = { ...entry, customFields: [{ id: 'phone', name: 'Phone', type: 'phone' as const, value: '+46700000000' }] };
    const order = ['username', 'password', 'url', 'phone', 'attachments'];
    expect(applyFieldPreset(existing, order, ['notes'])).toEqual(['notes', 'username', 'password', 'phone', 'attachments']);
    expect(applyFieldPreset(existing, order, [])).toEqual(['username', 'password', 'phone', 'attachments']);
  });
  it('keeps remotely added notes and account fields even when the saved layout predates them', () => {
    const fields = entryStandardFields({ ...entry, notes: 'Notes from desktop', email: 'fixture@example.test' }, '');
    expect(fields).toContain('notes');
    expect(fields).toContain('email');
    expect(fields.slice(0, 2)).toEqual(['username', 'password']);
  });

  it('includes synchronized authentication and attachment data in an older layout', () => {
    const fields = entryStandardFields({ ...entry, totpSecret: 'has-totp', hasPasskey: true, attachmentCount: 1 }, '');
    expect(fields).toContain('totpSecret');
    expect(fields).toContain('passkey');
    expect(fields).toContain('attachments');
    expect(entryStandardFields(entry, 'recovery code')).toContain('totpSecret');
  });

  it('does not restore a deliberately removed empty field', () => {
    expect(entryStandardFields(entry, '')).not.toContain('notes');
  });

  it('retains the layout identity across repeated edits', () => {
    const metadata = fieldOrderMetadata(entry.customFields, ['username', 'password', 'notes']);
    expect(metadata.id).toBe('layout');
    expect(fieldOrderMetadata([metadata], ['password', 'username', 'notes']).id).toBe('layout');
  });

  it('matches the first layout added independently to an existing entry', () => {
    const id = 'f3bdf8b4-a94f-406f-8b0b-3b1413e71cb4';
    const phone = fieldOrderMetadata([], ['username', 'password'], id);
    const desktop = fieldOrderMetadata([], ['password', 'username'], id);
    expect(phone.id).toBe(desktop.id);
    expect(phone.value).not.toBe(desktop.value);
  });
});
