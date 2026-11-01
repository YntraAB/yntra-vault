import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';

export function previewFiles(version, platform) {
    if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) throw new Error('Invalid version');
    const suffixes = {
        windows: ['x64-setup.exe', 'x64_en-US.msi', 'portable.exe', 'cli.exe'],
        linux: ['amd64.AppImage', 'amd64.deb'],
        android: ['universal.apk'],
    }[platform];
    if (!suffixes) throw new Error('Invalid preview platform');
    return suffixes.map(suffix => `Yntra.Vault_${version}_${suffix}`);
}

export function validatePreview(release, source, tag, commit) {
    if (!/^[a-f0-9]{40}$/.test(commit) || source.commit !== commit || source.tag !== tag || release.tag_name !== tag) {
        throw new Error('Preview source does not match this build');
    }
    if (!release.prerelease) throw new Error('Refusing to change a stable release');
}

export function verifyExistingAsset(asset, hash) {
    if (asset.digest !== `sha256:${hash}`) throw new Error(`Existing asset differs or lacks a verified digest: ${asset.name}`);
}

export async function publishPreview(platform) {
    const repo = process.env.GITHUB_REPOSITORY;
    const commit = process.env.RELEASE_COMMIT;
    const tag = process.env.RELEASE_TAG;
    const token = process.env.GH_TOKEN;
    if (repo !== 'YntraAB/yntra-vault' || !token) throw new Error('Expected authorized public repository job');
    const version = JSON.parse(fs.readFileSync('package.json', 'utf8')).version;
    if (tag !== `v${version}`) throw new Error('Tag/version mismatch');
    const buffers = new Map(previewFiles(version, platform).map(name => [name, fs.readFileSync(path.join('release-assets', name))]));
    if ([...buffers.values()].some(bytes => !bytes.length)) throw new Error('Empty preview package');
    const hash = bytes => createHash('sha256').update(bytes).digest('hex');
    buffers.set(`SHA256SUMS-${platform}`, Buffer.from([...buffers].map(([name, bytes]) => `${hash(bytes)}  ${name}`).join('\n') + '\n'));
    const base = `https://api.github.com/repos/${repo}`;
    const headers = { Authorization: `Bearer ${token}`, Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28' };
    async function request(url, options = {}) {
        const response = await fetch(url, { ...options, headers: { ...headers, ...options.headers } });
        if (!response.ok) throw new Error(`GitHub request failed (${response.status})`);
        return response;
    }
    const release = await (await request(`${base}/releases/tags/${tag}`)).json();
    const sourceAsset = release.assets.find(asset => asset.name === 'release-source.json');
    if (!sourceAsset) throw new Error('Missing release provenance');
    const source = await (await request(`${base}/releases/assets/${sourceAsset.id}`, { headers: { Accept: 'application/octet-stream' } })).json();
    validatePreview(release, source, tag, commit);
    const target = await (await request(`${base}/commits/${tag}`)).json();
    if (target.sha !== commit) throw new Error('Release tag changed');
    // Check all existing files before uploading anything. Never replace published bytes.
    for (const [name, bytes] of buffers) {
        const asset = release.assets.find(item => item.name === name);
        if (asset) verifyExistingAsset(asset, hash(bytes));
    }
    for (const [name, bytes] of buffers) {
        if (release.assets.some(item => item.name === name)) continue;
        const uploaded = await (await request(`https://uploads.github.com/repos/${repo}/releases/${release.id}/assets?name=${encodeURIComponent(name)}`, {
            method: 'POST', headers: { 'Content-Type': 'application/octet-stream' }, body: bytes,
        })).json();
        verifyExistingAsset(uploaded, hash(bytes));
    }
    if (platform === 'windows') {
        const section = fs.readFileSync('CHANGELOG.md', 'utf8').split(`## [${version}]`)[1]?.split(/^## /m)[0];
        if (!section) throw new Error('Missing release notes');
        await request(`${base}/releases/${release.id}`, {
            method: 'PATCH', headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ name: `Yntra Vault v${version}`, body: section.replace(/^[^\r\n]*\r?\n/, '').trim(), draft: false, prerelease: true, make_latest: 'false' }),
        });
    }
    if (process.env.GITHUB_STEP_SUMMARY) fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, `${platform} download: https://github.com/${repo}/releases/tag/${tag}\n`);
    console.log(`${platform} preview packages verified; existing assets and stable update metadata preserved.`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) await publishPreview(process.argv[2]);
