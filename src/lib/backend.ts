/**
 * Backend Abstraction Layer
 * 
 * This module defines the interface that ALL backends must implement.
 * The frontend code uses this interface exclusively — it never calls
 * Tauri or WASM directly.
 * 
 * Backends:
 * - TauriBackend (desktop) — calls Rust via IPC
 * - WasmBackend (web) — calls Rust compiled to WASM
 * - MockBackend (development) — uses in-memory data
 */

import type { EmergencyKit, EmergencyShare, EmergencyKitAudit, EmergencyKitAuditEntry, PairingStats, LocalDeviceInfo, QrSessionInfo, QrClientPairingResult, CheckUpdateResult } from '@/types/ipc';
export type { EmergencyKit, EmergencyShare, EmergencyKitAudit, EmergencyKitAuditEntry, PairingStats, LocalDeviceInfo, QrSessionInfo, QrClientPairingResult, CheckUpdateResult };

// ─── Types ──────────────────────────────────────────────────────────────

export interface VaultInfo {
  id: string;
  name: string;
  path: string;
  entry_count: number;
  last_opened: string | null;
}

export interface BiometricInfo {
  available: boolean;
  biometric_type: string;
}

export type Hardware2FaProtocol = 'YubiKeyChallengeResponse' | 'Fido2Ctap2HmacSecret';

export interface HardwareKeyInfo {
  id: string;
  name: string;
  protocol: Hardware2FaProtocol;
  serial: number | null;
  is_connected: boolean;
}

export interface Hardware2FaInfo {
  available: boolean;
  key_count: number;
  supported_protocols: Hardware2FaProtocol[];
  connected_keys: HardwareKeyInfo[];
}

export interface Hardware2FaChallengeInfo {
  enabled: boolean;
  protocol: Hardware2FaProtocol;
  key_name: string;
  challenge_salt: number[];
  credential_id: number[];
}

export interface TrustedDevice {
  id: string;
  name: string;
  device_type: 'desktop' | 'mobile' | 'tablet' | 'other' | string;
  os: string;
  paired_at: string;
  last_sync_at?: string | null;
  token_hash?: string;
}

export interface MergeStats {
  entries_added: number;
  entries_updated: number;
  entries_kept_local: number;
  tags_merged: number;
  trash_merged: number;
}

export interface AttachmentInfo {
  id: string;
  name: string;
  size: number;
  mime_type: string;
  created_at: string;
}

export interface NewAttachment {
  name: string;
  mime_type: string;
  data: Uint8Array | number[];
}

export interface EntryPreview {
  id: string;
  title: string;
  username: string;
  url: string;
  email: string;
  tags: string[];
  favorite: boolean;
  pinned: boolean;
  has_totp: boolean;
  entry_type: EntryType;
  updated_at: string;
  breach_status: BreachStatus;
  strength_score: StrengthScore | null;
  password_age_days: number;
  has_passkey: boolean;
  attachment_count?: number;
}

export interface DecryptedEntry {
  id: string;
  title: string;
  username: string;
  password: string;
  url: string;
  email: string;
  notes: string;
  tags: string[];
  favorite: boolean;
  pinned: boolean;
  totp_secret: string | null;
  custom_fields: CustomField[];
  entry_type: EntryType;
  created_at: string;
  updated_at: string;
  password_changed_at: string;
  breach_status: BreachStatus;
  strength_score: StrengthScore | null;
  password_history_count: number;
  has_passkey: boolean;
  passkey_public_key: number[] | null;
  attachments?: AttachmentInfo[];
}

export interface NewEntry {
  title: string;
  username: string;
  password: string;
  url: string;
  email: string;
  notes: string;
  tags: string[];
  totp_secret: string | null;
  custom_fields: CustomField[];
  entry_type: EntryType | null;
  generate_passkey?: boolean;
  attachments?: NewAttachment[];
}

export interface UpdateEntry {
  title?: string;
  username?: string;
  password?: string;
  url?: string;
  email?: string;
  notes?: string;
  tags?: string[];
  favorite?: boolean;
  pinned?: boolean;
  totp_secret?: string;
  custom_fields?: CustomField[];
  breach_status?: BreachStatus;
  passkey_action?: 'generate' | 'remove';
  new_attachments?: NewAttachment[];
  delete_attachment_ids?: string[];
}

export interface CustomField {
  id: string;
  name: string;
  field_type: FieldType;
  value: string;
  sensitive: boolean;
}

