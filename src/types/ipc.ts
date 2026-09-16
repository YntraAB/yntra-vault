/**
 * Tauri 2 IPC Schema Contract & Type Registry
 *
 * This file defines the strictly-typed IPC interface between the React frontend
 * and the Tauri 2 / Rust backend (yntra-vault-app and yntra-vault-core).
 *
 * It prevents silent runtime parameter mismatch, missing capabilities,
 * and untyped invoke() calls.
 */

import { invoke } from '@tauri-apps/api/core';

// ─── Core Data Models ─────────────────────────────────────────────────────────

export interface VaultInfo {
  id: string;
  name: string;
  path: string;
  entry_count: number;
  last_opened: string | null;
}

export type FieldType =
  | 'Text'
  | 'Password'
  | 'Username'
  | 'Email'
  | 'Url'
  | 'Phone'
  | 'Date'
  | 'Address'
  | 'Notes'
  | 'Totp'
  | 'File';

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

export type BreachStatus =
  | { type: 'Unknown' }
  | { type: 'Checking' }
  | { type: 'Safe'; checked_at: string }
  | { type: 'Breached'; breach_count: number; checked_at: string }
  | { type: 'Error'; message: string };

export type StrengthLevel = 'Critical' | 'Weak' | 'Fair' | 'Strong' | 'Excellent';

export interface StrengthScore {
  entropy_bits: number;
  crack_time: string;
  level: StrengthLevel;
  warnings: string[];
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
  data: number[] | Uint8Array;
}

export interface PasswordHistoryItem {
  password_encrypted: any;
  changed_at: string;
  breach_status: BreachStatus;
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
  url?: string;
  email?: string;
  notes?: string;
  tags?: string[];
  favorite?: boolean;
  pinned?: boolean;
  totp_secret?: string | null;
  custom_fields?: CustomField[];
  entry_type?: EntryType | null;
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
  totp_secret?: string | null;
  custom_fields?: CustomField[];
  entry_type?: EntryType | null;
  breach_status?: BreachStatus;
  passkey_action?: 'generate' | 'remove';
  new_attachments?: NewAttachment[];
  delete_attachment_ids?: string[];
}

export interface TrashedEntryPreview {
  id: string;
  title: string;
  deleted_at: string;
  days_until_permanent: number;
  username?: string;
  url?: string;
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
  breach_status: BreachStatus;
}

// ─── TOTP & Generator ─────────────────────────────────────────────────────────

export type TotpAlgorithm = 'SHA1' | 'SHA256' | 'SHA512' | 'Steam';

export interface TotpConfig {
  secret: string;
  algorithm: TotpAlgorithm;
  digits: number;
  period: number;
  issuer: string | null;
  label: string | null;
}

export interface TotpCode {
  code: string;
  seconds_remaining: number;
  period: number;
}

export interface GeneratorOptions {
  mode?: 'Random' | 'Diceware';
  length: number;
  uppercase: boolean;
  lowercase: boolean;
  digits: boolean;
  symbols: boolean;
  exclude_ambiguous?: boolean;
  custom_symbols?: string | null;
  word_count?: number;
  separator?: string;
  min_uppercase?: number;
  min_lowercase?: number;
  min_digits?: number;
  min_symbols?: number;
}

export interface BreachResult {
  is_breached: boolean;
  breach_count: number;
  checked_at: string;
}

export type IssueType = 'Breached' | 'WeakPassword' | 'ReusedPassword' | 'OldPassword' | 'Missing2FA' | 'ShortPassword';
export type IssueSeverity = 'Info' | 'Warning' | 'Critical';

