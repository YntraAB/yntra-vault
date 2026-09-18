import { describe, it, expect } from 'bun:test';
import { isAppPath, isWebUrl, getDomain, deriveTitle } from './utils';

describe('isAppPath & isWebUrl detection', () => {
  it('correctly classifies web URLs and domains as web (not apps)', () => {
    const webUrls = [
      'https://store.steampowered.com/',
      'https://store.steampowered.com/login/',
      'http://store.steampowered.com',
      'store.steampowered.com',
      'https://steamcommunity.com',
      'https://github.com/login',
      'https://auth.bank.co.uk',
      'http://localhost:3000',
      'sub.example.com/path?query=1',
    ];

    for (const url of webUrls) {
      expect(isAppPath(url)).toBe(false);
      expect(isWebUrl(url)).toBe(true);
    }
  });

  it('correctly classifies desktop application targets as apps (not web)', () => {
    const appTargets = [
      'steam://rungameid/730',
      'steam://',
      'spotify://',
      'discord://',
      'vscode://',
      'C:\\Program Files (x86)\\Steam\\steam.exe',
      'C:/Program Files/Discord/Discord.exe',
      'D:\\Games\\Game.exe',
      '\\\\nas\\apps\\client.exe',
      '/Applications/Steam.app',
      '/usr/bin/steam',
      'run.bat',
      'installer.msi',
    ];

    for (const target of appTargets) {
      expect(isAppPath(target)).toBe(true);
      expect(isWebUrl(target)).toBe(false);
    }
  });

  it('handles empty, null, and undefined values safely', () => {
    expect(isAppPath('')).toBe(false);
    expect(isAppPath(null)).toBe(false);
    expect(isAppPath(undefined)).toBe(false);

    expect(isWebUrl('')).toBe(false);
    expect(isWebUrl(null)).toBe(false);
    expect(isWebUrl(undefined)).toBe(false);
  });
});

describe('getDomain', () => {
  it('extracts clean hostname from web URLs', () => {
    expect(getDomain('https://store.steampowered.com/')).toBe('store.steampowered.com');
    expect(getDomain('https://www.steampowered.com')).toBe('steampowered.com');
    expect(getDomain('store.steampowered.com')).toBe('store.steampowered.com');
    expect(getDomain('https://github.com/login')).toBe('github.com');
  });

  it('returns null for application paths and protocol schemes', () => {
    expect(getDomain('steam://rungameid/730')).toBe(null);
    expect(getDomain('C:\\Program Files\\Steam\\steam.exe')).toBe(null);
    expect(getDomain('/Applications/Steam.app')).toBe(null);
  });
});

describe('deriveTitle', () => {
  it('preserves existing user titles', () => {
    expect(deriveTitle('Custom Steam Account', 'https://store.steampowered.com/')).toBe('Custom Steam Account');
  });

  it('derives titles from web domains properly', () => {
    expect(deriveTitle('', 'https://store.steampowered.com/')).toBe('Steampowered');
    expect(deriveTitle('', 'https://github.com')).toBe('Github');
  });

  it('derives titles from application schemes and executable paths', () => {
    expect(deriveTitle('', 'steam://rungameid/730')).toBe('Steam');
    expect(deriveTitle('', 'spotify://')).toBe('Spotify');
    expect(deriveTitle('', 'C:\\Program Files (x86)\\Steam\\steam.exe')).toBe('Steam');
  });
});
