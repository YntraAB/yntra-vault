export type CompetitorBrand =
  | 'auto_detect'
  | 'bitwarden'
  | 'onepassword'
  | 'keepass'
  | 'chrome'
  | 'lastpass'
  | 'dashlane'
  | 'protonpass'
  | 'generic';

export interface BrandInfo {
  id: CompetitorBrand;
  name: string;
  badge: string;
  description: string;
  instructions: string[];
  supportedFormatKey: string;
}

export function BrandLogo({ brandId, className = "h-5 w-5" }: { brandId: CompetitorBrand; className?: string }) {
  switch (brandId) {
    case 'auto_detect':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <rect width="24" height="24" rx="5" fill="#6366F1" />
          <path d="M12 4l1.8 3.6L18 9.4l-3 2.9.7 4.2-3.7-2-3.7 2 .7-4.2-3-2.9 4.2-1.8L12 4z" fill="#FFFFFF" />
        </svg>
      );
    case 'bitwarden':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <path d="M12 1.75L3.5 5.5v6.25c0 5.86 3.63 11 8.5 12.5 4.87-1.5 8.5-6.64 8.5-12.5V5.5L12 1.75z" fill="#175DDC" />
          <path d="M12 4.25v16.5c3.67-1.35 6.5-5.54 6.5-10.25V7.1L12 4.25z" fill="#1148AA" />
          <path d="M12 8.5a2.5 2.5 0 00-2.5 2.5v3h5v-3A2.5 2.5 0 0012 8.5z" fill="#FFFFFF" />
        </svg>
      );
    case 'onepassword':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <rect width="24" height="24" rx="5" fill="#0094F5" />
          <circle cx="12" cy="12" r="6" stroke="#FFFFFF" strokeWidth="2.2" fill="none" />
          <rect x="10.8" y="8" width="2.4" height="8" rx="1.2" fill="#FFFFFF" />
        </svg>
      );
    case 'keepass':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <rect width="24" height="24" rx="5" fill="#369336" />
          <path d="M7 12a3.5 3.5 0 116.12 2.3l.58.58v1.62h1.62v1.62H17v1.62h-1.62L13.1 17.5a3.5 3.5 0 01-6.1-5.5z" fill="#FFFFFF" />
          <circle cx="9.5" cy="10.5" r="1" fill="#369336" />
        </svg>
      );
    case 'chrome':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <circle cx="12" cy="12" r="10" fill="#4285F4" />
          <path d="M12 12L17.2 3.8A10 10 0 006.8 3.8L12 12z" fill="#EA4335" />
          <path d="M12 12L6.8 3.8A10 10 0 0012 22L17.2 12H12z" fill="#34A853" />
          <path d="M12 12H22A10 10 0 0017.2 3.8L12 12z" fill="#FBBC05" />
          <circle cx="12" cy="12" r="4.5" fill="#FFFFFF" />
          <circle cx="12" cy="12" r="3.5" fill="#1A73E8" />
        </svg>
      );
    case 'lastpass':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <rect width="24" height="24" rx="5" fill="#D32F2F" />
          <circle cx="7.5" cy="12" r="1.8" fill="#FFFFFF" />
          <path d="M13 8.5h4v7h-4z" fill="#FFFFFF" />
        </svg>
      );
    case 'dashlane':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <rect width="24" height="24" rx="5" fill="#0F7060" />
          <path d="M7 6.5h6a4.5 4.5 0 010 9H7v-9zm2.5 2.5v4h3.5a2 2 0 100-4h-3.5z" fill="#FFFFFF" />
        </svg>
      );
    case 'protonpass':
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <rect width="24" height="24" rx="5" fill="#6D4AFF" />
          <path d="M12 5L6 8v4c0 4.2 2.7 8 6 9 3.3-1 6-4.8 6-9V8l-6-3z" fill="#FFFFFF" fillOpacity="0.25" />
          <circle cx="12" cy="10.5" r="2" fill="#FFFFFF" />
          <path d="M12 12.5v4" stroke="#FFFFFF" strokeWidth="2" strokeLinecap="round" />
        </svg>
      );
    case 'generic':
    default:
      return (
        <svg viewBox="0 0 24 24" fill="none" className={className}>
          <rect width="24" height="24" rx="5" fill="#4B5563" />
          <path d="M7 7h10v10H7V7zm2 2v2h2V9H9zm4 0v2h2V9h-2zm-4 4v2h2v-2H9zm4 0v2h2v-2h-2z" fill="#FFFFFF" fillOpacity="0.9" />
        </svg>
      );
  }
}

