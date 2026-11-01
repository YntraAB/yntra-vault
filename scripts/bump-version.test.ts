import { afterEach, expect, test } from 'bun:test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { bumpVersion, nextVersion } from './bump-version.mjs';

const roots: string[] = [];
afterEach(() => { for (const root of roots.splice(0)) fs.rmSync(root, { recursive: true }); });
function fixture() {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'yntra-version-'));
    roots.push(root);
    const write = (file: string, content: string) => {
        fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
        fs.writeFileSync(path.join(root, file), content);
    };
    for (const file of ['package.json', 'src-tauri/tauri.conf.json']) write(file, '{\n  "version": "0.2.7"\n}\n');
    const names = ['app', 'core', 'crypto', 'cli'];
    ['src-tauri', 'crates/core', 'crates/crypto', 'crates/cli'].forEach((dir, i) => {
        write(`${dir}/Cargo.toml`, `[package]\nname = "${names[i]}"\nversion = "0.2.7"\n`);
    });
    const lock = names.map(name => `[[package]]\nname = "${name}"\nversion = "0.2.7"\n`).join('\n');
    write('Cargo.lock', lock);
    write('CHANGELOG.md', '# Changelog\r\n\r\n## [Unreleased]\r\n\r\n### Fixed\r\n- Pending fix\r\n\r\n## [0.2.7] - 2026-10-09\r\n- History\r\n');
    const snapshot = () => fs.readdirSync(root, { recursive: true }).map(String)
        .filter(file => fs.statSync(path.join(root, file)).isFile())
        .sort().map(file => [file, fs.readFileSync(path.join(root, file)).toString('hex')]);
    return { root, write, snapshot, refresh: () => write('Cargo.lock', lock.replaceAll('0.2.7', '0.2.8')) };
}

test('version policy preserves rollover and refuses replacement or downgrade', () => {
    expect(nextVersion('0.2.9')).toBe('0.3.0');
    expect(nextVersion('0.9.9')).toBe('1.0.0');
    expect(nextVersion('0.2.9', 'patch')).toBe('0.2.10');
    for (const value of ['0.2.7', '0.2.6', '00.2.8', 'invalid']) expect(() => nextVersion('0.2.7', value)).toThrow();
});

test('missing or inconsistent manifests fail before writing', () => {
    const f = fixture();
    fs.unlinkSync(path.join(f.root, 'crates/cli/Cargo.toml'));
    const before = f.snapshot();
    expect(() => bumpVersion(f.root, 'auto', () => { throw Error('must not run'); })).toThrow();
    expect(f.snapshot()).toEqual(before);
});

test('cargo failure restores manifests, changelog and changed lock byte for byte', () => {
    const f = fixture();
    const before = f.snapshot();
    expect(() => bumpVersion(f.root, 'auto', () => { f.refresh(); throw Error('compiler failed'); })).toThrow('compiler failed');
    expect(f.snapshot()).toEqual(before);
});

test('incorrect lock after successful cargo is rejected and rolled back', () => {
    const f = fixture();
    const before = f.snapshot();
    expect(() => bumpVersion(f.root, 'auto', () => {})).toThrow('Cargo.lock version mismatch');
    expect(f.snapshot()).toEqual(before);
});

test('successful bump promotes unreleased notes and preserves historical notes', () => {
    const f = fixture();
    expect(bumpVersion(f.root, 'auto', f.refresh)).toBe('0.2.8');
    const changelog = fs.readFileSync(path.join(f.root, 'CHANGELOG.md'), 'utf8');
    expect(changelog).toContain('## [Unreleased]\r\n\r\n## [0.2.8]');
    expect(changelog).toContain('### Fixed\r\n- Pending fix');
    expect(changelog).toContain('## [0.2.7] - 2026-10-09\r\n- History\r\n');
    expect(JSON.parse(fs.readFileSync(path.join(f.root, 'package.json'), 'utf8')).version).toBe('0.2.8');
});
