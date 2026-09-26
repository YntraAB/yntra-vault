/**
 * Tauri Backend — calls Rust via IPC invoke()
 * 
 * Each method maps 1:1 to a #[tauri::command] in src-tauri/src/commands.rs
 */

import { invokeIpc as invoke } from '@/types/ipc';
import type {
  YntraVaultBackend,
  EmergencyKit,
  EmergencyKitAudit,
  VaultInfo,
  EntryPreview,
  DecryptedEntry,
  NewEntry,
  UpdateEntry,
  TrashedEntryPreview,
  VaultStorageMetrics,
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
  PairingStats,
  LocalDeviceInfo,
  QrSessionInfo,
  QrClientPairingResult,
  TrustedDevice,
  ParsedImportEntry,
  ImportPreviewResult,
  BiometricInfo,
  Hardware2FaInfo,
  Hardware2FaChallengeInfo,
  HardwareKeyInfo,
  AttachmentInfo,
  Hardware2FaProtocol,
  OpenDialogOptions,
  SaveDialogOptions,
  CheckUpdateResult,
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
    const raw = await invoke('list_entries');
    return raw.map(entry => ({
      ...entry,
      breach_status: deserializeBreachStatus(entry.breach_status),
    }));
  }

  async searchEntries(query: string): Promise<EntryPreview[]> {
    const raw = await invoke('search_entries', { query });
    return raw.map(entry => ({
      ...entry,
      breach_status: deserializeBreachStatus(entry.breach_status),
    }));
  }

  async getEntry(id: string): Promise<DecryptedEntry> {
    const raw = await invoke('get_entry', { id });
    return {
      ...raw,
      breach_status: deserializeBreachStatus(raw.breach_status),
    };
  }

  async addEntry(entry: NewEntry): Promise<string> {
    const entryPayload = { ...entry };
    if (entry.attachments && entry.attachments.length > 0) {
      entryPayload.attachments = entry.attachments.map(a => ({
        ...a,
        data: a.data instanceof Uint8Array ? Array.from(a.data) : a.data,
      }));
    }
    return invoke('add_entry', { entry: entryPayload });
  }

  async updateEntry(id: string, update: UpdateEntry): Promise<void> {
    const updatePayload = { ...update };
    if (update.breach_status) {
      updatePayload.breach_status = serializeBreachStatus(update.breach_status);
    }
    if (update.new_attachments && update.new_attachments.length > 0) {
      updatePayload.new_attachments = update.new_attachments.map(a => ({
        ...a,
        data: a.data instanceof Uint8Array ? Array.from(a.data) : a.data,
      }));
    }
    return invoke('update_entry', { id, update: updatePayload });
  }

  async updateEntryBreachStatus(id: string, breachStatus: BreachStatus): Promise<void> {
    return invoke('update_entry_breach_status', {
      id,
      breachStatus: serializeBreachStatus(breachStatus),
    });
  }

  async saveVault(): Promise<void> {
    return invoke('save_vault');
  }

  async reloadVault(): Promise<void> {
    return invoke('reload_vault');
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

  // ─── Attachments ────────────────────────────────────────────────

  async getAttachmentData(entryId: string, attachmentId: string): Promise<number[] | Uint8Array> {
    const res: any = await invoke('get_attachment_data', { entryId, attachmentId });
    if (res instanceof ArrayBuffer) {
      return new Uint8Array(res);
    }
    if (res instanceof Uint8Array) {
      return res;
    }
    return res;
  }

  async addAttachment(entryId: string, name: string, mimeType: string, data: number[]): Promise<AttachmentInfo> {
    return invoke('add_attachment', { entryId, name, mimeType, data });
  }

  async deleteAttachment(entryId: string, attachmentId: string): Promise<void> {
    return invoke('delete_attachment', { entryId, attachmentId });
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

  async emptyTrash(): Promise<void> {
    return invoke('empty_trash');
  }

  async purgeExpiredTrash(maxAgeDays?: number): Promise<number> {
    return invoke('purge_expired_trash', { maxAgeDays });
  }

  async getStorageMetrics(): Promise<VaultStorageMetrics> {
    return invoke('get_storage_metrics');
  }

  async compactVault(): Promise<VaultStorageMetrics> {
    return invoke('compact_vault');
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
    const res = await invoke('check_password_breach', { password });
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

  async reorderTags(tagIds: string[]): Promise<void> {
    return invoke('reorder_tags', { tagIds });
  }

  async checkVaultFileExists(path: string): Promise<boolean> {
    return invoke('check_vault_file_exists', { path });
  }

  async showInExplorer(path: string): Promise<void> {
    return invoke('show_in_explorer', { path });
  }

  async getInstalledApps(): Promise<import('@/types').InstalledApp[]> {
    return invoke('get_installed_apps');
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

  async getFavicon(domain: string): Promise<string | null> {
    return invoke('get_favicon', { domain });
  }

  async setExternalFaviconsEnabled(enabled: boolean): Promise<void> {
    return invoke('set_external_favicons_enabled', { enabled });
  }

  async isExternalFaviconsEnabled(): Promise<boolean> {
    return invoke('is_external_favicons_enabled');
  }

  async setMinimizeToTray(enabled: boolean): Promise<void> {
    return invoke('set_minimize_to_tray', { enabled });
  }

  async setWindowCaptureProtection(enable: boolean): Promise<void> {
    return invoke('set_window_capture_protection', { enable });
  }

  async setLockOnFocusLoss(enabled: boolean): Promise<void> {
    return invoke('set_lock_on_focus_loss', { enabled });
  }

  async setLockOnSystemLock(enabled: boolean): Promise<void> {
    return invoke('set_lock_on_system_lock', { enabled });
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

  async runP2pSyncListener(listenAddr: string, dbPath: string): Promise<MergeStats> {
    return invoke('run_p2p_sync_listener', { listenAddr, dbPath });
  }

  async runP2pSyncClient(serverAddr: string, dbPath: string, deviceId?: string): Promise<MergeStats> {
    return invoke('run_p2p_sync_client', { serverAddr, dbPath, deviceId });
  }

  async getLocalIp(): Promise<string | null> {
    return invoke('get_local_ip', {});
  }

  async getLocalIps(): Promise<string[]> {
    return invoke('get_local_ips', {});
  }

  async scanP2pDiscovery(timeoutMs?: number | null): Promise<string | null> {
    return invoke('scan_p2p_discovery', { timeoutMs });
  }

  async generatePairingCode(): Promise<string> {
    return invoke('generate_pairing_code');
  }

  async getLocalDeviceInfo(): Promise<LocalDeviceInfo> {
    return invoke('get_local_device_info');
  }

  async getTrustedDevices(): Promise<TrustedDevice[]> {
    return invoke('get_trusted_devices');
  }

  async revokeTrustedDevice(deviceId: string): Promise<void> {
    return invoke('revoke_trusted_device', { deviceId });
  }

  async startPairingHost(listenAddr: string, password: string, pairingCode: string, deviceName?: string): Promise<PairingStats> {
    return invoke('start_pairing_host', { listenAddr, password, pairingCode, deviceName });
  }

  async cancelPairingHost(): Promise<void> {
    return invoke('cancel_pairing_host');
  }

  async startPairingClient(serverAddr: string, password: string, pairingCode: string, dbPath: string, deviceName?: string): Promise<PairingStats> {
    return invoke('start_pairing_client', { serverAddr, password, pairingCode, dbPath, deviceName });
  }

  async scanPairingDiscovery(password: string, pairingCode: string, timeoutMs?: number | null): Promise<string | null> {
    return invoke('scan_pairing_discovery', { password, pairingCode, timeoutMs });
  }

  async generateQrPairingSession(deviceName?: string): Promise<QrSessionInfo> {
    return invoke('generate_qr_pairing_session', { deviceName });
  }

  async startQrPairingHost(password: string, includePassword: boolean, deviceName?: string): Promise<PairingStats> {
    return invoke('start_qr_pairing_host', { password, includePassword, deviceName });
  }

  async cancelQrPairingHost(): Promise<void> {
    return invoke('cancel_qr_pairing_host');
  }

  async startQrPairingClient(qrPayload: string, deviceName?: string, password?: string): Promise<QrClientPairingResult> {
    return invoke('start_qr_pairing_client', { qrPayload, deviceName, password });
  }

  async completeAdoptedVault(password: string): Promise<PairingStats> {
    return invoke('complete_adopted_vault', { password });
  }

  async splitMasterPassword(password: string): Promise<string[]> {
    return invoke('split_master_password', { password });
  }

  async reconstructMasterPassword(shareA: string, shareB: string): Promise<string> {
    return invoke('reconstruct_master_password', { shareA, shareB });
  }

  async reconstructMasterPasswordHash(shareA: string, shareB: string): Promise<string> {
    return invoke('reconstruct_master_password_hash', { shareA, shareB });
  }

  async generateEmergencyKit(masterPassword: string): Promise<EmergencyKit> {
    return invoke('generate_emergency_kit', { masterPassword });
  }

  async getEmergencyKitAudit(): Promise<EmergencyKitAudit | null> {
    return invoke('get_emergency_kit_audit');
  }

  async resetEmergencyKitAudit(): Promise<void> {
    return invoke('reset_emergency_kit_audit');
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

  async getHardware2FaChallenge(path: string): Promise<Hardware2FaChallengeInfo | null> {
    return invoke('get_hardware2fa_challenge', { path });
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
    credentialId?: number[],
  ): Promise<number[]> {
    return invoke('perform_hardware2fa_challenge', { protocol, challenge, credentialId });
  }

  async enableHardware2Fa(
    password: string,
    keyFilePath: string | undefined,
    protocol: Hardware2FaProtocol,
    keyName: string,
    challengeSalt: number[] | undefined,
    credentialId: number[] | undefined,
    hardwareResponse: number[],
  ): Promise<void> {
    return invoke('enable_hardware2fa', {
      password,
      keyFilePath,
      protocol,
      keyName,
      challengeSalt,
      credentialId,
      hardwareResponse,
    });
  }

  async disableHardware2Fa(): Promise<void> {
    return invoke('disable_hardware2fa');
  }

  // ─── Clipboard Defense ───────────────────────────────────────────────

  async copyToClipboard(text: string, isSensitive: boolean = true, clearAfterSecs?: number): Promise<void> {
    try {
      await invoke('copy_to_clipboard', {
        text,
        isSensitive,
        clearAfterSecs,
      });
    } catch {
      // Fallback to standard web clipboard if IPC call fails
      await navigator.clipboard.writeText(text);
    }
  }

  async clearClipboard(): Promise<void> {
    try {
      await invoke('clear_clipboard');
    } catch {
      await navigator.clipboard.writeText('');
    }
  }

  async copyEntryPassword(entryId: string, clearAfterSecs?: number): Promise<void> {
    return invoke('copy_entry_password', { entryId, clearAfterSecs });
  }

  async copyEntryUsername(entryId: string): Promise<void> {
    return invoke('copy_entry_username', { entryId });
  }

  async copyEntryTotp(entryId: string, clearAfterSecs?: number): Promise<void> {
    return invoke('copy_entry_totp', { entryId, clearAfterSecs });
  }

  async createVaultBytes(name: string, passwordBytes: Uint8Array | number[], path: string, keyFilePath?: string): Promise<VaultInfo> {
    return invoke('create_vault_bytes', {
      name,
      passwordBytes: Array.from(passwordBytes),
      path,
      keyFilePath,
    });
  }

  async openVaultBytes(path: string, passwordBytes: Uint8Array | number[], keyFilePath?: string): Promise<VaultInfo> {
    return invoke('open_vault_bytes', {
      path,
      passwordBytes: Array.from(passwordBytes),
      keyFilePath,
    });
  }

  async changeMasterPasswordBytes(currentBytes: Uint8Array | number[], newPasswordBytes: Uint8Array | number[], currentKeyFile?: string, newKeyFile?: string): Promise<void> {
    return invoke('change_master_password_bytes', {
      currentBytes: Array.from(currentBytes),
      newPasswordBytes: Array.from(newPasswordBytes),
      currentKeyFile,
      newKeyFile,
    });
  }

  async autotypeEntryPassword(entryId: string, charDelayMs?: number, settleDelayMs?: number): Promise<void> {
    return invoke('autotype_entry_password', { entryId, charDelayMs, settleDelayMs });
  }

  async autotypeEntrySmart(entryId: string, launchBrowser?: boolean, charDelayMs?: number, fieldDelayMs?: number): Promise<void> {
    return invoke('autotype_entry_smart', { entryId, launchBrowser, charDelayMs, fieldDelayMs });
  }

  async verifyBiometric2Fa(prompt?: string): Promise<void> {
    return invoke('verify_biometric_2fa', { prompt });
  }

  // ─── Mobile Native Autofill Integration ──────────────────────────────────

  async queryMobileAutofillStatus(): Promise<import('./backend').MobileAutofillStatus> {
    return invoke('query_mobile_autofill_status');
  }

  async getAutofillCredentialsForPackage(packageName: string, webDomain?: string): Promise<import('./backend').AutofillDatasetPayload> {
    return invoke('get_autofill_credentials_for_package', { packageName, webDomain });
  }

  // ─── File Dialogs ────────────────────────────────────────────────
  async openFileDialog(options?: OpenDialogOptions): Promise<string | string[] | null> {
    const { open } = await import('@tauri-apps/plugin-dialog');
    return open(options as any);
  }

  async saveFileDialog(options?: SaveDialogOptions): Promise<string | null> {
    const { save } = await import('@tauri-apps/plugin-dialog');
    return save(options as any);
  }

  async getMobileVaultPath(name: string): Promise<string | null> {
    return invoke('get_mobile_vault_path', { name });
  }

  // ─── Smart Login ────────────────────────────────────────────────

  async smartLoginPrecheck(entryId?: string): Promise<import('./backend').SmartLoginPreCheckResult> {
    return invoke('smart_login_precheck', { entryId });
  }

  async smartLoginCloseBrowser(processName: string): Promise<void> {
    return invoke('smart_login_close_browser', { processName });
  }

  async smartLoginStart(entryId: string, browserIndex: number): Promise<void> {
    return invoke('smart_login_start', { entryId, browserIndex });
  }

  async smartLoginCancel(): Promise<void> {
    return invoke('smart_login_cancel');
  }

  async onSmartLoginProgress(callback: (event: any) => void): Promise<() => void> {
    const { listen } = await import('@tauri-apps/api/event');
    const unlisten = await listen('smart-login-progress', (e) => callback(e.payload));
    return unlisten;
  }

  async onSmartLoginResult(callback: (result: any) => void): Promise<() => void> {
    const { listen } = await import('@tauri-apps/api/event');
    const unlisten = await listen('smart-login-result', (e) => callback(e.payload));
    return unlisten;
  }

  async checkAppUpdate(customEndpoint?: string): Promise<CheckUpdateResult> {
    return invoke('check_app_update', { customEndpoint });
  }

  async downloadAndInstallApk(apkUrl: string, expectedSha256?: string): Promise<string> {
    return invoke('download_and_install_apk', { apkUrl, expectedSha256 });
  }

  async installPortableUpdate(url: string, expectedSha256?: string): Promise<void> {
    return invoke('install_portable_update', { url, expectedSha256 });
  }

  async getAppVersion(): Promise<string> {
    return invoke('get_app_version');
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



