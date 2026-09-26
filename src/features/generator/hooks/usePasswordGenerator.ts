import { useState, useCallback, useRef, useEffect } from 'react';
import { type GeneratorOptions, type StrengthScore, type BreachResult } from '@/lib/backend';
import { useBackend } from '@/lib/useBackend';

export function usePasswordGenerator() {
  const { backend } = useBackend();
  const [password, setPassword] = useState<string>('');
  const [strength, setStrength] = useState<StrengthScore | null>(null);
  const [breach, setBreach] = useState<BreachResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const generation = useRef(0);
  useEffect(() => () => { generation.current++; }, []);

  const generate = useCallback(async (options?: GeneratorOptions) => {
    if (!backend) return '';
    const request = ++generation.current;
    setPassword('');
    setStrength(null);
    setBreach(null);
    setError(null);
    try {
      const pw = options
        ? await backend.generatePassword(options)
        : await backend.generatePasswordDefault();
      const score = await backend.analyzePasswordStrength(pw);
      if (request !== generation.current) return '';
      setPassword(pw);
      setStrength(score);
      return pw;
    } catch (err) {
      if (request === generation.current) setError(String(err));
      return '';
    }
  }, [backend]);

  const checkBreach = useCallback(async (pw?: string) => {
    if (!backend) return;
    const target = pw || password;
    if (!target) return;
    try {
      const result = await backend.checkPasswordBreach(target);
      setBreach(result);
      return result;
    } catch {
      // Offline — skip breach check
    }
  }, [backend, password]);

  const analyzeStrength = useCallback(async (pw: string) => {
    if (!backend) return null;
    const score = await backend.analyzePasswordStrength(pw);
    setStrength(score);
    return score;
  }, [backend]);

  return { password, strength, breach, error, generate, checkBreach, analyzeStrength };
}

export default usePasswordGenerator;
