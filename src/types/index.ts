export interface PasswordEntry {
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
  totpSecret?: string;
  recoveryCodes?: string;
  customFields: CustomField[];
  createdAt: string;
  updatedAt: string;
  breachStatus?: import('@/lib/backend').BreachStatus;
  hasPasskey: boolean;
  passkeyPublicKey?: number[];
  generatePasskey?: boolean;
  passkeyAction?: 'generate' | 'remove';
  attachments?: AttachmentInfo[];
  newAttachments?: NewAttachment[];
  deleteAttachmentIds?: string[];
  attachmentCount?: number;
}

export interface AttachmentInfo {
  id: string;
  name: string;
  size: number;
  mimeType: string;
  createdAt: string;
}

export interface NewAttachment {
  name: string;
  mimeType: string;
  mime_type?: string;
  data: Uint8Array | number[];
}

export interface CustomField {
  id: string;
  name: string;
  type: FieldType;
  value: string;
}

export type FieldType =
  | 'text'
  | 'password'
  | 'username'
  | 'email'
  | 'url'
  | 'phone'
  | 'date'
  | 'address'
  | 'notes'
  | 'totp'
  | 'file';

export interface Tag {
  id: string;
  name: string;
  color: string;
  icon: string;
  count: number;
}

export interface Vault {
  id: string;
  name: string;
  path: string;
}

export interface KeybindShortcut {
  ctrlKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
  metaKey?: boolean;
  key: string;
}

export interface KeybindsConfig {
  search: KeybindShortcut;
  newEntry: KeybindShortcut;
  lockVault: KeybindShortcut;
  copyPassword: KeybindShortcut;
  copyUsername: KeybindShortcut;
  copyUrl: KeybindShortcut;
  copyTotp: KeybindShortcut;
  editEntry: KeybindShortcut;
  deleteEntry: KeybindShortcut;
  openUrl: KeybindShortcut;
  autotype: KeybindShortcut;
}

export interface AppSettings {
  theme: 'dark' | 'light' | 'system';
  language: string;
  sidebarWidth: number;
  passwordListWidth: number;
  fontSize: number;
  density: 'compact' | 'normal' | 'comfortable';
  autoLockMinutes: number;
  clipboardClearSeconds: number;
  minimizeToTray: boolean;
  launchOnStartup: boolean;
  disableSkeletonDelays: boolean;
  autoBreachCheck: boolean;
  showBreachInList: boolean;
  autotypeCharDelayMs: number;
  autotypeFieldDelayMs: number;
  autotypeSettleDelayMs: number;
  autotypeLaunchBrowser: boolean;
  tagSortOrder?: 'name' | 'count';
  showTagCounts?: boolean;
  entrySortOrder?: 'title' | 'updated' | 'created';
  keybinds?: KeybindsConfig;
}

export interface TOTPState {
  code: string | null;
  secret: string | null;
  secondsRemaining: number;
  period: number;
  digits: number;
}

export type FilterCategory = 'all' | 'favorites' | string; // string = tag name

export interface ToastMessage {
  id: string;
  message: string;
  type: 'success' | 'error' | 'info';
}



