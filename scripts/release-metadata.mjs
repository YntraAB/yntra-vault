import { createHash } from 'node:crypto';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

// Called only after every required binary is available in the release staging
// directory. Missing/empty packages fail instead of publishing empty checksums.
const [tag, directory = 'release-assets'] = process.argv.slice(2);
const version = JSON.parse(readFileSync('package.json', 'utf8')).version;
if (tag !== `v${version}`) throw new Error('Release tag does not match package version');
const suffixes = ['amd64.AppImage', 'amd64.deb', 'cli.exe', 'portable.exe', 'universal.apk', 'x64-setup.exe', 'x64_en-US.msi'];
// Optional builds still need update links and checksums when they are present.
const macPlatforms = { 'aarch64.dmg': 'darwin-aarch64', 'x64.dmg': 'darwin-x86_64' };
for (const suffix of Object.keys(macPlatforms)) {
  if (existsSync(join(directory, `Yntra.Vault_${version}_${suffix}`))) suffixes.push(suffix);
}
const packages = Object.fromEntries(suffixes.map(suffix => {
  const name = `Yntra.Vault_${version}_${suffix}`;
  const bytes = readFileSync(join(directory, name));
  if (!bytes.length) throw new Error(`Empty release package: ${name}`);
  return [suffix, { version, url: `https://github.com/YntraAB/yntra-vault/releases/download/${tag}/${name}`, sha256: createHash('sha256').update(bytes).digest('hex') }];
}));
const changelog = readFileSync('CHANGELOG.md', 'utf8');
const section = changelog.match(new RegExp(`^## \\[${version.replaceAll('.', '\\.')}\\][^\\r\\n]*\\r?\\n([\\s\\S]*?)(?=^## |$(?![\\s\\S]))`, 'm'));
if (!section) throw new Error('Release notes not found');
const notes = section[1].trim();
const manifest = {
  version, notes, pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': { ...packages['x64-setup.exe'], signature: '' },
    'linux-x86_64': { ...packages['amd64.AppImage'], signature: '' },
  },
  extra: {
    android: packages['universal.apk'],
    cli: { 'windows-x86_64': packages['cli.exe'] },
    portable: { 'windows-x86_64': packages['portable.exe'] },
  },
};
for (const [suffix, platform] of Object.entries(macPlatforms)) {
  if (packages[suffix]) manifest.platforms[platform] = { ...packages[suffix], signature: '' };
}
writeFileSync(join(directory, 'latest.json'), JSON.stringify(manifest, null, 2) + '\n');
writeFileSync(join(directory, 'SHA256SUMS'), suffixes.map(suffix => `${packages[suffix].sha256}  Yntra.Vault_${version}_${suffix}`).join('\n') + '\n');
writeFileSync('RELEASE_NOTES.md', `${notes}\n`);
