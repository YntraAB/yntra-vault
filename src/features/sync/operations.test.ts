import { expect, it } from 'bun:test';
import { updateSyncOperations } from './operations';
import type { SyncOperation } from './types';

it('clears an old failure only when that peer completes another sync', () => {
  const failed: SyncOperation = { id: 'first', transport: 'p2p_client', phase: 'incomplete', error: 'receipt missing', peer_device_id: 'phone' };
  const other: SyncOperation = { ...failed, id: 'other', peer_device_id: 'tablet' };
  const working: SyncOperation = { ...failed, id: 'retry', phase: 'saving', error: null };
  expect(updateSyncOperations([failed, other], working)).toContainEqual(failed);
  const complete: SyncOperation = { ...working, phase: 'complete' };
  expect(updateSyncOperations([failed, other, working], complete)).toEqual([other, complete]);
  expect(updateSyncOperations([failed], { ...complete, peer_device_id: null })).toContainEqual(failed);
});
