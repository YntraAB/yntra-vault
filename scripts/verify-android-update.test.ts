import { describe, expect, it } from 'bun:test';
import { readFileSync } from 'node:fs';
import { ANDROID_SIGNER, verifyAndroidUpdate } from './verify-android-update.mjs';

describe('release upgrade compatibility', () => {
  const badging = "package: name='com.yntravault.app' versionCode='2004' versionName='0.2.4'";
  const signer = `Signer #1 certificate SHA-256 digest: ${ANDROID_SIGNER}\n`;
  it('accepts the permanent identity and rejects a different app, key, or build version', () => {
    expect(() => verifyAndroidUpdate(badging, signer, '0.2.4')).not.toThrow();
    for (const invalid of [badging.replace('com.yntravault.app', 'com.yntravault.newapp'), badging.replace('2004', '1'), badging.replace('0.2.4', '0.2.3')]) {
      expect(() => verifyAndroidUpdate(invalid, signer, '0.2.4')).toThrow();
    }
    expect(() => verifyAndroidUpdate(badging, signer.replace(ANDROID_SIGNER, '0'.repeat(64)), '0.2.4')).toThrow();
    expect(() => verifyAndroidUpdate(badging, '', '0.2.4')).toThrow();
    expect(() => verifyAndroidUpdate(badging, signer, '0.2.4-beta')).toThrow();
  });
  it('keeps the application identity and WebView origin of existing installations', () => {
    const config = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
    expect(config.identifier).toBe('com.yntravault.app');
    expect(config.app.windows[0].label).toBe('main');
    expect(config.app.windows[0].useHttpsScheme).toBe(false);
    expect(config.app.windows[0].dataDirectory).toBeUndefined();
  });
  it('accepts Build Tools 37 scheme labels while rejecting extra or unrecognized signers', () => {
    const modern = `Verifies\nVerified using v3 scheme (APK Signature Scheme v3): true\nNumber of signers: 1\nV3.0 Signer: certificate SHA-256 digest: ${ANDROID_SIGNER}\n`;
    expect(() => verifyAndroidUpdate(badging, modern, '0.2.4')).not.toThrow();
    expect(() => verifyAndroidUpdate(badging, modern.replaceAll('\n', '\r\n'), '0.2.4')).not.toThrow();
    expect(() => verifyAndroidUpdate(badging, modern + `V3.1 Signer: certificate SHA-256 digest: ${ANDROID_SIGNER}\n`, '0.2.4')).not.toThrow();
    for (const invalid of [
      modern.replace('signers: 1', 'signers: 2'),
      modern.replace(ANDROID_SIGNER, '0'.repeat(64)),
      modern + `V3.1 Signer: certificate SHA-256 digest: ${'0'.repeat(64)}\n`,
      modern + `Unknown signer certificate SHA-256 digest: ${ANDROID_SIGNER}\n`,
      signer + signer,
    ]) expect(() => verifyAndroidUpdate(badging, invalid, '0.2.4')).toThrow();
  });
  it('can execute the Android customization script before project generation', () => {
    const process = Bun.spawnSync(['bun', '--check', 'scripts/apply-android-customizations.js']);
    expect(process.exitCode).toBe(0);
  });
});
