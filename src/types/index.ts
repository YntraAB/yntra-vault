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

export interface AppSettings {
  theme: 'dark' | 'light' | 'system';
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

