#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');

// 1. Read current version from package.json
const currentPkg = JSON.parse(fs.readFileSync(path.join(rootDir, 'package.json'), 'utf8'));
const currentVersion = currentPkg.version;

let arg = process.argv[2]?.trim();

function getNextVersion(current) {
    const match = current.match(/^(\d+)\.(\d+)\.(\d+)$/);
    if (!match) throw new Error(`Invalid current version: ${current}`);
    let major = parseInt(match[1], 10);
    let minor = parseInt(match[2], 10);
    let patch = parseInt(match[3], 10);

    // Increment patch, rolling over at 9 (e.g., 0.2.9 -> 0.3.0, 0.9.9 -> 1.0.0)
    if (patch >= 9) {
        patch = 0;
        minor += 1;
        if (minor >= 10) {
            minor = 0;
            major += 1;
        }
    } else {
        patch += 1;
    }
    return `${major}.${minor}.${patch}`;
}

let targetVersion = '';
let autoCalculated = false;

if (!arg || arg === 'auto' || arg === 'next') {
    targetVersion = getNextVersion(currentVersion);
    autoCalculated = true;
} else if (arg === 'patch') {
    const match = currentVersion.match(/^(\d+)\.(\d+)\.(\d+)$/);
    let [maj, min, pat] = [parseInt(match[1], 10), parseInt(match[2], 10), parseInt(match[3], 10)];
    targetVersion = `${maj}.${min}.${pat + 1}`;
} else if (arg === 'minor') {
    const match = currentVersion.match(/^(\d+)\.(\d+)\.(\d+)$/);
    let [maj, min] = [parseInt(match[1], 10), parseInt(match[2], 10)];
    targetVersion = `${maj}.${min + 1}.0`;
} else if (arg === 'major') {
    const match = currentVersion.match(/^(\d+)\.(\d+)\.(\d+)$/);
    let maj = parseInt(match[1], 10);
    targetVersion = `${maj + 1}.0.0`;
} else if (/^\d+\.\d+\.\d+$/.test(arg)) {
    targetVersion = arg;
} else {
    console.error(`Error: Version must be in semver format (X.Y.Z) or 'auto' (received: '${arg}')`);
    console.error('Usage: bun run bump [version]');
    process.exit(1);
}

console.log(`\x1b[36m================================================\x1b[0m`);
console.log(`\x1b[36m Yntra Vault: Automated Version Synchronization \x1b[0m`);
console.log(`\x1b[36m Current Version: ${currentVersion}\x1b[0m`);
console.log(`\x1b[36m Target Version:  ${targetVersion}${autoCalculated ? ' (auto-calculated)' : ''}\x1b[0m`);
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

// 2. Update manifest files
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

// 3. Refresh Cargo.lock
console.log(`\n\x1b[33mRefreshing Cargo.lock via cargo check --workspace...\x1b[0m`);
try {
    execSync('cargo check --workspace', { cwd: rootDir, stdio: 'inherit' });
    console.log(`\x1b[32m✔ Cargo.lock synchronized across workspace\x1b[0m`);
} catch (err) {
    console.error(`\x1b[31mError: cargo check --workspace failed\x1b[0m`);
    process.exit(1);
}

// 4. Ensure CHANGELOG.md has a section for this version
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

// 5. Verification Check
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
