import { describe, expect, it } from 'bun:test';
import { hardenManifest, backupRules, extractionRules } from './android-security.mjs';
describe('Android backup policy', () => {
  it('replaces generated permissive defaults and is idempotent', () => {
    const input = '<manifest><application android:allowBackup="true" android:fullBackupContent="true" android:icon="@mipmap/ic_launcher"></application></manifest>';
    const output = hardenManifest(input);
    expect(output).toContain('android:allowBackup="false"');
    expect(output).toContain('android:dataExtractionRules="@xml/data_extraction_rules"');
    expect(output).toContain('android:icon="@mipmap/ic_launcher"');
    expect(hardenManifest(output)).toBe(output);
    expect(backupRules).toContain('domain="sharedpref"');
    expect(extractionRules).toContain('<device-transfer>');
    expect(() => hardenManifest('<manifest/>')).toThrow();
  });
});
