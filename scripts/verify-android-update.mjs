import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

// Public certificate fingerprint of the permanent release identity. This is not a secret.
export const ANDROID_SIGNER = '6360e0c97cc43206e58b7fd4351ab38afe1c7e096d15056bdcac6b87e91f7ff0';
export function verifyAndroidUpdate(badging, signing, version) {
  const parsed = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
  if (!parsed) throw new Error('Android releases require a stable semantic version');
  const [, major, minor, patch] = parsed.map(Number);
  const code = major * 1000000 + minor * 1000 + patch;
  if (minor > 999 || patch > 999 || code < 1 || code > 2100000000) throw new Error('Android versionCode is outside the stable version mapping');
  const packageLine = badging.split(/\r?\n/).find(line => line.startsWith('package: ')) || '';
  if (!packageLine.includes("name='com.yntravault.app'")
    || !packageLine.includes(`versionName='${version}'`) || !packageLine.includes(`versionCode='${code}'`)) {
    throw new Error('Android package identity/version changed; refusing an incompatible update');
  }
  const signers = [...signing.matchAll(/^Signer #\d+ certificate SHA-256 digest: ([a-f0-9]+)\s*$/gmi)].map(match => match[1].toLowerCase());
  if (signers.length !== 1 || signers[0] !== ANDROID_SIGNER) {
    throw new Error('Android release must use the permanent signing certificate');
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [badging, signing, version] = process.argv.slice(2);
  verifyAndroidUpdate(readFileSync(badging, 'utf8'), readFileSync(signing, 'utf8'), version);
  console.log('Android update identity and version verified.');
}
