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
  MergeStats,
  ParsedImportEntry,
  ImportPreviewResult,
  BiometricInfo,
  Hardware2FaInfo,
  HardwareKeyInfo,
  Hardware2FaProtocol,
} from './backend';

export class TauriBackend implements YntraVaultBackend {

  // ─── Vault ──────────────────────────────────────────────────────

  async createVault(name: string, password: string, path: string, keyFilePath?: string): Promise<VaultInfo> {
    return invoke('create_vault', { name, password, path, keyFilePath });
  }

  async openVault(path: string, password: string, keyFilePath?: string): Promise<VaultInfo> {
    return invoke('open_vault', { path, password, keyFilePath });
  }

  async lockVault(): Promise<void> {
    return invoke('lock_vault');
  }

  async getVaultInfo(): Promise<VaultInfo | null> {
    return invoke('get_vault_info');
  }

  async generateKeyFile(path: string): Promise<void> {
    return invoke('generate_key_file', { path });
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
    const res = await invoke<any>('check_password_breach', { password });
    return {
      is_breached: res.is_breached,
      breach_count: res.breach_count,
      checked_at: res.checked_at,
    };
  }

  async analyzePasswordStrength(password: string): Promise<StrengthScore> {
    return invoke('analyze_password_strength', { password });
  }

  // ─── Security ───────────────────────────────────────────────────

  async securityAudit(): Promise<SecurityAudit> {
    return invoke('security_audit');
  }

  async changeMasterPassword(
    current: string,
    newPassword: string,
    currentKeyFile?: string,
    newKeyFile?: string,
  ): Promise<void> {
    return invoke('change_master_password', {
      current,
      newPassword,
      currentKeyFile,
      newKeyFile,
    });
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

  async updateTag(id: string, name: string, color: string, icon: string): Promise<void> {
    return invoke('update_tag', { id, name, color, icon });
  }

  async checkVaultFileExists(path: string): Promise<boolean> {
    return invoke('check_vault_file_exists', { path });
  }

  async showInExplorer(path: string): Promise<void> {
    return invoke('show_in_explorer', { path });
  }

  // Advanced features
  async autotype(text: string, charDelayMs: number, settleDelayMs: number): Promise<void> {
    return invoke('autotype', { text, charDelayMs, settleDelayMs });
  }

  async runSmartAutotype(username: string, password: string, totpSecret: string, url: string, launchBrowser: boolean, charDelayMs: number, fieldDelayMs: number): Promise<void> {
    return invoke('run_smart_autotype', { username, password, totpSecret, url, launchBrowser, charDelayMs, fieldDelayMs });
  }

  async enableAutostart(): Promise<void> {
    return invoke('enable_autostart');
  }

  async disableAutostart(): Promise<void> {
    return invoke('disable_autostart');
  }

  async isAutostartEnabled(): Promise<boolean> {
    return invoke('is_autostart_enabled');
  }

  async setMinimizeToTray(enabled: boolean): Promise<void> {
    return invoke('set_minimize_to_tray', { enabled });
  }

  async webdavTestConnection(url: string, username: string, password: string | null): Promise<void> {
    return invoke('webdav_test_connection', { url, username, password });
  }

  async webdavUpload(url: string, username: string, password: string | null, dbPath: string, ifMatchEtag?: string | null): Promise<string | null> {
    return invoke('webdav_upload', { url, username, password, dbPath, ifMatchEtag: ifMatchEtag || null });
  }

  async webdavDownload(url: string, username: string, password: string | null, destDbPath: string): Promise<void> {
    return invoke('webdav_download', { url, username, password, destDbPath });
  }

  async webdavSync(url: string, username: string, password: string | null): Promise<MergeStats> {
    return invoke('webdav_sync', { url, username, password });
  }

  async runP2pSyncListener(listenAddr: string, dbPath: string): Promise<void> {
    return invoke('run_p2p_sync_listener', { listenAddr, dbPath });
  }

  async runP2pSyncClient(serverAddr: string, dbPath: string): Promise<void> {
    return invoke('run_p2p_sync_client', { serverAddr, dbPath });
  }

  async splitMasterPassword(password: string): Promise<string[]> {
    return invoke('split_master_password', { password });
  }

  async reconstructMasterPasswordHash(shareA: string, shareB: string): Promise<string> {
    return invoke('reconstruct_master_password_hash', { shareA, shareB });
  }

  // Export & Import
  async exportVault(destPath: string): Promise<void> {
    return invoke('export_vault', { destPath });
  }

  async exportVaultCsv(destPath: string): Promise<void> {
    return invoke('export_vault_csv', { destPath });
  }

  async exportVaultJson(destPath: string): Promise<void> {
    return invoke('export_vault_json', { destPath });
  }

  async getVaultPath(): Promise<string> {
    return invoke('get_vault_path');
  }

  async parseImportFile(filePath: string, format?: string): Promise<ImportPreviewResult> {
    return invoke('parse_import_file', { filePath, format });
  }

  async parseImportContent(content: string, format?: string): Promise<ImportPreviewResult> {
    return invoke('parse_import_content', { content, format });
  }

  async importEntries(entries: ParsedImportEntry[], duplicateStrategy: 'skip' | 'overwrite' | 'keep_both'): Promise<number> {
    return invoke('import_entries', { entries, duplicateStrategy });
  }

  // ─── Biometrics ──────────────────────────────────────────────

  async checkBiometricAvailable(): Promise<BiometricInfo> {
    return invoke('check_biometric_available');
  }

  async isBiometricEnabled(path: string): Promise<boolean> {
    return invoke('is_biometric_enabled', { path });
  }

  async unlockVaultBiometric(path: string): Promise<VaultInfo> {
    return invoke('unlock_vault_biometric', { path });
  }

  async enableBiometric(): Promise<void> {
    return invoke('enable_biometric');
  }

  async disableBiometric(): Promise<void> {
    return invoke('disable_biometric');
  }

  // ─── Hardware 2FA / YubiKey ──────────────────────────────────────

  async checkHardware2FaAvailable(): Promise<Hardware2FaInfo> {
    return invoke('check_hardware2fa_available');
  }

  async listHardwareKeys(): Promise<HardwareKeyInfo[]> {
    return invoke('list_hardware_keys');
  }

  async isHardware2FaEnabled(path: string): Promise<boolean> {
    return invoke('is_hardware2fa_enabled', { path });
  }

  async openVaultWithHardware2Fa(
    path: string,
    password: string,
    keyFilePath: string | undefined,
    hardwareResponse: number[],
  ): Promise<VaultInfo> {
    return invoke('open_vault_with_hardware2fa', {
      path,
      password,
      keyFilePath,
      hardwareResponse,
    });
  }

  async performHardware2FaChallenge(
    protocol: Hardware2FaProtocol,
    challenge?: number[],
  ): Promise<number[]> {
    return invoke('perform_hardware2fa_challenge', { protocol, challenge });
  }

  async enableHardware2Fa(
    protocol: Hardware2FaProtocol,
    keyName: string,
    hardwareResponse: number[],
  ): Promise<void> {
    return invoke('enable_hardware2fa', { protocol, keyName, hardwareResponse });
  }

  async disableHardware2Fa(): Promise<void> {
    return invoke('disable_hardware2fa');
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



