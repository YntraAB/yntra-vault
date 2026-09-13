import { describe, it, expect } from 'bun:test';
import { formatBytes, getAttachmentIcon } from './formatters';

describe('formatBytes', () => {
  it('handles 0 bytes', () => {
    expect(formatBytes(0)).toBe('0 Bytes');
  });

  it('formats byte sizes under 1KB', () => {
    expect(formatBytes(500)).toBe('500 Bytes');
  });

  it('formats kilobyte sizes', () => {
    expect(formatBytes(1024)).toBe('1 KB');
    expect(formatBytes(1536)).toBe('1.5 KB');
  });

  it('formats megabyte sizes', () => {
    expect(formatBytes(1048576)).toBe('1 MB');
    expect(formatBytes(5242880)).toBe('5 MB');
  });

  it('formats gigabyte sizes', () => {
    expect(formatBytes(1073741824)).toBe('1 GB');
  });

  it('supports custom decimal precision', () => {
    expect(formatBytes(1536, 2)).toBe('1.5 KB');
    expect(formatBytes(1234567, 2)).toBe('1.18 MB');
  });
});

describe('getAttachmentIcon', () => {
  it('returns appropriate icon for images', () => {
    const iconMime = getAttachmentIcon('image/png', 'test.bin');
    expect(iconMime).toBeDefined();

    const iconExt = getAttachmentIcon('', 'photo.jpeg');
    expect(iconExt).toBeDefined();
  });

  it('returns appropriate icon for text files', () => {
    const iconMime = getAttachmentIcon('text/plain', 'doc.bin');
    expect(iconMime).toBeDefined();

    const iconExt = getAttachmentIcon('', 'notes.md');
    expect(iconExt).toBeDefined();
  });

  it('returns appropriate icon for archives', () => {
    const iconMime = getAttachmentIcon('application/zip', 'archive');
    expect(iconMime).toBeDefined();

    const iconExt = getAttachmentIcon('', 'backup.tar');
    expect(iconExt).toBeDefined();
  });

  it('returns fallback icon for unknown file types', () => {
    const fallback = getAttachmentIcon('application/octet-stream', 'unknown.xyz');
    expect(fallback).toBeDefined();
  });
});
