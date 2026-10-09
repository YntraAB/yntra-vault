#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');

const targetVersion = process.argv[2]?.trim();

if (!targetVersion || !/^\d+\.\d+\.\d+$/.test(targetVersion)) {
    console.error('Usage: bun run bump <version>  (e.g., bun run bump 0.2.8)');
    console.error('Error: Version must be in semver format (X.Y.Z)');
    process.exit(1);
}

console.log(`\x1b[36m================================================\x1b[0m`);
console.log(`\x1b[36m Yntra Vault: Automated Version Synchronization \x1b[0m`);
console.log(`\x1b[36m Target Version: ${targetVersion}\x1b[0m`);
console.log(`\x1b[36m================================================\x1b[0m\n`);

const filesToUpdate = [
    {
        name: 'package.json',
        path: path.join(rootDir, 'package.json'),
        regex: /(^\s*"version"\s*:\s*)"[^"]+"/m,
        replace: `$1"${targetVersion}"`
    },
    {
        name: 'src-tauri/tauri.conf.json',
        path: path.join(rootDir, 'src-tauri', 'tauri.conf.json'),
        regex: /(^\s*"version"\s*:\s*)"[^"]+"/m,
        replace: `$1"${targetVersion}"`
    },
    {
        name: 'src-tauri/Cargo.toml',
        path: path.join(rootDir, 'src-tauri', 'Cargo.toml'),
        regex: /(\[package\][\s\S]*?^version\s*=\s*)"[^"]+"/m,
        replace: `$1"${targetVersion}"`
    },
    {
        name: 'crates/core/Cargo.toml',
        path: path.join(rootDir, 'crates', 'core', 'Cargo.toml'),
        regex: /(\[package\][\s\S]*?^version\s*=\s*)"[^"]+"/m,
        replace: `$1"${targetVersion}"`
    },
    {
        name: 'crates/crypto/Cargo.toml',
        path: path.join(rootDir, 'crates', 'crypto', 'Cargo.toml'),
        regex: /(\[package\][\s\S]*?^version\s*=\s*)"[^"]+"/m,
        replace: `$1"${targetVersion}"`
    },
    {
        name: 'crates/cli/Cargo.toml',
        path: path.join(rootDir, 'crates', 'cli', 'Cargo.toml'),
        regex: /(\[package\][\s\S]*?^version\s*=\s*)"[^"]+"/m,
        replace: `$1"${targetVersion}"`
    }
];

// 1. Update manifest files
for (const file of filesToUpdate) {
    if (!fs.existsSync(file.path)) {
        console.error(`\x1b[31mError: Missing file ${file.path}\x1b[0m`);
        process.exit(1);
    }
    const content = fs.readFileSync(file.path, 'utf8');
    if (!file.regex.test(content)) {
        console.error(`\x1b[31mError: Could not match version pattern in ${file.name}\x1b[0m`);
        process.exit(1);
    }
    const updated = content.replace(file.regex, file.replace);
    fs.writeFileSync(file.path, updated, 'utf8');
    console.log(`\x1b[32m✔ Updated\x1b[0m ${file.name} -> ${targetVersion}`);
}

// 2. Refresh Cargo.lock
console.log(`\n\x1b[33mRefreshing Cargo.lock via cargo check --workspace...\x1b[0m`);
try {
    execSync('cargo check --workspace', { cwd: rootDir, stdio: 'inherit' });
    console.log(`\x1b[32m✔ Cargo.lock synchronized across workspace\x1b[0m`);
} catch (err) {
    console.error(`\x1b[31mError: cargo check --workspace failed\x1b[0m`);
    process.exit(1);
}

// 3. Ensure CHANGELOG.md has a section for this version
const changelogPath = path.join(rootDir, 'CHANGELOG.md');
if (fs.existsSync(changelogPath)) {
    const changelog = fs.readFileSync(changelogPath, 'utf8');
    const versionHeaderRegex = new RegExp(`^## \\[${targetVersion.replace(/\./g, '\\.')}\\]`, 'm');
    if (!versionHeaderRegex.test(changelog)) {
        const today = new Date().toISOString().split('T')[0];
        const template = `## [${targetVersion}] - ${today}\n\n### Security\n\n### Added\n\n### Changed\n\n### Fixed\n\n`;
        // Insert after preamble (# Changelog ...)
        const preambleMatch = changelog.match(/(# Changelog[\s\S]*?Semantic Versioning\([^\)]+\)\.\s*\n\n)/m);
        if (preambleMatch) {
            const updatedChangelog = changelog.replace(preambleMatch[1], `${preambleMatch[1]}${template}`);
            fs.writeFileSync(changelogPath, updatedChangelog, 'utf8');
            console.log(`\x1b[32m✔ Added template section for [${targetVersion}] in CHANGELOG.md\x1b[0m`);
        } else {
            console.log(`\x1b[33mNotice: Could not auto-insert into CHANGELOG.md preamble; please update manually.\x1b[0m`);
        }
    } else {
        console.log(`\x1b[32m✔ CHANGELOG.md already contains section for [${targetVersion}]\x1b[0m`);
    }
}

// 4. Verification Check
console.log(`\n\x1b[36mValidating version parity across all 7 manifests...\x1b[0m`);
const pkgVer = JSON.parse(fs.readFileSync(path.join(rootDir, 'package.json'), 'utf8')).version;
const tauriVer = JSON.parse(fs.readFileSync(path.join(rootDir, 'src-tauri', 'tauri.conf.json'), 'utf8')).version;
if (pkgVer !== targetVersion || tauriVer !== targetVersion) {
    console.error(`\x1b[31mVerification mismatch in JSON manifests!\x1b[0m`);
    process.exit(1);
}

for (const manifest of ['src-tauri/Cargo.toml', 'crates/core/Cargo.toml', 'crates/crypto/Cargo.toml', 'crates/cli/Cargo.toml']) {
    const cargoText = fs.readFileSync(path.join(rootDir, manifest), 'utf8');
    const match = cargoText.match(/(?:^|\n)version\s*=\s*"([^"]+)"/);
    if (!match || match[1] !== targetVersion) {
        console.error(`\x1b[31mVerification mismatch in ${manifest} (found: ${match?.[1]})\x1b[0m`);
        process.exit(1);
    }
}

console.log(`\x1b[32m✔ All 7 manifests verified synchronously aligned to ${targetVersion}!\x1b[0m\n`);
