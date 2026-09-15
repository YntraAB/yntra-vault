# Vault Storage Lifecycle, Trash Compaction & Footprint Analytics

> Technical documentation on Yntra Vault's soft-deletion tombstone mechanics, automatic 30-day retention pruning, search index compaction, and encrypted storage footprint metrics.

---

## 1. Storage Architecture & Single-File `.vdb`

Yntra Vault bundles all vault contents, attachments, folders, tags, and soft-deleted items into a single authenticated `.vdb` archive.

To ensure minimal file size on disk and optimal cryptographic operation:
1. **Encrypted Attachments**: Encrypted file blobs are serialized within the entry tree.
2. **Soft Deletions**: Deleting an entry moves it to the `trash` tombstone list, allowing immediate user recovery without data loss.
3. **Atomic Writing**: Every disk save writes to a sibling temporary file (`.vdb.tmp`) and executes an atomic OS rename.

---

## 2. Trash Lifecycle & 30-Day Auto-Pruning

### Tombstone Structure
When `delete_entry(id)` is called:
1. The entry is removed from `data.entries`.
2. All corresponding trigrams are purged from the in-memory inverted search index via `remove_entry_from_index(id)`.
3. The entry is appended to `data.trash` wrapped in `TrashedEntry { entry, deleted_at: Utc::now() }`.

### Retention Invariant
Entries in trash are retained for **30 days**:
$$\text{Days Remaining} = 30 - (\text{now} - \text{deleted\_at})_{\text{days}}$$

### Automatic and On-Demand Pruning
- **On Save**: Every execution of `VaultManager::save()` automatically applies the retention filter:
  ```rust
  let cutoff = Utc::now() - chrono::Duration::days(30);
  self.data.trash.retain(|t| t.deleted_at > cutoff);
  ```
- **On Demand**: The `purge_expired_trash(max_age_days: i64) -> usize` API allows manual or scheduled purging of expired items.
- **Empty Trash**: `purge_all_trash() -> usize` completely empties the trash and atomically flushes the vault to disk.

---

## 3. Storage Footprint Analytics (`VaultStorageMetrics`)

To provide users and system operators with detailed insight into encrypted storage utilization, `VaultManager::get_storage_metrics()` calculates:

| Metric Field | Description |
|---|---|
| `entry_count` | Number of active password and note entries. |
| `trashed_entry_count` | Number of items currently residing in the trash. |
| `tag_count` | Number of unique tags assigned across entries. |
| `active_attachment_count` | Total number of file attachments across active entries. |
| `active_attachment_bytes` | Aggregate raw byte size of all active attachments. |
| `trashed_attachment_count` | Total number of attachments held within trashed entries. |
| `trashed_attachment_bytes` | Aggregate raw byte size of attachments held within trashed entries. |
| `vault_file_bytes` | Exact physical byte size of the `.vdb` database file on disk. |

---

## 4. Full Vault Compaction (`compact_vault`)

Over time, frequent additions, updates, deletions, and tag mutations can leave fragmented memory states or orphaned search buckets.

The `compact_vault(&mut self) -> Result<VaultStorageMetrics>` command performs a comprehensive 4-stage optimization:
1. **Purges Expired Trash**: Removes all tombstones older than 30 days.
2. **Rebuilds Search Index**: Completely drops and re-indexes all active entries, eliminating stale trigram buckets and reducing memory footprint.
3. **Repacks Storage Archive**: Serializes clean MessagePack data and writes an atomic encrypted archive to disk.
4. **Emits Fresh Metrics**: Returns newly computed `VaultStorageMetrics`.

---

## 5. IPC Interface

```typescript
// IPC Method Signatures (Tauri 2)
invoke('purge_expired_trash', { maxAgeDays?: number }): Promise<number>;
invoke('get_storage_metrics'): Promise<VaultStorageMetrics>;
invoke('compact_vault'): Promise<VaultStorageMetrics>;
```
