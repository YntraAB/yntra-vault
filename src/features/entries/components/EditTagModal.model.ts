/**
 * EditTagModal — Edit or delete an existing tag
 *
 * Pre-filled name/color, delete with confirmation.
 * Same modal pattern as CreateTagModal.
 */

import type { Tag } from '@/types';


export interface EditTagModalProps {
  open: boolean;
  onClose: () => void;
  tag: Tag | null;
}


export const PRESET_COLORS = [
  '#5b8def', '#5acf7e', '#f5a623', '#bd7ee8', '#ef6b6b',
  '#4ecdc4', '#f78fb3', '#778beb', '#e77f67', '#63cdda',
];