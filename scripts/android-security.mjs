// Security policy applied to generated projects; launcher resources are unrelated.
export function hardenManifest(xml) {
  if (!/<application\b/.test(xml)) throw new Error('Android application element missing');
  xml = xml.replace(/<uses-permission\b[^>]*android:name="android\.permission\.(READ_EXTERNAL_STORAGE|READ_MEDIA_IMAGES)"[^>]*\/>/g, '');
  return xml.replace(/<application\b[^>]*>/, tag => tag
    .replace(/\s+android:(allowBackup|fullBackupContent|dataExtractionRules)="[^"]*"/g, '')
    .replace('>', ' android:allowBackup="false" android:fullBackupContent="@xml/backup_rules" android:dataExtractionRules="@xml/data_extraction_rules">'));
}
const excludes = ['root', 'file', 'database', 'sharedpref', 'external', 'device_root', 'device_file', 'device_database', 'device_sharedpref']
  .map(domain => `<exclude domain="${domain}" path="." />`).join('');
export const backupRules = `<full-backup-content>${excludes}</full-backup-content>`;
export const extractionRules = `<data-extraction-rules><cloud-backup>${excludes}</cloud-backup><device-transfer>${excludes}</device-transfer></data-extraction-rules>`;