export type EntryType =
  | 'Login'
  | 'CreditCard'
  | 'Identity'
  | 'SecureNote'
  | 'SshKey'
  | 'ApiKey'
  | 'WifiPassword'
  | 'CryptoWallet'
  | 'Custom';

export type FieldType =
  | 'Text' | 'Password' | 'Username' | 'Email'
  | 'Url' | 'Phone' | 'Date' | 'Address'
  | 'Notes' | 'Totp' | 'File';

export type BreachStatus =
  | { type: 'Unknown' }
  | { type: 'Checking' }
  | { type: 'Safe'; checked_at: string }
  | { type: 'Breached'; breach_count: number; checked_at: string }
  | { type: 'Error'; message: string };

export interface StrengthScore {
  entropy_bits: number;
  crack_time: string;
  level: StrengthLevel;
  warnings: string[];
}

export type StrengthLevel = 'Critical' | 'Weak' | 'Fair' | 'Strong' | 'Excellent';

export interface TotpCode {
  code: string;
  seconds_remaining: number;
  period: number;
}

export interface TotpConfig {
  secret: string;
  algorithm: 'SHA1' | 'SHA256' | 'SHA512' | 'Steam';
  digits: number;
  period: number;
  issuer: string | null;
  label: string | null;
}

export interface GeneratorOptions {
  mode: 'Random' | 'Diceware';
  length: number;
  uppercase: boolean;
  lowercase: boolean;
  digits: boolean;
  symbols: boolean;
  exclude_ambiguous: boolean;
  custom_symbols: string | null;
  word_count: number;
  separator: string;
  capitalize_words: boolean;
  add_number: boolean;
}

export interface BreachResult {
  is_breached: boolean;
  breach_count: number;
  checked_at: string;
}

export interface SecurityAudit {
  total_entries: number;
  breached_count: number;
  weak_count: number;
  reused_count: number;
  old_count: number;
  no_2fa_count: number;
  health_score: number;
  issues: SecurityIssue[];
}

export interface SecurityIssue {
  entry_id: string;
  entry_title: string;
  issue_type: IssueType;
  severity: IssueSeverity;
  description: string;
}

export type IssueType = 'Breached' | 'WeakPassword' | 'ReusedPassword' | 'OldPassword' | 'Missing2FA' | 'ShortPassword';
export type IssueSeverity = 'Info' | 'Warning' | 'Critical';

export interface Tag {
  id: string;
  name: string;
  color: string;
  icon: string;
}

export interface TrashedEntryPreview {
  id: string;
  title: string;
  deleted_at: string;
  days_until_permanent: number;
}

export interface VaultStorageMetrics {
  entry_count: number;
  trashed_entry_count: number;
  tag_count: number;
  active_attachment_count: number;
  active_attachment_bytes: number;
  trashed_attachment_count: number;
  trashed_attachment_bytes: number;
  vault_file_bytes: number;
}

export interface DecryptedHistoryItem {
  password: string;
  changed_at: string;
}

export interface ParsedImportEntry {
  title: string;
  username: string;
  password: string;
  url: string;
  email: string;
  notes: string;
  totp_secret: string | null;
  tags: string[];
  is_duplicate: boolean;
  duplicate_reason?: string | null;
}

export interface ImportPreviewResult {
  format_detected: string;
  detected_format_key: string;
  is_format_mismatch: boolean;
  suggested_brand_name?: string | null;
  total_found: number;
  entries: ParsedImportEntry[];
  duplicates_count: number;
}

export interface MobileAutofillStatus {
  supported: boolean;
  enabled: boolean;
  active_provider: string;
  mapped_packages_count: number;
  strict_domain_matching: boolean;
  asset_links_enforced: boolean;
  webview_origin_protected: boolean;
  biometric_stepup_required: boolean;
}

export interface AutofillCredentialItem {
  entry_id: string;
  title: string;
  username: string;
  domain: string;
  matched_by: string;
  is_exact_package_match: boolean;
  requires_user_consent: boolean;
  requires_biometric_reauth: boolean;
}

export interface AutofillDatasetPayload {
  package_name: string;
  web_domain?: string;
  matched_credentials: AutofillCredentialItem[];
  asset_links_verified: boolean;
}

export interface DialogFilter {
  name: string;
  extensions: string[];
}

