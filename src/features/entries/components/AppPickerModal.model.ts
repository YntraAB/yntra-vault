/**
 * AppPickerModal — SOTA Installed Application Picker & Categorized Filter
 *
 * Scans installed desktop applications, filters out non-login OS utilities,
 * provides category tabs, and fallbacks to manual OS file browser.
 */

import type { InstalledApp } from '@/types';


export interface AppPickerModalProps {
  open: boolean;
  onClose: () => void;
  onSelectApp: (appPath: string) => void;
}


export type AppCategoryTab = 'loggable' | 'all' | 'browsers_communication' | 'productivity_dev' | 'gaming' | 'system';


export const COMMON_APPS: InstalledApp[] = [
  // Browsers & Communication
  { name: 'Google Chrome', path: 'googlechrome://', category: 'browsers_communication', is_system: false },
  { name: 'Mozilla Firefox', path: 'firefox://', category: 'browsers_communication', is_system: false },
  { name: 'Discord', path: 'discord://', category: 'browsers_communication', is_system: false },
  { name: 'Slack', path: 'slack://', category: 'browsers_communication', is_system: false },
  { name: 'Telegram', path: 'tg://', category: 'browsers_communication', is_system: false },
  { name: 'WhatsApp', path: 'whatsapp://', category: 'browsers_communication', is_system: false },
  { name: 'Signal', path: 'sgnl://', category: 'browsers_communication', is_system: false },
  { name: 'Microsoft Teams', path: 'msteams://', category: 'browsers_communication', is_system: false },
  { name: 'Thunderbird', path: 'thunderbird://', category: 'browsers_communication', is_system: false },

  // Dev & Productivity
  { name: 'Visual Studio Code', path: 'vscode://', category: 'productivity_dev', is_system: false },
  { name: 'Cursor', path: 'cursor://', category: 'productivity_dev', is_system: false },
  { name: 'Windsurf', path: 'windsurf://', category: 'productivity_dev', is_system: false },
  { name: 'Antigravity', path: 'antigravity://', category: 'productivity_dev', is_system: false },
  { name: 'Obsidian', path: 'obsidian://', category: 'productivity_dev', is_system: false },
  { name: 'Notion', path: 'notion://', category: 'productivity_dev', is_system: false },
  { name: 'Linear', path: 'linear://', category: 'productivity_dev', is_system: false },
  { name: 'Docker Desktop', path: 'docker://', category: 'productivity_dev', is_system: false },
  { name: 'Postman', path: 'postman://', category: 'productivity_dev', is_system: false },
  { name: 'DBeaver', path: 'dbeaver://', category: 'productivity_dev', is_system: false },
  { name: 'GitKraken', path: 'gitkraken://', category: 'productivity_dev', is_system: false },
  { name: 'Figma', path: 'figma://', category: 'productivity_dev', is_system: false },

  // Gaming & Media
  { name: 'Steam', path: 'steam://', category: 'gaming', is_system: false },
  { name: 'Spotify', path: 'spotify://', category: 'gaming', is_system: false },
  { name: 'NVIDIA App', path: 'nvidia://', category: 'gaming', is_system: false },
  { name: 'Epic Games Store', path: 'com.epicgames.launcher://', category: 'gaming', is_system: false },
  { name: 'Battle.net', path: 'battlenet://', category: 'gaming', is_system: false },
  { name: 'Roblox', path: 'roblox://', category: 'gaming', is_system: false },
  { name: 'VLC Media Player', path: 'vlc://', category: 'gaming', is_system: false },
];