#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const manifests = ['package.json', 'src-tauri/tauri.conf.json', 'src-tauri/Cargo.toml',
    'crates/core/Cargo.toml', 'crates/crypto/Cargo.toml', 'crates/cli/Cargo.toml'];
const jsonVersion = /(^\s*"version"\s*:\s*)"([^"]+)"/m;
const cargoVersion = /(\[package\][\s\S]*?^version\s*=\s*)"([^"]+)"/m;

function parts(version) {
    if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) throw new Error(`Invalid version: ${version}`);
    const result = version.split('.').map(Number);
    if (!result.every(Number.isSafeInteger)) throw new Error('Version component is too large');
    return result;
}

export function nextVersion(current, argument = 'auto') {
    let [major, minor, patch] = parts(current);
    const arg = argument.trim() || 'auto';
    if (arg === 'auto' || arg === 'next') {
        // Preserve the project's automatic decimal rollover policy.
        if (patch >= 9) { patch = 0; minor++; if (minor >= 10) { minor = 0; major++; } }
        else { patch++; }
    } else if (arg === 'patch') { patch++; }
    else if (arg === 'minor') { minor++; patch = 0; }
    else if (arg === 'major') { major++; minor = 0; patch = 0; }
    else { [major, minor, patch] = parts(arg); }
    const target = `${major}.${minor}.${patch}`;
    const before = parts(current);
    const after = parts(target);
    const index = after.findIndex((value, i) => value !== before[i]);
    if (index < 0 || after[index] < before[index]) throw new Error('New version must be greater than the current version');
    return target;
}

function checkLock(content, packages, version) {
    const entries = content.split(/^\[\[package\]\]\s*$/m).slice(1);
    for (const name of packages) {
        const matches = entries.filter(entry => entry.match(/^name = "([^"]+)"/m)?.[1] === name);
        if (matches.length !== 1 || matches[0].match(/^version = "([^"]+)"/m)?.[1] !== version) {
            throw new Error(`Cargo.lock version mismatch for ${name}`);
        }
    }
}

export function bumpVersion(root, argument, runCargo = () => {
    execFileSync('cargo', ['check', '--workspace', '--offline'], { cwd: root, stdio: 'inherit' });
}) {
    const files = [...manifests, 'Cargo.lock', 'CHANGELOG.md'];
    // Preflight every input before changing any file. Keep exact bytes for rollback.
    const originals = new Map(files.map(file => [file, fs.readFileSync(path.join(root, file))]));
    const current = JSON.parse(originals.get('package.json').toString()).version;
    const target = nextVersion(current, argument);
    const packages = [];
    const updates = new Map();
    for (const file of manifests) {
        const content = originals.get(file).toString();
        const json = file.endsWith('.json');
        const pattern = json ? jsonVersion : cargoVersion;
        const match = content.match(pattern);
        if (!match || match[2] !== current || (json && JSON.parse(content).version !== current)) {
            throw new Error(`Version mismatch in ${file}`);
        }
        if (!json) {
            const name = content.match(/\[package\][\s\S]*?^name\s*=\s*"([^"]+)"/m)?.[1];
            if (!name) throw new Error(`Missing package name in ${file}`);
            packages.push(name);
        }
        updates.set(file, content.replace(pattern, `$1"${target}"`));
    }
    checkLock(originals.get('Cargo.lock').toString(), packages, current);
    const changelog = originals.get('CHANGELOG.md').toString();
    if (changelog.includes(`## [${target}]`)) throw new Error('Target version already exists in CHANGELOG.md');
    const newline = changelog.includes('\r\n') ? '\r\n' : '\n';
    const header = `## [${target}] - ${new Date().toISOString().slice(0, 10)}`;
    const updatedChangelog = /^## \[Unreleased\]\s*$/m.test(changelog)
        ? changelog.replace(/^## \[Unreleased\][^\r\n]*$/m, `## [Unreleased]${newline}${newline}${header}`)
        : changelog.replace(/^## \[/m, `${header}${newline}${newline}### Changed${newline}${newline}## [`);
    if (updatedChangelog === changelog) throw new Error('Missing changelog version section');
    updates.set('CHANGELOG.md', updatedChangelog);
    try {
        for (const [file, content] of updates) fs.writeFileSync(path.join(root, file), content);
        runCargo();
        checkLock(fs.readFileSync(path.join(root, 'Cargo.lock'), 'utf8'), packages, target);
        for (const [file, content] of updates) {
            if (fs.readFileSync(path.join(root, file), 'utf8') !== content) throw new Error(`Unexpected change in ${file}`);
        }
    } catch (error) {
        const failures = [];
        for (const [file, content] of originals) {
            try { fs.writeFileSync(path.join(root, file), content); }
            catch { failures.push(file); }
        }
        if (failures.length) throw new Error(`Version update failed; restore these files from backup: ${failures.join(', ')}`, { cause: error });
        throw error;
    }
    return target;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
    try {
        const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
        console.log(`Version synchronized: ${bumpVersion(root, process.argv[2])}`);
    } catch (error) {
        console.error(`Version update failed: ${error.message}`);
        process.exitCode = 1;
    }
}
