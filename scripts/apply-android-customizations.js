#!/usr/bin/env node

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

  const requiredPermissions = [
    '    <!-- Camera for optical QR pairing and TOTP 2FA secret scanning -->',
    '    <uses-permission android:name="android.permission.CAMERA" />',
    '    <uses-permission android:name="android.permission.READ_EXTERNAL_STORAGE" android:maxSdkVersion="32" />',
    '    <uses-permission android:name="android.permission.READ_MEDIA_IMAGES" />',
    '    <uses-feature android:name="android.hardware.camera" android:required="false" />',
    '    <uses-feature android:name="android.hardware.camera.autofocus" android:required="false" />',
  ];

  if (!manifestContent.includes('android.permission.CAMERA')) {
    const manifestTagMatch = manifestContent.match(/<manifest\b[^>]*>/);
    if (manifestTagMatch) {
      const insertIdx = manifestTagMatch.index + manifestTagMatch[0].length;
      manifestContent =
        manifestContent.slice(0, insertIdx) +
        '\n' +
        requiredPermissions.join('\n') +
        manifestContent.slice(insertIdx);

      fs.writeFileSync(manifestPath, manifestContent, 'utf8');
      console.log('✓ Injected camera and storage permissions into AndroidManifest.xml');
    } else {
      console.warn('⚠️ Could not find <manifest> tag in AndroidManifest.xml');
    }
  } else {
    console.log('✓ Camera permission already present in AndroidManifest.xml');
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
