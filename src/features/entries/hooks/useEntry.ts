import { useState, useEffect } from 'react';
import { useBackend } from '@/lib/useBackend';
import type { DecryptedEntry } from '@/lib/backend';

export function useEntry(id: string | null) {
  const { backend } = useBackend();
  const [entry, setEntry] = useState<DecryptedEntry | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!backend || !id) {
      setEntry(null);
      return;
    }
    setLoading(true);
    backend.getEntry(id)
      .then(setEntry)
      .catch(() => setEntry(null))
      .finally(() => setLoading(false));
  }, [backend, id]);

  return { entry, loading };
}
