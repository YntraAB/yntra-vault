import { createPrivateKey, createPublicKey, sign } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

export function signManifest(bytes, privateDer, publicHex, now = Math.floor(Date.now() / 1000)) {
  const key = createPrivateKey({ key: privateDer, type: 'pkcs8', format: 'der' });
  if (key.asymmetricKeyType !== 'ed25519') throw new Error('An Ed25519 update signing key is required');
  const actual = createPublicKey(key).export({ type: 'spki', format: 'der' }).subarray(-32).toString('hex');
  if (!/^[a-f0-9]{64}$/.test(publicHex) || actual !== publicHex) throw new Error('Update signing key does not match pinned public identity');
  // Keep the legacy manifest readable by older clients; authenticate its exact bytes separately.
  const envelope = { schema: 1, key_id: 'yntra-update-1', issued_at: now, expires_at: now + 90 * 86400 };
  const context = Buffer.from(`yntra-update-v1\n${envelope.key_id}\n${envelope.issued_at}\n${envelope.expires_at}\n`);
  return { ...envelope, signature: sign(null, Buffer.concat([context, bytes]), key).toString('base64') };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const path = process.argv[2] ?? 'release-assets/latest.json';
  const encoded = process.env.YNTRA_UPDATE_SIGNING_KEY;
  delete process.env.YNTRA_UPDATE_SIGNING_KEY;
  if (!encoded) throw new Error('YNTRA_UPDATE_SIGNING_KEY is required; unsigned releases are forbidden');
  const secret = Buffer.from(encoded, 'base64');
  try {
    const publicHex = readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), '../update-signing-public-key.hex'), 'utf8').trim();
    const signed = signManifest(readFileSync(path), secret, publicHex);
    writeFileSync(`${path}.sig`, JSON.stringify(signed, null, 2) + '\n');
  } finally { secret.fill(0); }
}