export interface SecurityIssue {
  entry_id: string;
  entry_title: string;
  issue_type: IssueType;
  severity: IssueSeverity;
  description: string;
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

// ─── Biometrics, Hardware 2FA & Sync ──────────────────────────────────────────

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

export interface EmergencyShare {
  share_index: number;
  label: string;
  share_data: string;
}

export interface EmergencyKit {
  vault_id: string;
  vault_name: string;
  created_at: string;
  generated_at: string;
  format_version: number;
  total_entries: number;
  shares: EmergencyShare[];
  verification_hash: string;
  document_markdown: string;
}

export interface EmergencyKitAuditEntry {
  timestamp: string;
  fingerprint: string;
  action: 'generated' | 'reset' | 'invalidated' | string;
}

export interface EmergencyKitAudit {
  active_fingerprint: string;
  last_generated_at: string;
  generation_count: number;
  history: EmergencyKitAuditEntry[];
}

export interface Tag {
  id: string;
  name: string;
  color: string;
  icon: string;
  count: number;
}

export interface MergeStats {
  entries_added: number;
  entries_updated: number;
  entries_kept_local: number;
  tags_merged: number;
  trash_merged: number;
}

export interface PairingStats {
  entries_sent: number;
  entries_received: number;
  entries_merged: number;
  total_entries: number;
  vault_path?: string | null;
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

export interface InstalledApp {
  name: string;
  path: string;
  category?: string;
  is_system?: boolean;
}

export interface ParsedImportEntry {
  title: string;
  username: string;
  password: string;
  url: string;
  email: string;
  notes: string;
  totp_secret: string | null;
  custom_fields?: CustomField[];
  entry_type?: EntryType;
  tags: string[];
  is_duplicate: boolean;
  duplicate_reason?: string | null;
  duplicate_target_id?: string;
  duplicate_target_title?: string;
}

export interface ImportPreviewResult {
  format_detected: string;
  detected_format_key: string;
  is_format_mismatch: boolean;
  suggested_brand_name?: string | null;
  total_found: number;
  entries: ParsedImportEntry[];
  duplicates_count: number;
  parse_errors?: string[];
}

// ─── Tauri 2 IPC Command Contract ─────────────────────────────────────────────

export interface IpcCommands {
  // Vault Management
  create_vault: {
    args: { name: string; password: string; path: string; keyFilePath?: string | null };
    return: VaultInfo;
  };
  open_vault: {
    args: { path: string; password: string; keyFilePath?: string | null };
    return: VaultInfo;
  };
  lock_vault: {
    args: Record<string, never> | undefined;
    return: void;
  };
  get_vault_info: {
    args: Record<string, never> | undefined;
    return: VaultInfo | null;
  };
  generate_key_file: {
    args: { path: string };
    return: void;
  };
  get_vault_path: {
    args: Record<string, never> | undefined;
    return: string;
  };
  check_vault_file_exists: {
    args: { path: string };
    return: boolean;
  };
  show_in_explorer: {
    args: { path: string };
    return: void;
  };

  // Biometrics
  check_biometric_available: {
    args: Record<string, never> | undefined;
    return: BiometricInfo;
  };
  is_biometric_enabled: {
    args: { path: string };
    return: boolean;
  };
  unlock_vault_biometric: {
    args: { path: string };
    return: VaultInfo;
  };
  enable_biometric: {
    args: Record<string, never> | undefined;
    return: void;
  };
  disable_biometric: {
    args: Record<string, never> | undefined;
    return: void;
  };
  verify_biometric_2fa: {
    args: { prompt?: string | null } | undefined;
    return: void;
  };

  // Hardware 2FA
  check_hardware2fa_available: {
    args: Record<string, never> | undefined;
    return: Hardware2FaInfo;
  };
  list_hardware_keys: {
    args: Record<string, never> | undefined;
    return: HardwareKeyInfo[];
  };
  is_hardware2fa_enabled: {
    args: { path: string };
    return: boolean;
  };
  get_hardware2fa_challenge: {
    args: { path: string };
    return: Hardware2FaChallengeInfo | null;
  };
  open_vault_with_hardware2fa: {
    args: { path: string; password: string; keyFilePath?: string | null; hardwareResponse: number[] | Uint8Array };
    return: VaultInfo;
  };
  perform_hardware2fa_challenge: {
    args: { protocol: string; challenge?: number[] | null; credentialId?: number[] | null };
    return: number[];
  };
  enable_hardware2fa: {
    args: {
      password: string;
      keyFilePath?: string | null;
      protocol: string;
      keyName: string;
      challengeSalt?: number[] | null;
      credentialId?: number[] | null;
      hardwareResponse: number[] | Uint8Array;
    };
    return: void;
  };
  disable_hardware2fa: {
    args: Record<string, never> | undefined;
    return: void;
  };

  // Entries
  list_entries: {
    args: Record<string, never> | undefined;
    return: EntryPreview[];
  };
  search_entries: {
    args: { query: string };
    return: EntryPreview[];
  };
  get_entry: {
    args: { id: string };
    return: DecryptedEntry;
  };
  add_entry: {
    args: { entry: NewEntry };
    return: string;
  };
  update_entry: {
    args: { id: string; update: UpdateEntry };
    return: void;
  };
  update_entry_breach_status: {
    args: { id: string; breachStatus: any };
    return: void;
  };
  save_vault: {
    args: Record<string, never> | undefined;
    return: void;
  };
  reload_vault: {
    args: Record<string, never> | undefined;
    return: void;
  };
  delete_entry: {
    args: { id: string };
    return: void;
  };
  toggle_favorite: {
    args: { id: string };
    return: boolean;
  };
  toggle_pin: {
    args: { id: string };
    return: boolean;
  };