export interface OpenDialogOptions {
  title?: string;
  filters?: DialogFilter[];
  defaultPath?: string;
  multiple?: boolean;
  directory?: boolean;
  recursive?: boolean;
  canCreateDirectories?: boolean;
}

export interface SaveDialogOptions {
  title?: string;
  filters?: DialogFilter[];
  defaultPath?: string;
  canCreateDirectories?: boolean;
}




// ─── Backend Interface ──────────────────────────────────────────────────

export interface YntraVaultBackend {
  // Vault
  createVault(name: string, password: string, path: string, keyFilePath?: string): Promise<VaultInfo>;
  openVault(path: string, password: string, keyFilePath?: string): Promise<VaultInfo>;
  lockVault(): Promise<void>;
  getVaultInfo(): Promise<VaultInfo | null>;
  generateKeyFile(path: string): Promise<void>;

  // Entries
  listEntries(): Promise<EntryPreview[]>;
  searchEntries(query: string): Promise<EntryPreview[]>;
  getEntry(id: string): Promise<DecryptedEntry>;
  addEntry(entry: NewEntry): Promise<string>;
  updateEntry(id: string, update: UpdateEntry): Promise<void>;
  updateEntryBreachStatus(id: string, breachStatus: BreachStatus): Promise<void>;
  saveVault(): Promise<void>;
  reloadVault(): Promise<void>;
  deleteEntry(id: string): Promise<void>;
  toggleFavorite(id: string): Promise<boolean>;
  togglePin(id: string): Promise<boolean>;

  // Attachments
  getAttachmentData(entryId: string, attachmentId: string): Promise<number[] | Uint8Array>;
  addAttachment(entryId: string, name: string, mimeType: string, data: number[]): Promise<AttachmentInfo>;
  deleteAttachment(entryId: string, attachmentId: string): Promise<void>;

  // Trash & Compaction
  listTrash(): Promise<TrashedEntryPreview[]>;
  restoreFromTrash(id: string): Promise<void>;
  permanentDelete(id: string): Promise<void>;
  emptyTrash(): Promise<void>;
  purgeExpiredTrash(maxAgeDays?: number): Promise<number>;
  getStorageMetrics(): Promise<VaultStorageMetrics>;
  compactVault(): Promise<VaultStorageMetrics>;

  // Password History
  getPasswordHistory(entryId: string): Promise<DecryptedHistoryItem[]>;

  // TOTP
  generateTotp(secret: string): Promise<TotpCode>;
  generateTotpWithConfig(config: TotpConfig): Promise<TotpCode>;
  parseOtpauthUri(uri: string): Promise<TotpConfig>;

  // Password Generator
  generatePassword(options: GeneratorOptions): Promise<string>;
  generatePasswordDefault(): Promise<string>;

  // Breach Detection
  checkPasswordBreach(password: string): Promise<BreachResult>;
  analyzePasswordStrength(password: string): Promise<StrengthScore>;

  // Security
  securityAudit(): Promise<SecurityAudit>;
  changeMasterPassword(current: string, newPassword: string, currentKeyFile?: string, newKeyFile?: string): Promise<void>;

  // Tags
  getTags(): Promise<Tag[]>;
  addTag(name: string, color: string, icon: string): Promise<string>;
  deleteTag(id: string): Promise<void>;
  updateTag(id: string, name: string, color: string, icon: string): Promise<void>;
  reorderTags(tagIds: string[]): Promise<void>;

  // Vault File Helper
  checkVaultFileExists(path: string): Promise<boolean>;
  showInExplorer(path: string): Promise<void>;
  getInstalledApps(): Promise<import('@/types').InstalledApp[]>;

