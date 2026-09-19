// Public components
export { PasswordList, default as PasswordListDefault } from './components/PasswordList';
export type { PasswordListProps } from './components/PasswordList';

export { PasswordDetail, default as PasswordDetailDefault } from './components/PasswordDetail';
export { EntryModal, default as EntryModalDefault } from './components/EntryModal';
export { BulkEditModal, default as BulkEditModalDefault } from './components/BulkEditModal';
export type { BulkEditModalProps } from './components/BulkEditModal';

export { DeleteEntryModal, default as DeleteEntryModalDefault } from './components/DeleteEntryModal';
export type { DeleteEntryModalProps } from './components/DeleteEntryModal';

export { Favicon, default as FaviconDefault } from './components/Favicon';
export type { FaviconProps } from './components/Favicon';

export { TOTPDisplay, default as TOTPDisplayDefault } from './components/TOTPDisplay';
export type { TOTPDisplayProps } from './components/TOTPDisplay';

export { AutotypeButton, default as AutotypeButtonDefault } from './components/AutotypeButton';
export type { AutotypeButtonProps } from './components/AutotypeButton';

export { default as SmartLoginButton } from './components/SmartLoginButton';
export { default as SmartLoginModal } from './components/SmartLoginModal';

export { AttachmentPreviewModal, default as AttachmentPreviewModalDefault } from './components/AttachmentPreviewModal';
export type { AttachmentPreviewModalProps } from './components/AttachmentPreviewModal';

export { AppPickerModal, default as AppPickerModalDefault } from './components/AppPickerModal';
export type { AppPickerModalProps, AppCategoryTab } from './components/AppPickerModal';

export { EntryContextMenu, default as EntryContextMenuDefault } from './components/EntryContextMenu';
export type { EntryContextMenuProps } from './components/EntryContextMenu';

export { PasswordListAreaContextMenu, default as PasswordListAreaContextMenuDefault } from './components/PasswordListAreaContextMenu';
export type { PasswordListAreaContextMenuProps } from './components/PasswordListAreaContextMenu';

export { TagContextMenu, default as TagContextMenuDefault } from './components/TagContextMenu';
export type { TagContextMenuProps } from './components/TagContextMenu';

export { TagsAreaContextMenu, default as TagsAreaContextMenuDefault } from './components/TagsAreaContextMenu';
export type { TagsAreaContextMenuProps } from './components/TagsAreaContextMenu';

export { CreateTagModal, default as CreateTagModalDefault, PRESET_COLORS } from './components/CreateTagModal';
export type { CreateTagModalProps } from './components/CreateTagModal';

export { EditTagModal, default as EditTagModalDefault } from './components/EditTagModal';
export type { EditTagModalProps } from './components/EditTagModal';

export { DeleteTagModal, default as DeleteTagModalDefault } from './components/DeleteTagModal';
export type { DeleteTagModalProps } from './components/DeleteTagModal';

export { DeleteUnusedTagsModal, default as DeleteUnusedTagsModalDefault } from './components/DeleteUnusedTagsModal';
export type { DeleteUnusedTagsModalProps } from './components/DeleteUnusedTagsModal';

// Context & Hooks
export {
  EntriesProvider,
  useEntries,
  useFilteredEntries,
  entryPreviewToPasswordEntry,
  decryptedEntryToPasswordEntry,
  isRecoveryField,
} from './context/EntriesContext';
export type { EntriesContextType } from './context/EntriesContext';

export { useEntry } from './hooks/useEntry';
export { useTotp } from './hooks/useTotp';