  // Attachments
  get_attachment_data: {
    args: { entryId: string; attachmentId: string };
    return: ArrayBuffer | Uint8Array | number[];
  };
  add_attachment: {
    args: { entryId: string; name: string; mimeType: string; data: number[] | Uint8Array };
    return: AttachmentInfo;
  };
  delete_attachment: {
    args: { entryId: string; attachmentId: string };
    return: void;
  };

  // Trash
  list_trash: {
    args: Record<string, never> | undefined;
    return: TrashedEntryPreview[];
  };
  restore_from_trash: {
    args: { id: string };
    return: void;
  };
  permanent_delete: {
    args: { id: string };
    return: void;
  };
  empty_trash: {
    args: Record<string, never> | undefined;
    return: void;
  };
  purge_expired_trash: {
    args: { maxAgeDays?: number } | undefined;
    return: number;
  };
  get_storage_metrics: {
    args: Record<string, never> | undefined;
    return: VaultStorageMetrics;
  };
  compact_vault: {
    args: Record<string, never> | undefined;
    return: VaultStorageMetrics;
  };

  // Password History
  get_password_history: {
    args: { entryId: string };
    return: DecryptedHistoryItem[];
  };

  // TOTP & Tools
  generate_totp: {
    args: { secret: string };
    return: TotpCode;
  };
  generate_totp_with_config: {
    args: { config: TotpConfig };
    return: TotpCode;
  };
  parse_otpauth_uri: {
    args: { uri: string };
    return: TotpConfig;
  };
  generate_password: {
    args: { options: GeneratorOptions };
    return: string;
  };
  generate_password_default: {
    args: Record<string, never> | undefined;
    return: string;
  };
  check_password_breach: {
    args: { password: string };
    return: BreachResult;
  };
  analyze_password_strength: {
    args: { password: string };
    return: StrengthScore;
  };
  security_audit: {
    args: Record<string, never> | undefined;
    return: SecurityAudit;
  };
  change_master_password: {
    args: { current: string; newPassword: string; currentKeyFile?: string | null; newKeyFile?: string | null };
    return: void;
  };
  generate_emergency_kit: {
    args: { masterPassword: string };
    return: EmergencyKit;
  };

  // Tags
  get_tags: {
    args: Record<string, never> | undefined;
    return: Tag[];
  };
  add_tag: {
    args: { name: string; color: string; icon: string };
    return: string;
  };
  delete_tag: {
    args: { id: string };
    return: void;
  };
  update_tag: {
    args: { id: string; name: string; color: string; icon: string };
    return: void;
  };
  reorder_tags: {
    args: { tagIds: string[] };
    return: void;
  };

  // Autotype & System Integration
  autotype: {
    args: { text: string; charDelayMs?: number; settleDelayMs?: number };
    return: void;
  };
  run_smart_autotype: {
    args: { username: string; password: string; totpSecret: string; url: string; launchBrowser: boolean; charDelayMs: number; fieldDelayMs: number };
    return: void;
  };
  enable_autostart: {
    args: Record<string, never> | undefined;
    return: void;
  };
  disable_autostart: {
    args: Record<string, never> | undefined;
    return: void;
  };
  is_autostart_enabled: {
    args: Record<string, never> | undefined;
    return: boolean;
  };
  get_favicon: {
    args: { domain: string };
    return: string | null;
  };
  set_external_favicons_enabled: {
    args: { enabled: boolean };
    return: void;
  };
  is_external_favicons_enabled: {
    args: Record<string, never> | undefined;
    return: boolean;
  };
  set_minimize_to_tray: {
    args: { enabled: boolean };
    return: void;
  };
  get_installed_apps: {
    args: Record<string, never> | undefined;
    return: InstalledApp[];
  };
  set_window_capture_protection: {
    args: { enable: boolean };
    return: void;
  };
  set_lock_on_focus_loss: {
    args: { enabled: boolean };
    return: void;
  };
  set_lock_on_system_lock: {
    args: { enabled: boolean };
    return: void;
  };

