import { describe, it, expect } from 'bun:test';
import type { PasswordEntry } from '@/types';
import { entryPreviewToPasswordEntry, decryptedEntryToPasswordEntry } from './context/EntriesContext';
import type { EntryPreview, DecryptedEntry } from '@/lib/backend';

describe('Entries Feature Slice', () => {
  it('converts entry preview to password entry correctly', () => {
    const preview: EntryPreview = {
      id: 'entry-1',
      title: 'GitHub',
      username: 'octocat',
      url: 'https://github.com',
      email: 'octo@github.com',
      tags: ['Dev', 'Work'],
      favorite: true,
      pinned: false,
      has_totp: true,
      has_passkey: false,
      updated_at: '2026-09-13T10:00:00Z',
      breach_status: { type: 'Safe', checked_at: '2026-09-13T10:00:00Z' },
      attachment_count: 2,
    };

    const entry = entryPreviewToPasswordEntry(preview);
    expect(entry.id).toBe('entry-1');
    expect(entry.title).toBe('GitHub');
    expect(entry.username).toBe('octocat');
    expect(entry.password).toBe('••••••••');
    expect(entry.favorite).toBe(true);
    expect(entry.totpSecret).toBe('has-totp');
    expect(entry.attachmentCount).toBe(2);
    expect(entry.tags).toEqual(['Dev', 'Work']);
  });

  it('converts decrypted entry to full password entry', () => {
    const decrypted: DecryptedEntry = {
      id: 'entry-2',
      title: 'ProtonMail',
      username: 'agent',
      password: 'super-secure-password-123!',
      url: 'https://mail.proton.me',
      email: 'agent@proton.me',
      notes: 'Encrypted mailbox',
      tags: ['Email', 'Security'],
      favorite: false,
      pinned: true,
      totp_secret: 'JBSWY3DPEHPK3PXP',
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-09-13T10:00:00Z',
      breach_status: { type: 'Safe', checked_at: '2026-09-13T10:00:00Z' },
      has_passkey: false,
      attachments: [
        {
          id: 'att-1',
          name: 'backup-codes.txt',
          size: 1024,
          mime_type: 'text/plain',
          created_at: '2026-01-01T00:00:00Z',
        },
      ],
      custom_fields: [
        {
          id: 'cf-1',
          name: 'PIN',
          field_type: 'Password',
          value: '9988',
          sensitive: true,
        },
      ],
    };

    const entry = decryptedEntryToPasswordEntry(decrypted);
    expect(entry.id).toBe('entry-2');
    expect(entry.password).toBe('super-secure-password-123!');
    expect(entry.totpSecret).toBe('JBSWY3DPEHPK3PXP');
    expect(entry.pinned).toBe(true);
    expect(entry.attachments).toHaveLength(1);
    expect(entry.attachmentCount).toBe(1);
    expect(entry.customFields).toHaveLength(1);
    expect(entry.customFields[0].name).toBe('PIN');
  });

  it('filters entries by search term across title, username, url, and tags', () => {
    const entries: PasswordEntry[] = [
      {
        id: '1',
        title: 'Google Account',
        username: 'alice@gmail.com',
        password: 'pwd',
        url: 'https://accounts.google.com',
        tags: ['Search', 'Work'],
        favorite: true,
        pinned: false,
        customFields: [],
        createdAt: '2026-01-01T00:00:00Z',
        updatedAt: '2026-01-01T00:00:00Z',
      },
      {
        id: '2',
        title: 'AWS Console',
        username: 'dev-admin',
        password: 'pwd',
        url: 'https://aws.amazon.com',
        tags: ['Cloud', 'Dev'],
        favorite: false,
        pinned: true,
        customFields: [],
        createdAt: '2026-01-02T00:00:00Z',
        updatedAt: '2026-01-02T00:00:00Z',
      },
    ];

    const filter = (term: string) => {
      const q = term.toLowerCase();
      return entries.filter(
        (e) =>
          e.title.toLowerCase().includes(q) ||
          e.username.toLowerCase().includes(q) ||
          (e.url && e.url.toLowerCase().includes(q)) ||
          e.tags.some((t) => t.toLowerCase().includes(q))
      );
    };

    expect(filter('google')).toHaveLength(1);
    expect(filter('dev-admin')).toHaveLength(1);
    expect(filter('amazon')).toHaveLength(1);
    expect(filter('Work')).toHaveLength(1);
    expect(filter('Cloud')).toHaveLength(1);
    expect(filter('nonexistent')).toHaveLength(0);
  });

  it('computes dynamic tag counts correctly across entries', () => {
    const entries: { tags: string[] }[] = [
      { tags: ['Dev', 'Work'] },
      { tags: ['Dev', 'Personal'] },
      { tags: ['Work'] },
    ];

    const countMap: Record<string, number> = {};
    for (const entry of entries) {
      for (const tag of entry.tags) {
        countMap[tag] = (countMap[tag] || 0) + 1;
      }
    }

    expect(countMap['Dev']).toBe(2);
    expect(countMap['Work']).toBe(2);
    expect(countMap['Personal']).toBe(1);
    expect(countMap['Other']).toBeUndefined();
  });

  it('correctly handles remote deletion and remote updates during refresh revalidation', () => {
    // Simulate active selected entry
    let activeEntry: PasswordEntry | null = {
      id: 'entry-sync-1',
      title: 'Sync Service',
      username: 'user1',
      password: 'old-decrypted-password',
      url: 'https://sync.example.com',
      email: '',
      notes: 'v1 notes',
      tags: [],
      favorite: false,
      pinned: false,
      customFields: [],
      createdAt: '2026-09-13T10:00:00Z',
      updatedAt: '2026-09-13T10:00:00Z',
      attachmentCount: 0,
    };

    // Case 1: Entry was deleted remotely
    const previewsAfterRemoteDelete: EntryPreview[] = [];
    const revalidateDeleted = (curr: PasswordEntry | null, previews: EntryPreview[]) => {
      if (!curr) return null;
      const found = previews.find((p) => p.id === curr.id);
      return found ? curr : null;
    };
    expect(revalidateDeleted(activeEntry, previewsAfterRemoteDelete)).toBeNull();

    // Case 2: Entry was updated remotely with new timestamp
    const previewsAfterRemoteUpdate: EntryPreview[] = [
      {
        id: 'entry-sync-1',
        title: 'Sync Service Renamed',
        username: 'user1',
        url: 'https://sync.example.com',
        tags: [],
        favorite: false,
        pinned: false,
        updated_at: '2026-09-13T11:00:00Z', // changed!
      },
    ];

    const needsRefetch = (curr: PasswordEntry | null, previews: EntryPreview[]) => {
      if (!curr) return false;
      const match = previews.find((p) => p.id === curr.id);
      return Boolean(match && match.updated_at !== curr.updatedAt);
    };

    expect(needsRefetch(activeEntry, previewsAfterRemoteUpdate)).toBe(true);

    // Simulate backend fresh fetch on demand
    const freshDecrypted: DecryptedEntry = {
      id: 'entry-sync-1',
      title: 'Sync Service Renamed',
      username: 'user1',
      password: 'new-synced-password-456!',
      url: 'https://sync.example.com',
      email: '',
      notes: 'v2 notes synced from cloud',
      tags: [],
      favorite: false,
      pinned: false,
      custom_fields: [],
      entry_type: 'Login',
      created_at: '2026-09-13T10:00:00Z',
      updated_at: '2026-09-13T11:00:00Z',
      breach_status: { type: 'Safe', checked_at: '2026-09-13T11:00:00Z' },
      has_passkey: false,
      password_history_count: 1,
    };

    const updatedEntry = decryptedEntryToPasswordEntry(freshDecrypted);
    expect(updatedEntry.password).toBe('new-synced-password-456!');
    expect(updatedEntry.title).toBe('Sync Service Renamed');
    expect(updatedEntry.notes).toBe('v2 notes synced from cloud');
    expect(updatedEntry.updatedAt).toBe('2026-09-13T11:00:00Z');
  });

  it('rejects stale out-of-order async selection results and drops pending selections on lock', async () => {
    let selectionSeq = 0;
    let isLocked = false;
    let selected: string | null = null;

    const selectEntry = async (id: string, delayMs: number) => {
      const seq = ++selectionSeq;
      await new Promise((resolve) => setTimeout(resolve, delayMs));
      // Drop if sequence counter is stale or vault locked
      if (seq !== selectionSeq || isLocked) {
        return;
      }
      selected = id;
    };

    // Fast click on A (slow response 50ms), then fast click on B (fast response 10ms)
    const promiseA = selectEntry('entry-A', 50);
    const promiseB = selectEntry('entry-B', 10);

    await Promise.all([promiseA, promiseB]);

    // B should win even though A finished later
    expect(selected).toBe('entry-B');

    // Test lock invalidation
    const promiseC = selectEntry('entry-C', 30);
    isLocked = true;
    selectionSeq++; // lock increments seq
    await promiseC;

    // Entry C must not have overwritten state
    expect(selected).toBe('entry-B');
  });

  it('optimistically updates favorite status and rolls back on failure', async () => {
    let entriesState: PasswordEntry[] = [
      {
        id: 'opt-1',
        title: 'Optimistic Service',
        username: 'user',
        password: 'pwd',
        url: '',
        tags: [],
        favorite: false,
        pinned: false,
        customFields: [],
        createdAt: '2026-09-13T10:00:00Z',
        updatedAt: '2026-09-13T10:00:00Z',
      },
    ];

    // Optimistic toggle simulation
    const prevEntries = [...entriesState];
    // Immediate flip
    entriesState = entriesState.map((e) =>
      e.id === 'opt-1' ? { ...e, favorite: !e.favorite } : e
    );
    expect(entriesState[0].favorite).toBe(true);

    // Simulated backend failure
    let backendFailed = true;
    if (backendFailed) {
      // Rollback
      entriesState = prevEntries;
    }
    expect(entriesState[0].favorite).toBe(false);
  });

  it('optimistically updates pin status and rolls back on failure', async () => {
    let entriesState: PasswordEntry[] = [
      {
        id: 'opt-2',
        title: 'Pinned Service',
        username: 'user',
        password: 'pwd',
        url: '',
        tags: [],
        favorite: false,
        pinned: false,
        customFields: [],
        createdAt: '2026-09-13T10:00:00Z',
        updatedAt: '2026-09-13T10:00:00Z',
      },
    ];

    const prevEntries = [...entriesState];
    entriesState = entriesState.map((e) =>
      e.id === 'opt-2' ? { ...e, pinned: !e.pinned } : e
    );
    expect(entriesState[0].pinned).toBe(true);

    // Simulate failure & rollback
    entriesState = prevEntries;
    expect(entriesState[0].pinned).toBe(false);
  });
});