  // Advanced features
  autotype(text: string, charDelayMs: number, settleDelayMs: number): Promise<void>;
  runSmartAutotype(username: string, password: string, totpSecret: string, url: string, launchBrowser: boolean, charDelayMs: number, fieldDelayMs: number): Promise<void>;
  enableAutostart(): Promise<void>;
  disableAutostart(): Promise<void>;
  isAutostartEnabled(): Promise<boolean>;
  getFavicon(domain: string): Promise<string | null>;
  setExternalFaviconsEnabled(enabled: boolean): Promise<void>;
  isExternalFaviconsEnabled(): Promise<boolean>;
  setMinimizeToTray(enabled: boolean): Promise<void>;
  setWindowCaptureProtection(enable: boolean): Promise<void>;
  setLockOnFocusLoss(enabled: boolean): Promise<void>;
  setLockOnSystemLock(enabled: boolean): Promise<void>;
  webdavTestConnection(url: string, username: string, password: string | null): Promise<void>;
  webdavUpload(url: string, username: string, password: string | null, dbPath: string, ifMatchEtag?: string | null): Promise<string | null>;
  webdavDownload(url: string, username: string, password: string | null, destDbPath: string): Promise<void>;
  webdavSync(url: string, username: string, password: string | null): Promise<MergeStats>;
  runP2pSyncListener(listenAddr: string, dbPath: string): Promise<MergeStats>;
  runP2pSyncClient(serverAddr: string, dbPath: string, deviceId?: string): Promise<MergeStats>;
  getLocalIp(): Promise<string | null>;
  getLocalIps(): Promise<string[]>;
  scanP2pDiscovery(timeoutMs?: number | null): Promise<string | null>;
  generatePairingCode(): Promise<string>;
  getLocalDeviceInfo(): Promise<LocalDeviceInfo>;
  getTrustedDevices(): Promise<TrustedDevice[]>;
  revokeTrustedDevice(deviceId: string): Promise<void>;
  startPairingHost(listenAddr: string, password: string, pairingCode: string, deviceName?: string): Promise<PairingStats>;
  cancelPairingHost(): Promise<void>;
  startPairingClient(serverAddr: string, password: string, pairingCode: string, dbPath: string, deviceName?: string): Promise<PairingStats>;
  scanPairingDiscovery(password: string, pairingCode: string, timeoutMs?: number | null): Promise<string | null>;
  generateQrPairingSession(deviceName?: string): Promise<QrSessionInfo>;
  startQrPairingHost(password: string, includePassword: boolean, deviceName?: string): Promise<PairingStats>;
  cancelQrPairingHost(): Promise<void>;
  startQrPairingClient(qrPayload: string, deviceName?: string, password?: string): Promise<QrClientPairingResult>;
  completeAdoptedVault(password: string): Promise<PairingStats>;
  splitMasterPassword(password: string): Promise<string[]>;
  reconstructMasterPassword(shareA: string, shareB: string): Promise<string>;
  reconstructMasterPasswordHash(shareA: string, shareB: string): Promise<string>;
  generateEmergencyKit(masterPassword: string): Promise<EmergencyKit>;
  getEmergencyKitAudit(): Promise<EmergencyKitAudit | null>;
  resetEmergencyKitAudit(): Promise<void>;

  // Export & Import
  exportVault(destPath: string): Promise<void>;
  exportVaultCsv(destPath: string): Promise<void>;
  exportVaultJson(destPath: string): Promise<void>;
  getVaultPath(): Promise<string>;
  parseImportFile(filePath: string, format?: string): Promise<ImportPreviewResult>;
  parseImportContent(content: string, format?: string): Promise<ImportPreviewResult>;
  importEntries(entries: ParsedImportEntry[], duplicateStrategy: 'skip' | 'overwrite' | 'keep_both'): Promise<number>;

  // Biometrics
  checkBiometricAvailable(): Promise<BiometricInfo>;
  isBiometricEnabled(path: string): Promise<boolean>;
  unlockVaultBiometric(path: string): Promise<VaultInfo>;
  enableBiometric(): Promise<void>;
  disableBiometric(): Promise<void>;

  // Hardware 2FA / YubiKey
  checkHardware2FaAvailable(): Promise<Hardware2FaInfo>;
  listHardwareKeys(): Promise<HardwareKeyInfo[]>;
  isHardware2FaEnabled(path: string): Promise<boolean>;
  getHardware2FaChallenge(path: string): Promise<Hardware2FaChallengeInfo | null>;
  openVaultWithHardware2Fa(path: string, password: string, keyFilePath: string | undefined, hardwareResponse: number[]): Promise<VaultInfo>;
  performHardware2FaChallenge(protocol: Hardware2FaProtocol, challenge?: number[], credentialId?: number[]): Promise<number[]>;
  enableHardware2Fa(password: string, keyFilePath: string | undefined, protocol: Hardware2FaProtocol, keyName: string, challengeSalt: number[] | undefined, credentialId: number[] | undefined, hardwareResponse: number[]): Promise<void>;
  disableHardware2Fa(): Promise<void>;

  // Clipboard Defense
  copyToClipboard(text: string, isSensitive?: boolean, clearAfterSecs?: number): Promise<void>;
  clearClipboard(): Promise<void>;

