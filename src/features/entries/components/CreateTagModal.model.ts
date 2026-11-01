/**
 * CreateTagModal — Create a new tag (vault category)
 *
 * Name input, color picker (preset palette), validation.
 * Same modal pattern as CreateVaultModal / EntryModal.
 */



export interface CreateTagModalProps {
  open: boolean;
  onClose: () => void;
}


export const PRESET_COLORS = [
  '#5b8def', // Primary blue
  '#4ade80', // Green
  '#f59e0b', // Amber
  '#a855f7', // Purple
  '#ef6b6b', // Red
  '#4ecdc4', // Teal
  '#f78fb3', // Pink
  '#778beb', // Indigo
  '#e77f67', // Coral
  '#63cdda', // Cyan
];