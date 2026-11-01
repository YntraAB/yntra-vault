import type { PasswordEntry } from '@/types';
import { getFieldLayout } from '@/lib/utils';
import { isRecoveryField } from './context/EntriesContext.model';

type LoginEntry = Pick<PasswordEntry, 'username' | 'email' | 'password' | 'url' | 'customFields'>;
export function loginFields(entry: LoginEntry, order?: string[]) {
  const fields = entry.customFields.filter(field => !field.name.startsWith('_') && !isRecoveryField(field.name));
  const candidates = (order ?? getFieldLayout(entry.customFields, ['username', 'email', 'password', 'url']))
    .map(id => {
      if (id === 'username' || id === 'email' || id === 'password' || id === 'url') return { id, type: id, value: entry[id] };
      return fields.find(field => field.id === id);
    }).filter(field => field?.value.trim());
  return {
    identifier: candidates.find(field => ['username', 'email', 'phone'].includes(field!.type)),
    password: candidates.find(field => field!.type === 'password'),
    url: candidates.find(field => field!.type === 'url'),
  };
}