  // Zero-Disclosure Native Handle IPC & Binary Secret Transport
  copyEntryPassword(entryId: string, clearAfterSecs?: number): Promise<void>;
  copyEntryUsername(entryId: string): Promise<void>;
  copyEntryTotp(entryId: string, clearAfterSecs?: number): Promise<void>;
  createVaultBytes(name: string, passwordBytes: Uint8Array | number[], path: string, keyFilePath?: string): Promise<VaultInfo>;
  openVaultBytes(path: string, passwordBytes: Uint8Array | number[], keyFilePath?: string): Promise<VaultInfo>;
  changeMasterPasswordBytes(currentBytes: Uint8Array | number[], newPasswordBytes: Uint8Array | number[], currentKeyFile?: string, newKeyFile?: string): Promise<void>;
  autotypeEntryPassword(entryId: string, charDelayMs?: number, settleDelayMs?: number): Promise<void>;
  autotypeEntrySmart(entryId: string, launchBrowser?: boolean, charDelayMs?: number, fieldDelayMs?: number): Promise<void>;
  verifyBiometric2Fa(prompt?: string): Promise<void>;

  // Mobile Native Autofill Integration
  queryMobileAutofillStatus(): Promise<MobileAutofillStatus>;
  getAutofillCredentialsForPackage(packageName: string, webDomain?: string): Promise<AutofillDatasetPayload>;

  // Smart Login
  smartLoginPrecheck(entryId?: string): Promise<SmartLoginPreCheckResult>;
  smartLoginCloseBrowser(processName: string): Promise<void>;
  smartLoginStart(entryId: string, browserIndex: number): Promise<void>;
  smartLoginCancel(): Promise<void>;
  onSmartLoginProgress(callback: (event: any) => void): Promise<() => void>;
  onSmartLoginResult(callback: (result: any) => void): Promise<() => void>;

  // File Dialogs
  openFileDialog(options?: OpenDialogOptions): Promise<string | string[] | null>;
  saveFileDialog(options?: SaveDialogOptions): Promise<string | null>;
  getMobileVaultPath(name: string): Promise<string | null>;

  // App Updates
  checkAppUpdate(customEndpoint?: string): Promise<CheckUpdateResult>;
  downloadAndInstallApk(apkUrl: string, expectedSha256?: string): Promise<string>;
  installPortableUpdate(url: string, expectedSha256?: string): Promise<void>;
  getAppVersion(): Promise<string>;
}

// ─── Smart Login Types ──────────────────────────────────────────────────

export interface SmartLoginBrowserInfo {
  name: string;
  exe_path: string;
  profile_dir: string;
  process_name: string;
  is_default: boolean;
  is_running: boolean;
}

export interface SmartLoginPreCheckResult {
  browsers: SmartLoginBrowserInfo[];
  recommended_index: number | null;
  needs_close: boolean;
  error: string | null;
}


// ─── File Dialog Abstraction ────────────────────────────────────────────

/**
 * Opens a file picker dialog.
 * In desktop (Tauri) mode, invokes `@tauri-apps/plugin-dialog`.
 * In web/mock environments, safely returns null.
 */
export async function openFileDialog(options?: OpenDialogOptions): Promise<string | string[] | null> {
  if (isTauri()) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    return open(options as any);
  }
  return null;
}

/**
 * Opens a file save dialog.
 * In desktop (Tauri) mode, invokes `@tauri-apps/plugin-dialog`.
 * In web/mock environments, safely returns null.
 */
export async function saveFileDialog(options?: SaveDialogOptions): Promise<string | null> {
  if (isTauri()) {
    const { save } = await import('@tauri-apps/plugin-dialog');
    return save(options as any);
  }
  return null;
}


// ─── Backend Detection & Factory ────────────────────────────────────────

let _backend: YntraVaultBackend | null = null;

/**
 * Detect which backend to use based on the runtime environment.
 */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/**
 * Get the active backend instance (lazy-initialized).
 */
export async function getBackend(): Promise<YntraVaultBackend> {
  if (_backend) return _backend;

  if (isTauri()) {
    const { TauriBackend } = await import('./tauri-backend');
    _backend = new TauriBackend();
  } else {
    // Web/WASM mode — for now, throw until WASM backend is implemented
    throw new Error('WASM backend not yet implemented. Run as Tauri desktop app.');
  }

  return _backend!;
}

/**
 * Reset the backend (for testing or hot-reload).
 */
export function resetBackend(): void {
  _backend = null;
}



