import type { SyncOperation } from './types';

export function updateSyncOperations(previous: SyncOperation[], operation: SyncOperation): SyncOperation[] {
  return [...previous.filter(item => {
    if (item.id === operation.id) return false;
    return !(operation.phase === 'complete' && operation.peer_device_id &&
      item.peer_device_id === operation.peer_device_id &&
      ['failed', 'incomplete', 'cancelled'].includes(item.phase));
  }), operation];
}
