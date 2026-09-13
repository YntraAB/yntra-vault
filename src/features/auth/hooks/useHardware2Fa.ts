import { useState, useCallback } from 'react';
import { useBackend } from '@/lib/useBackend';
import type { Hardware2FaInfo, Hardware2FaProtocol } from '@/lib/backend';

export function useHardware2Fa() {
  const { backend } = useBackend();
  const [info, setInfo] = useState<Hardware2FaInfo | null>(null);
  const [loading, setLoading] = useState(false);

  const checkAvailability = useCallback(async () => {
    if (!backend) return null;
    try {
      const res = await backend.checkHardware2FaAvailable();
      setInfo(res);
      return res;
    } catch {
      return null;
    }
  }, [backend]);

  const isEnabled = useCallback(async (path: string) => {
    if (!backend) return false;
    try {
      return await backend.isHardware2FaEnabled(path);
    } catch {
      return false;
    }
  }, [backend]);

  const getChallenge = useCallback(async (path: string) => {
    if (!backend) return null;
    try {
      return await backend.getHardware2FaChallenge(path);
    } catch {
      return null;
    }
  }, [backend]);

  const performChallenge = useCallback(async (protocol: Hardware2FaProtocol, challenge?: number[], credentialId?: number[]) => {
    if (!backend) return [];
    return backend.performHardware2FaChallenge(protocol, challenge, credentialId);
  }, [backend]);

  const enable = useCallback(async (
    password: string,
    keyFilePath: string | undefined,
    protocol: Hardware2FaProtocol,
    keyName: string,
    challengeSalt: number[] | undefined,
    credentialId: number[] | undefined,
    hardwareResponse: number[],
  ) => {
    if (!backend) return;
    setLoading(true);
    try {
      await backend.enableHardware2Fa(password, keyFilePath, protocol, keyName, challengeSalt, credentialId, hardwareResponse);
    } finally {
      setLoading(false);
    }
  }, [backend]);

  const disable = useCallback(async () => {
    if (!backend) return;
    setLoading(true);
    try {
      await backend.disableHardware2Fa();
    } finally {
      setLoading(false);
    }
  }, [backend]);

  return { info, loading, checkAvailability, isEnabled, getChallenge, performChallenge, enable, disable };
}