export const BRANDS: BrandInfo[] = [
  {
    id: 'auto_detect',
    name: 'Smart Auto-Detect',
    badge: 'Recommended',
    description: 'Select or drop any export file (JSON, CSV, XML). Format is automatically recognized.',
    instructions: [
      'Export your vault from your existing password manager',
      'Drop the file directly into the dropzone or select it from disk',
      'Yntra Vault will automatically identify the format and structure',
    ],
    supportedFormatKey: 'auto',
  },
  {
    id: 'bitwarden',
    name: 'Bitwarden',
    badge: 'JSON / CSV',
    description: 'Export your vault as encrypted or unencrypted JSON or CSV from Bitwarden.',
    instructions: [
      'Open Bitwarden Web Vault or Desktop app',
      'Go to Settings → Export Vault',
      'Select JSON or CSV format and enter your master password',
      'Save the file to your computer and select it below',
    ],
    supportedFormatKey: 'bitwarden_json',
  },
  {
    id: 'onepassword',
    name: '1Password',
    badge: '1PUX / CSV',
    description: 'Export logins and items from 1Password 7 or 8 as CSV format.',
    instructions: [
      'Open 1Password on your desktop',
      'Click File → Export → All Items',
      'Choose CSV format and save the exported file',
      'Select the generated CSV file below',
    ],
    supportedFormatKey: 'onepassword_csv',
  },
  {
    id: 'keepass',
    name: 'KeePass / XC',
    badge: 'XML / CSV',
    description: 'Import database entries exported from KeePass 2.x or KeePassXC.',
    instructions: [
      'Open KeePassXC or KeePass 2.x',
      'Go to Database → Export → XML or CSV File',
      'Confirm export and save to disk',
      'Select the XML or CSV file below',
    ],
    supportedFormatKey: 'keepass_csv',
  },
  {
    id: 'chrome',
    name: 'Google Chrome / Edge',
    badge: 'Browser CSV',
    description: 'Import passwords saved in Chrome, Edge, Brave, or Firefox browsers.',
    instructions: [
      'Open Chrome/Edge Settings → Passwords',
      'Click the three dots next to Saved Passwords → Export passwords',
      'Confirm with your OS password and save CSV',
      'Select the browser CSV file below',
    ],
    supportedFormatKey: 'chrome_csv',
  },
  {
    id: 'lastpass',
    name: 'LastPass',
    badge: 'CSV Export',
    description: 'Import items exported from your LastPass vault.',
    instructions: [
      'Log into LastPass browser extension or website',
      'Go to Advanced Options → Export',
      'Enter Master Password and download CSV',
      'Select the LastPass CSV file below',
    ],
    supportedFormatKey: 'lastpass_csv',
  },
  {
    id: 'dashlane',
    name: 'Dashlane',
    badge: 'CSV Export',
    description: 'Import credentials exported from Dashlane web application.',
    instructions: [
      'Open Dashlane Web App → Settings → Export Data',
      'Choose CSV format and export',
      'Select the exported CSV file below',
    ],
    supportedFormatKey: 'dashlane_csv',
  },
  {
    id: 'protonpass',
    name: 'Proton Pass',
    badge: 'JSON / CSV',
    description: 'Import logins and notes exported from Proton Pass.',
    instructions: [
      'Open Proton Pass web or desktop application',
      'Go to Settings → Export Vault → JSON or CSV',
      'Select the exported file below',
    ],
    supportedFormatKey: 'protonpass_json',
  },
  {
    id: 'generic',
    name: 'Generic CSV',
    badge: 'Any CSV',
    description: 'Import from any password manager CSV with automatic column header detection.',
    instructions: [
      'Ensure CSV has column headers like Title, Username, Password, URL, Notes',
      'Save as .csv file',
      'Select the file below for auto-detection',
    ],
    supportedFormatKey: 'generic_csv',
  },
];
