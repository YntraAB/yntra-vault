import type { ReactElement } from 'react';
import { Image, FileText, FileArchive, Paperclip } from 'lucide-react';

export function formatBytes(bytes: number, decimals = 1): string {
  if (bytes === 0) return '0 Bytes';
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ['Bytes', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
}

export function getAttachmentIcon(mimeType: string | undefined = '', fileName: string = ''): ReactElement {
  const mime = (mimeType || '').toLowerCase();
  const name = fileName || '';
  const ext = name.split('.').pop()?.toLowerCase() || '';
  if (mime.startsWith('image/') || ['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext)) {
    return <Image size={14} className="text-[var(--text-secondary)] shrink-0" />;
  }
  if (mime.startsWith('text/') || ['txt', 'md', 'json', 'csv', 'log', 'xml'].includes(ext)) {
    return <FileText size={14} className="text-[var(--text-secondary)] shrink-0" />;
  }
  if (mime.includes('zip') || mime.includes('tar') || ['zip', '7z', 'rar', 'gz', 'tar'].includes(ext)) {
    return <FileArchive size={14} className="text-[var(--text-secondary)] shrink-0" />;
  }
  return <Paperclip size={14} className="text-[var(--text-secondary)] shrink-0" />;
}

export { formatDate, formatTime } from './utils';
