/** Preserve advertised ports and IPv6; bare addresses use the sync port. */
export function syncEndpoint(value: string): string {
  const address = value.trim();
  if (!address || /[\s/?#@]/.test(address)) throw new Error('Enter a valid device address.');
  if (address.startsWith('[')) {
    if (/^\[[^\]]+\]$/.test(address)) return `${address}:5322`;
    if (!/^\[[^\]]+\]:\d+$/.test(address)) throw new Error('Invalid IPv6 address.');
  } else if ((address.match(/:/g) || []).length > 1) {
    return `[${address}]:5322`;
  } else if (!address.includes(':')) {
    return `${address}:5322`;
  }
  const port = Number(address.slice(address.lastIndexOf(':') + 1));
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('Invalid port.');
  return address;
}
