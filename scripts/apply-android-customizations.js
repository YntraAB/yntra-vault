#!/usr/bin/env node
import { hardenManifest, backupRules, extractionRules } from './android-security.mjs';

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const rootDir = path.resolve(__dirname, '..');

// Candidates for android project directories
const candidatePaths = [
  path.join(rootDir, 'src-tauri', 'gen', 'android', 'app', 'src', 'main'),
  path.join(rootDir, 'gen', 'android', 'app', 'src', 'main'),
];

const mainDir = candidatePaths.find(p => fs.existsSync(p));

if (!mainDir) {
  console.log('[apply-android-customizations] No Android gen directory found. Skipping (run after `tauri android init`).');
  process.exit(0);
}

console.log(`[apply-android-customizations] Target Android directory found: ${mainDir}`);

// 1. Patch AndroidManifest.xml
const manifestPath = path.join(mainDir, 'AndroidManifest.xml');
if (fs.existsSync(manifestPath)) {
  let manifestContent = fs.readFileSync(manifestPath, 'utf8');
  manifestContent = hardenManifest(manifestContent);
  fs.writeFileSync(manifestPath, manifestContent, 'utf8');


  if (!manifestContent.includes('${applicationId}.updates')) {
    if (!manifestContent.includes('</application>')) throw new Error('Android application element missing');
    manifestContent = manifestContent.replace('</application>', `
        <provider android:name="com.yntravault.app.UpdateFileProvider"
            android:authorities="\${applicationId}.updates" android:exported="false" android:grantUriPermissions="true">
            <meta-data android:name="android.support.FILE_PROVIDER_PATHS" android:resource="@xml/update_paths" />
        </provider>
    </application>`);
    fs.writeFileSync(manifestPath, manifestContent, 'utf8');
  }
  const xmlDir = path.join(mainDir, 'res', 'xml');
  fs.mkdirSync(xmlDir, { recursive: true });
  fs.writeFileSync(path.join(xmlDir, 'backup_rules.xml'), backupRules);
  fs.writeFileSync(path.join(xmlDir, 'data_extraction_rules.xml'), extractionRules);
  fs.writeFileSync(path.join(xmlDir, 'update_paths.xml'), '<paths xmlns:android="http://schemas.android.com/apk/res/android"><cache-path name="verified_updates" path="updates/" /></paths>');

  const requiredEntries = [
    { name: 'android.permission.INTERNET', tag: '    <uses-permission android:name="android.permission.INTERNET" />' },
    { name: 'android.permission.CAMERA', tag: '    <uses-permission android:name="android.permission.CAMERA" />' },
    { name: 'android.hardware.camera', tag: '    <uses-feature android:name="android.hardware.camera" android:required="false" />' },
    { name: 'android.hardware.camera.autofocus', tag: '    <uses-feature android:name="android.hardware.camera.autofocus" android:required="false" />' },
    { name: 'android.permission.REQUEST_INSTALL_PACKAGES', tag: '    <uses-permission android:name="android.permission.REQUEST_INSTALL_PACKAGES" />' },
  ];

  const missingEntries = requiredEntries.filter(entry => !manifestContent.includes(entry.name));

  if (missingEntries.length > 0) {
    const manifestTagMatch = manifestContent.match(/<manifest\b[^>]*>/);
    if (manifestTagMatch) {
      const insertIdx = manifestTagMatch.index + manifestTagMatch[0].length;
      const injectedTags = missingEntries.map(e => e.tag).join('\n');
      manifestContent =
        manifestContent.slice(0, insertIdx) +
        '\n' +
        injectedTags +
        manifestContent.slice(insertIdx);

      fs.writeFileSync(manifestPath, manifestContent, 'utf8');
      console.log(`✓ Injected missing permissions/features into AndroidManifest.xml: ${missingEntries.map(e => e.name).join(', ')}`);
    } else {
      console.warn('⚠️ Could not find <manifest> tag in AndroidManifest.xml');
    }
  } else {
    console.log('✓ All required permissions and features already present in AndroidManifest.xml');
  }
} else {
  console.warn(`⚠️ AndroidManifest.xml not found at ${manifestPath}`);
}

// 2. Replace or update MainActivity.kt
const overrideMainActivityPath = path.join(rootDir, 'src-tauri', 'android-overrides', 'MainActivity.kt');
if (fs.existsSync(overrideMainActivityPath)) {
  const targetKotlinDir = path.join(mainDir, 'java', 'com', 'yntravault', 'app');
  fs.mkdirSync(targetKotlinDir, { recursive: true });

  const targetMainActivityPath = path.join(targetKotlinDir, 'MainActivity.kt');
  const overrideContent = fs.readFileSync(overrideMainActivityPath, 'utf8');

  fs.writeFileSync(targetMainActivityPath, overrideContent, 'utf8');
  fs.copyFileSync(path.join(rootDir, 'src-tauri', 'android-overrides', 'UpdateInstallerPlugin.kt'), path.join(targetKotlinDir, 'UpdateInstallerPlugin.kt'));
  fs.copyFileSync(path.join(rootDir, 'src-tauri', 'android-overrides', 'MobileServicesPlugin.kt'), path.join(targetKotlinDir, 'MobileServicesPlugin.kt'));
  console.log(`✓ Applied customized MainActivity.kt to ${targetMainActivityPath}`);
} else {
  console.warn(`⚠️ Override MainActivity.kt not found at ${overrideMainActivityPath}`);
}

// 3. Ensure custom app icons are copied if present
const iconsSrcDir = path.join(rootDir, 'src-tauri', 'icons', 'android');
const resTargetDir = path.join(mainDir, 'res');

if (fs.existsSync(iconsSrcDir) && fs.existsSync(resTargetDir)) {
  fs.cpSync(iconsSrcDir, resTargetDir, { recursive: true, force: true });
  console.log('✓ Synced Yntra Vault Android icons to res directory');
}

console.log('[apply-android-customizations] Android customizations successfully applied!');