  // Sync & WebDAV
  webdav_upload: {
    args: { url: string; username: string; password?: string | null; dbPath: string; ifMatchEtag?: string | null };
    return: string | null;
  };
  webdav_download: {
    args: { url: string; username: string; password?: string | null; destDbPath: string };
    return: void;
  };
  webdav_sync: {
    args: { url: string; username: string; password?: string | null };
    return: MergeStats;
  };
  webdav_test_connection: {
    args: { url: string; username: string; password?: string | null };
    return: void;
  };
  run_p2p_sync_listener: {
    args: { listenAddr: string; dbPath: string };
    return: MergeStats;
  };
  run_p2p_sync_client: {
    args: { serverAddr: string; dbPath: string; deviceId?: string };
    return: MergeStats;
  };
  get_local_ip: {
    args: Record<string, never>;
    return: string | null;
  };
  scan_p2p_discovery: {
    args: { timeoutMs?: number | null };
    return: string | null;
  };
  generate_pairing_code: {
    args?: Record<string, never>;
    return: string;
  };
  get_trusted_devices: {
    args?: Record<string, never>;
    return: TrustedDevice[];
  };
  revoke_trusted_device: {
    args: { deviceId: string };
    return: void;
  };
  start_pairing_host: {
    args: { listenAddr: string; password: string; pairingCode: string; deviceName?: string };
    return: PairingStats;
  };
  start_pairing_client: {
    args: { serverAddr: string; password: string; pairingCode: string; dbPath: string; deviceName?: string };
    return: PairingStats;
  };
  scan_pairing_discovery: {
    args: { password: string; pairingCode: string; timeoutMs?: number | null };
    return: string | null;
  };

  // Shamir Secret Sharing
  split_master_password: {
    args: { password: string };
    return: string[];
  };
  reconstruct_master_password: {
    args: { shareA: string; shareB: string };
    return: string;
  };
  reconstruct_master_password_hash: {
    args: { shareA: string; shareB: string };
    return: string;
  };
  get_emergency_kit_audit: {
    args?: Record<string, never>;
    return: EmergencyKitAudit | null;
  };
  reset_emergency_kit_audit: {
    args?: Record<string, never>;
    return: void;
  };

  // Export & Import
  export_vault: {
    args: { destPath: string };
    return: void;
  };
  export_vault_csv: {
    args: { destPath: string };
    return: void;
  };
  export_vault_json: {
    args: { destPath: string };
    return: void;
  };
  query_mobile_autofill_status: {
    args: Record<string, never> | undefined;
    return: any;
  };
  get_autofill_credentials_for_package: {
    args: { packageName: string; webDomain?: string | null };
    return: any;
  };
  parse_import_file: {
    args: { filePath: string; format?: string | null };
    return: ImportPreviewResult;
  };
  parse_import_content: {
    args: { content: string; format?: string | null };
    return: ImportPreviewResult;
  };
  import_entries: {
    args: { entries: ParsedImportEntry[]; duplicateStrategy: string };
    return: number;
  };

  // Clipboard & Zero-Disclosure Operations
  copy_to_clipboard: {
    args: { text: string; isSensitive?: boolean | null; clearAfterSecs?: number | null };
    return: void;
  };
  clear_clipboard: {
    args: Record<string, never> | undefined;
    return: void;
  };
  copy_entry_password: {
    args: { entryId: string; clearAfterSecs?: number | null };
    return: void;
  };
  copy_entry_username: {
    args: { entryId: string };
    return: void;
  };
  copy_entry_totp: {
    args: { entryId: string; clearAfterSecs?: number | null };
    return: void;
  };
  create_vault_bytes: {
    args: { name: string; passwordBytes: number[]; path: string; keyFilePath?: string | null };
    return: VaultInfo;
  };
  open_vault_bytes: {
    args: { path: string; passwordBytes: number[]; keyFilePath?: string | null };
    return: VaultInfo;
  };
  change_master_password_bytes: {
    args: { currentBytes: number[]; newPasswordBytes: number[]; currentKeyFile?: string | null; newKeyFile?: string | null };
    return: void;
  };
  autotype_entry_password: {
    args: { entryId: string; charDelayMs?: number | null; settleDelayMs?: number | null };
    return: void;
  };
  autotype_entry_smart: {
    args: { entryId: string; launchBrowser?: boolean | null; charDelayMs?: number | null; fieldDelayMs?: number | null };
    return: void;
  };
  smart_login_precheck: {
    args: Record<string, never> | undefined;
    return: import('@/lib/backend').SmartLoginPreCheckResult;
  };
  smart_login_close_browser: {
    args: { processName: string };
    return: void;
  };
  smart_login_start: {
    args: { entryId: string; browserIndex: number };
    return: void;
  };
  smart_login_cancel: {
    args: Record<string, never> | undefined;
    return: void;
  };
}

export type IpcCommandName = keyof IpcCommands;

export type IpcCommandArgs<T extends IpcCommandName> = IpcCommands[T]['args'];

export type IpcCommandReturn<T extends IpcCommandName> = IpcCommands[T]['return'];

/**
 * Type-safe IPC invoke function ensuring that command names,
 * argument parameters, and return types strictly match the backend contract.
 */
export async function invokeIpc<T extends IpcCommandName>(
  command: T,
  ...args: undefined extends IpcCommandArgs<T>
    ? [args?: IpcCommandArgs<T>]
    : [args: IpcCommandArgs<T>]
): Promise<IpcCommandReturn<T>> {
  return invoke(command, args[0] as any);
}
