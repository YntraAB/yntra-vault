import type { CustomField, PasswordEntry } from '@/types';

export type StandardFieldKey = 'username' | 'password' | 'email' | 'url' | 'notes' | 'totpSecret' | 'passkey' | 'attachments';
const standardKeys: StandardFieldKey[] = ['username', 'password', 'url', 'email', 'notes', 'totpSecret', 'passkey', 'attachments'];

export function applyFieldPreset(entry: Pick<PasswordEntry, 'username' | 'password' | 'email' | 'url' | 'notes' | 'totpSecret' | 'hasPasskey' | 'customFields'>, order: string[], preset: string[]): string[] {
  const retained = order.filter(id => {
    if (id === 'attachments') return true;
    if (id === 'passkey') return entry.hasPasskey;
    if (id === 'username' || id === 'password' || id === 'email' || id === 'url' || id === 'notes' || id === 'totpSecret') return Boolean(entry[id]);
    return entry.customFields.some(field => field.id === id);
  });
  return [...new Set([...preset, ...retained])];
}

export function entryStandardFields(entry: PasswordEntry, recoveryCodes: string): StandardFieldKey[] {
  const saved = entry.customFields.find(field => field.name === '_field_order')?.value;
  const fields = (saved?.split(',').map(value => value.trim()) ?? [])
    .filter((value): value is StandardFieldKey => standardKeys.includes(value as StandardFieldKey));
  // A remote edit can add data after this device last saved its display order.
  for (const field of ['username', 'password', 'url', 'email', 'notes'] as const) {
    if (entry[field] && entry[field] !== '••••••••') fields.push(field);
  }
  if (entry.totpSecret || recoveryCodes) fields.push('totpSecret');
  if (entry.hasPasskey) fields.push('passkey');
  if (entry.attachments?.length || entry.attachmentCount) fields.push('attachments');
  if (!fields.length && !saved) fields.push('username', 'password', 'url');
  return [...new Set(fields)];
}

export function fieldOrderMetadata(fields: CustomField[], order: string[], entryId?: string): CustomField {
  return {
    // An existing entry gives its first layout the same identity on every replica.
    id: fields.find(field => field.name === '_field_order')?.id || entryId || crypto.randomUUID(),
    name: '_field_order',
    type: 'text',
    value: order.join(','),
  };
}
