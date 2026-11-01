export type SyncChoice = 'local' | 'remote' | 'both' | 'edited';
export interface SyncDecision { key: string; choice: SyncChoice; text?: string | null }
export interface SyncPolicy { review_conflicts_only: boolean; prefer_latest: boolean }
export interface SyncChange {
  key: string;
  entry_id: string;
  title: string;
  field: string;
  custom_field_name?: string | null;
  conflict: boolean;
  suggested: SyncChoice | null;
  local_changed_at: string | null;
  remote_changed_at: string | null;
  local_device: string | null;
  remote_device: string | null;
  local_device_name?: string | null;
  remote_device_name?: string | null;
  local_present: boolean;
  remote_present: boolean;
  sensitive: boolean;
  can_keep_both: boolean;
  can_edit: boolean;
  time_order_reliable?: boolean;
  requires_explicit_choice?: boolean;
  keep_both_entries?: Array<{ id: string; title: string }> | null;
  local_state?: 'active' | 'trash' | 'deleted' | 'absent' | null;
  remote_state?: 'active' | 'trash' | 'deleted' | 'absent' | null;
}
export interface SyncPreview {
  id: string;
  background?: boolean;
  local_revision: string;
  remote_revision: string;
  changes: SyncChange[];
}

export interface SyncOperation {
  id: string;
  transport: 'p2p_listener' | 'p2p_client' | 'webdav' | 'sync' | 'pairing';
  phase: 'connecting' | 'listening' | 'comparing' | 'reviewing' | 'awaiting_review' | 'preparing' | 'saving' | 'waiting_peer' | 'complete' | 'incomplete' | 'cancelled' | 'failed';
  error: string | null;
  peer_device_id?: string | null;
}
export interface SyncOutcome { operation: SyncOperation; stats: import('@/lib/backend').MergeStats }
