import { afterEach, expect, test } from 'bun:test';
import { copyFileSync, mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const temporary: string[] = [];
afterEach(() => { for (const path of temporary.splice(0)) rmSync(path, { recursive: true }); });
const windowsTest = process.platform === 'win32' ? test : test.skip;
function fixture(script: string) {
    const root = mkdtempSync(join(tmpdir(), 'yntra-tooling-'));
    temporary.push(root);
    const scripts = join(root, 'scripts');
    mkdirSync(scripts);
    copyFileSync(resolve('scripts', script), join(scripts, script));
    return { root, scripts };
}

windowsTest('PowerShell version wrapper propagates a failed child process', () => {
    const { scripts } = fixture('bump-version.ps1');
    writeFileSync(join(scripts, 'bump-version.mjs'), 'process.exit(13);');
    const result = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-File', join(scripts, 'bump-version.ps1')], { encoding: 'utf8' });
    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain('Version update failed');
});

windowsTest('build stops on frontend install failure before using stale artifacts', () => {
    const { root, scripts } = fixture('build-reproducible.ps1');
    mkdirSync(join(root, 'target', 'release'), { recursive: true });
    writeFileSync(join(root, 'target', 'release', 'yntra-vault-app.exe'), 'stale fixture');
    const runner = join(scripts, 'runner.ps1');
    writeFileSync(runner, `function global:git { '1700000000'; $global:LASTEXITCODE = 0 }
function global:bun { $global:LASTEXITCODE = 7 }
function global:cargo { throw 'Cargo must not run after Bun fails' }
& (Join-Path $PSScriptRoot 'build-reproducible.ps1')
`);
    const result = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-File', runner], { encoding: 'utf8' });
    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain('bun failed');
    expect(result.stderr).not.toContain('Cargo must not run');
});
