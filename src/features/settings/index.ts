export { SettingsPanel, default as SettingsPanelDefault } from './components/SettingsPanel';
export { SettingSection, SettingRow, Toggle } from './components/SettingSection';
export type { SettingSectionProps, SettingRowProps } from './components/SettingSection';
export { LanguageCombobox } from './components/LanguageCombobox';
export type { LanguageComboboxProps } from './components/LanguageCombobox';
export { GeneralTab } from './components/GeneralTab';
export type { GeneralTabProps } from './components/GeneralTab';
export { AppearanceTab } from './components/AppearanceTab';
export { KeybindsTab } from './components/KeybindsTab';
export { SecurityTab } from './components/SecurityTab';
export type { SecurityTabProps } from './components/SecurityTab';
export { BackupTab } from './components/BackupTab';
export type { BackupTabProps } from './components/BackupTab';
export { TrashTab } from './components/TrashTab';
export { DeleteTrashModal } from './components/DeleteTrashModal';
export type { DeleteTrashModalProps } from './components/DeleteTrashModal';

export {
  SettingsProvider,
  useSettings,
  DEFAULT_SETTINGS,
} from './context/SettingsContext';
export type { SettingsContextType } from './context/SettingsContext';
