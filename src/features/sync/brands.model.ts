
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