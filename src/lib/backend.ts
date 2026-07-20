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

// ─── Types ──────────────────────────────────────────────────────────────

export interface VaultInfo {
  id: string;
  name: string;
  path: string;
  entry_count: number;
  last_opened: string | null;
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
  algorithm: 'SHA1' | 'SHA256' | 'SHA512';
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

export interface DecryptedHistoryItem {
  password: string;
  changed_at: string;
}

// ─── Backend Interface ──────────────────────────────────────────────────

export interface YntraVaultBackend {
  // Vault
  createVault(name: string, password: string, path: string): Promise<VaultInfo>;
  openVault(path: string, password: string): Promise<VaultInfo>;
  lockVault(): Promise<void>;
  getVaultInfo(): Promise<VaultInfo | null>;

  // Entries
  listEntries(): Promise<EntryPreview[]>;
  searchEntries(query: string): Promise<EntryPreview[]>;
  getEntry(id: string): Promise<DecryptedEntry>;
  addEntry(entry: NewEntry): Promise<string>;
  updateEntry(id: string, update: UpdateEntry): Promise<void>;
  deleteEntry(id: string): Promise<void>;
  toggleFavorite(id: string): Promise<boolean>;
  togglePin(id: string): Promise<boolean>;

  // Trash
  listTrash(): Promise<TrashedEntryPreview[]>;
  restoreFromTrash(id: string): Promise<void>;
  permanentDelete(id: string): Promise<void>;

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
  changeMasterPassword(current: string, newPassword: string): Promise<void>;

  // Tags
  getTags(): Promise<Tag[]>;
  addTag(name: string, color: string, icon: string): Promise<string>;
  deleteTag(id: string): Promise<void>;
  updateTag(id: string, name: string, color: string, icon: string): Promise<void>;

  // Vault File Helper
  checkVaultFileExists(path: string): Promise<boolean>;
  showInExplorer(path: string): Promise<void>;

  // Advanced features
  autotype(text: string, charDelayMs: number, settleDelayMs: number): Promise<void>;
  runSmartAutotype(username: string, password: string, totpSecret: string, url: string, launchBrowser: boolean, charDelayMs: number, fieldDelayMs: number): Promise<void>;
  enableAutostart(): Promise<void>;
  disableAutostart(): Promise<void>;
  isAutostartEnabled(): Promise<boolean>;
  setMinimizeToTray(enabled: boolean): Promise<void>;
  webdavUpload(url: string, username: string, password: string | null, dbPath: string): Promise<void>;
  webdavDownload(url: string, username: string, password: string | null, destDbPath: string): Promise<void>;
  runP2pSyncListener(listenAddr: string, dbPath: string): Promise<void>;
  runP2pSyncClient(serverAddr: string, dbPath: string): Promise<void>;
  splitMasterPassword(password: string): Promise<string[]>;
  reconstructMasterPasswordHash(shareA: string, shareB: string): Promise<string>;

  // Export
  exportVault(destPath: string): Promise<void>;
  getVaultPath(): Promise<string>;

  // Browser Extension
  installBrowserExtension(): Promise<string>;
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

  return _backend;
}

/**
 * Reset the backend (for testing or hot-reload).
 */
export function resetBackend(): void {
  _backend = null;
}



