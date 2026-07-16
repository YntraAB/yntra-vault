/**
 * Tauri Backend — calls Rust via IPC invoke()
 * 
 * Each method maps 1:1 to a #[tauri::command] in src-tauri/src/commands.rs
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  YntraVaultBackend,
  VaultInfo,
  EntryPreview,
  DecryptedEntry,
  NewEntry,
  UpdateEntry,
  TrashedEntryPreview,
  DecryptedHistoryItem,
  TotpCode,
  TotpConfig,
  GeneratorOptions,
  BreachResult,
  StrengthScore,
  SecurityAudit,
  Tag,
  BreachStatus,
} from './backend';

export class TauriBackend implements YntraVaultBackend {

  // ─── Vault ──────────────────────────────────────────────────────

  async createVault(name: string, password: string, path: string): Promise<VaultInfo> {
    return invoke('create_vault', { name, password, path });
  }

  async openVault(path: string, password: string): Promise<VaultInfo> {
    return invoke('open_vault', { path, password });
  }

  async lockVault(): Promise<void> {
    return invoke('lock_vault');
  }

  async getVaultInfo(): Promise<VaultInfo | null> {
    return invoke('get_vault_info');
  }

  // ─── Entries ────────────────────────────────────────────────────

  async listEntries(): Promise<EntryPreview[]> {
    const raw = await invoke<any[]>('list_entries');
    return raw.map(entry => ({
      ...entry,
      breach_status: deserializeBreachStatus(entry.breach_status),
    }));
  }

  async searchEntries(query: string): Promise<EntryPreview[]> {
    const raw = await invoke<any[]>('search_entries', { query });
    return raw.map(entry => ({
      ...entry,
      breach_status: deserializeBreachStatus(entry.breach_status),
    }));
  }

  async getEntry(id: string): Promise<DecryptedEntry> {
    const raw = await invoke<any>('get_entry', { id });
    return {
      ...raw,
      breach_status: deserializeBreachStatus(raw.breach_status),
    };
  }

  async addEntry(entry: NewEntry): Promise<string> {
    return invoke('add_entry', { entry });
  }

  async updateEntry(id: string, update: UpdateEntry): Promise<void> {
    const updatePayload = { ...update };
    if (update.breach_status) {
      updatePayload.breach_status = serializeBreachStatus(update.breach_status);
    }
    return invoke('update_entry', { id, update: updatePayload });
  }

  async deleteEntry(id: string): Promise<void> {
    return invoke('delete_entry', { id });
  }

  async toggleFavorite(id: string): Promise<boolean> {
    return invoke('toggle_favorite', { id });
  }

  async togglePin(id: string): Promise<boolean> {
    return invoke('toggle_pin', { id });
  }

  // ─── Trash ──────────────────────────────────────────────────────

  async listTrash(): Promise<TrashedEntryPreview[]> {
    return invoke('list_trash');
  }

  async restoreFromTrash(id: string): Promise<void> {
    return invoke('restore_from_trash', { id });
  }

  async permanentDelete(id: string): Promise<void> {
    return invoke('permanent_delete', { id });
  }

  // ─── Password History ───────────────────────────────────────────

  async getPasswordHistory(entryId: string): Promise<DecryptedHistoryItem[]> {
    return invoke('get_password_history', { entryId });
  }

  // ─── TOTP ───────────────────────────────────────────────────────

  async generateTotp(secret: string): Promise<TotpCode> {
    return invoke('generate_totp', { secret });
  }

  async generateTotpWithConfig(config: TotpConfig): Promise<TotpCode> {
    return invoke('generate_totp_with_config', { config });
  }

  async parseOtpauthUri(uri: string): Promise<TotpConfig> {
    return invoke('parse_otpauth_uri', { uri });
  }

  // ─── Password Generator ────────────────────────────────────────

  async generatePassword(options: GeneratorOptions): Promise<string> {
    return invoke('generate_password', { options });
  }

  async generatePasswordDefault(): Promise<string> {
    return invoke('generate_password_default');
  }

  // ─── Breach Detection ──────────────────────────────────────────

  async checkPasswordBreach(password: string): Promise<BreachResult> {
    const count = await invoke<number>('check_password_breach', { password });
    return {
      is_breached: count > 0,
      breach_count: count,
      checked_at: new Date().toISOString(),
    };
  }

  async analyzePasswordStrength(password: string): Promise<StrengthScore> {
    return invoke('analyze_password_strength', { password });
  }

  // ─── Security ───────────────────────────────────────────────────

  async securityAudit(): Promise<SecurityAudit> {
    return invoke('security_audit');
  }

  async changeMasterPassword(current: string, newPassword: string): Promise<void> {
    return invoke('change_master_password', { current, newPassword });
  }

  // ─── Tags ───────────────────────────────────────────────────────

  async getTags(): Promise<Tag[]> {
    return invoke('get_tags');
  }

  async addTag(name: string, color: string, icon: string): Promise<string> {
    return invoke('add_tag', { name, color, icon });
  }

  async deleteTag(id: string): Promise<void> {
    return invoke('delete_tag', { id });
  }

  async checkVaultFileExists(path: string): Promise<boolean> {
    return invoke('check_vault_file_exists', { path });
  }
}

// ─── BreachStatus IPC Mappers ─────────────────────────────────────────

function serializeBreachStatus(status?: BreachStatus): any {
  if (!status) return undefined;
  if (status.type === 'Unknown') return 'Unknown';
  if (status.type === 'Checking') return 'Checking';
  if (status.type === 'Safe') {
    return { Safe: { checked_at: status.checked_at } };
  }
  if (status.type === 'Breached') {
    return { Breached: { breach_count: status.breach_count, checked_at: status.checked_at } };
  }
  if (status.type === 'Error') {
    return { Error: { message: status.message } };
  }
  return 'Unknown';
}

function deserializeBreachStatus(rawStatus: any): BreachStatus {
  if (!rawStatus) return { type: 'Unknown' };
  if (rawStatus === 'Unknown') return { type: 'Unknown' };
  if (rawStatus === 'Checking') return { type: 'Checking' };
  if (typeof rawStatus === 'object') {
    if ('Safe' in rawStatus) {
      return { type: 'Safe', checked_at: rawStatus.Safe.checked_at };
    }
    if ('Breached' in rawStatus) {
      return {
        type: 'Breached',
        breach_count: rawStatus.Breached.breach_count,
        checked_at: rawStatus.Breached.checked_at,
      };
    }
    if ('Error' in rawStatus) {
      return { type: 'Error', message: rawStatus.Error.message };
    }
  }
  return { type: 'Unknown' };
}



