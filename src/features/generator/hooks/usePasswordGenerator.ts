import { useState, useCallback } from 'react';
import { type GeneratorOptions, type StrengthScore, type BreachResult } from '@/lib/backend';
import { useBackend } from '@/lib/useBackend';

export function usePasswordGenerator() {
  const { backend } = useBackend();
  const [password, setPassword] = useState<string>('');
  const [strength, setStrength] = useState<StrengthScore | null>(null);
  const [breach, setBreach] = useState<BreachResult | null>(null);

  const generate = useCallback(async (options?: GeneratorOptions) => {
    if (!backend) return '';
    let pw = '';
    let isBreached = true;
    let attempts = 0;

    // Generate and check against HIBP database. Loop up to 5 times.
    while (isBreached && attempts < 5) {
      attempts++;
      pw = options
        ? await backend.generatePassword(options)
        : await backend.generatePasswordDefault();
      
      try {
        const result = await backend.checkPasswordBreach(pw);
        isBreached = result.is_breached;
      } catch {
        // Safe fallback in case of no network / API errors
        isBreached = false;
      }
    }

    setPassword(pw);

    // Auto-analyze strength
    const score = await backend.analyzePasswordStrength(pw);
    setStrength(score);

    return pw;
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

  return { password, strength, breach, generate, checkBreach, analyzeStrength };
}

export default usePasswordGenerator;
