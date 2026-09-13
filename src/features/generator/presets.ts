export interface DomainRule {
  domains: string[];
  length: number;
  uppercase: boolean;
  lowercase: boolean;
  digits: boolean;
  symbols: boolean;
  excludeAmbiguous: boolean;
  displayName: string;
}

export const DOMAIN_RULES: DomainRule[] = [
  {
    domains: ['google.com', 'gmail.com', 'youtube.com'],
    length: 32,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'Google Preset'
  },
  {
    domains: ['github.com'],
    length: 32,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'GitHub Preset (max 72)'
  },
  {
    domains: ['discord.com'],
    length: 32,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'Discord Preset'
  },
  {
    domains: ['apple.com', 'icloud.com'],
    length: 20,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: false,
    excludeAmbiguous: true,
    displayName: 'Apple ID Preset (Alphanumeric)'
  },
  {
    domains: ['microsoft.com', 'live.com', 'outlook.com', 'hotmail.com'],
    length: 16,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'Microsoft Preset (max 16)'
  },
  {
    domains: ['paypal.com'],
    length: 20,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'PayPal Preset (max 20)'
  },
  {
    domains: ['facebook.com', 'instagram.com', 'meta.com'],
    length: 32,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'Meta Preset'
  },
  {
    domains: ['x.com', 'twitter.com'],
    length: 32,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'X / Twitter Preset'
  },
  {
    domains: ['spotify.com'],
    length: 32,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'Spotify Preset'
  },
  {
    domains: ['netflix.com'],
    length: 32,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'Netflix Preset'
  },
  {
    domains: ['linkedin.com'],
    length: 32,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'LinkedIn Preset'
  },
  {
    domains: ['reddit.com'],
    length: 32,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'Reddit Preset'
  },
  {
    domains: ['steamcommunity.com', 'steampowered.com'],
    length: 32,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'Steam Preset'
  },
  {
    domains: ['amazon.com', 'amazon.se', 'amazon.co.uk', 'amazon.de'],
    length: 20,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'Amazon Preset'
  },
  {
    domains: ['roblox.com'],
    length: 30,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'Roblox Preset (max 30)'
  },
  {
    domains: ['nintendo.com', 'nintendo.se', 'nintendo.co.uk'],
    length: 20,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'Nintendo Preset (max 20)'
  },
  {
    domains: ['playstation.com', 'sony.com'],
    length: 30,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: true,
    excludeAmbiguous: true,
    displayName: 'PlayStation Preset (max 30)'
  },
  {
    domains: ['swedbank.se', 'nordea.se', 'seb.se', 'handelsbanken.se', 'avanza.se'],
    length: 16,
    uppercase: true,
    lowercase: true,
    digits: true,
    symbols: false,
    excludeAmbiguous: true,
    displayName: 'Bank Preset (Alphanumeric max 16)'
  }
];

export function getDomainName(url: string): string {
  if (!url) return '';
  let cleaned = url.trim().toLowerCase();
  if (!cleaned.includes('://')) {
    cleaned = 'https://' + cleaned;
  }
  try {
    const parsed = new URL(cleaned);
    let host = parsed.hostname;
    if (host.startsWith('www.')) {
      host = host.slice(4);
    }
    return host;
  } catch {
    let domain = url.trim().toLowerCase();
    domain = domain.replace(/^(https?:\/\/)?(www\.)?/, '');
    domain = domain.split('/')[0];
    return domain;
  }
}
