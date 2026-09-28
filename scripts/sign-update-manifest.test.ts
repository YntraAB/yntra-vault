import { test, expect } from 'bun:test';
import { generateKeyPairSync, createPublicKey, verify } from 'node:crypto';
import { signManifest } from './sign-update-manifest.mjs';

test('signed release metadata authenticates bytes and has a bounded lifetime', () => {
  const key = generateKeyPairSync('ed25519');
  const der = key.privateKey.export({ type: 'pkcs8', format: 'der' });
  const publicHex = key.publicKey.export({ type: 'spki', format: 'der' }).subarray(-32).toString('hex');
  const bytes = Buffer.from('{"version":"0.2.5","sha256":"abc"}\n');
  const result = signManifest(bytes, der, publicHex, 1000);
  const context = Buffer.from(`yntra-update-v1\n${result.key_id}\n${result.issued_at}\n${result.expires_at}\n`);
  expect(verify(null, Buffer.concat([context, bytes]), createPublicKey(key.privateKey), Buffer.from(result.signature, 'base64'))).toBe(true);
  expect(verify(null, Buffer.concat([context, Buffer.from('tampered')]), key.publicKey, Buffer.from(result.signature, 'base64'))).toBe(false);
  expect(result.expires_at - result.issued_at).toBe(90 * 86400);
  expect(() => signManifest(bytes, der, '00'.repeat(32))).toThrow('does not match');
});
