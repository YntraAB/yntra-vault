import type { SyncChange } from './types';
export const syncFieldKeys: Record<string, string> = { title: 'entry.title', username: 'detail.username', email: 'detail.email', url: 'detail.url', notes: 'detail.notes', encrypted_password: 'detail.password', encrypted_totp_secret: 'entry.totp_placeholder', state: 'sync.review_state', tag: 'sync.review_tag', favorite: 'detail.favorite', pinned: 'entry.pinned_entry', passkey: 'detail.passkey', tags: 'sidebar.tags', tag_order: 'sidebar.tags', vault_name: 'settings.vault_name' };
export function syncFieldLabel(change: SyncChange, t: (key: string) => string): string {
  if (change.custom_field_name === '_field_order') return t('sync.review_field_order');
  if (change.field.startsWith('tag_membership/') || change.field.startsWith('tag_name/')) return t('sidebar.tags') + ': ' + (change.custom_field_name ?? '');
  if (syncFieldKeys[change.field]) return t(syncFieldKeys[change.field]);
  if (change.field.startsWith('custom/')) return change.custom_field_name || t('sync.review_custom');
  if (change.field.startsWith('attachment/')) return t('sync.review_attachment');
  return change.field;
}
