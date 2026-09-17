/**
 * Formats IP address or hostname input in real time as the user types.
 *
 * Behavior:
 * - If input starts with a letter (e.g. 'localhost', 'my-pc.local'), permits hostname characters without IPv4 dot auto-insertion.
 * - For IPv4 input:
 *   - Automatically appends a dot '.' after a 3-digit octet is typed.
 *   - Allows manual dot typing for 1-digit or 2-digit octets without duplicating dots.
 *   - Clamps 3-digit octets exceeding 255 to 255.
 *   - Permits smooth backspacing without re-inserting deleted dots.
 *   - Restricts to at most 4 octets.
 * - Preserves and cleans optional port suffix (e.g. ':5324').
 */
export function formatIpv4Input(value: string, prevValue: string = ''): string {
  // If user is deleting (backspace), allow deleting without immediately re-inserting dot
  if (value.length < prevValue.length) {
    return value;
  }

  const trimmed = value.trimStart();
  const colonIdx = trimmed.indexOf(':');
  let hostPart = colonIdx >= 0 ? trimmed.slice(0, colonIdx) : trimmed;
  let portPart = colonIdx >= 0 ? trimmed.slice(colonIdx + 1) : '';

  // Clean port: keep digits only, limit to 5 chars (max 65535)
  if (colonIdx >= 0) {
    portPart = ':' + portPart.replace(/\D/g, '').slice(0, 5);
  }

  // Hostname check: if host begins with letter, allow standard hostname syntax
  if (/^[a-zA-Z]/.test(hostPart)) {
    const cleanHost = hostPart.replace(/[^a-zA-Z0-9.-]/g, '');
    return cleanHost + portPart;
  }

  // Only allow digits and dots, squash consecutive dots
  hostPart = hostPart.replace(/[^\d.]/g, '').replace(/\.{2,}/g, '.');

  const rawOctets = hostPart.split('.');
  const formattedOctets: string[] = [];

  for (let i = 0; i < rawOctets.length && i < 4; i++) {
    let octet = rawOctets[i].slice(0, 3);
    if (octet.length === 3 && parseInt(octet, 10) > 255) {
      octet = '255';
    }
    formattedOctets.push(octet);

    // If octet has 3 digits and not at 4th octet, auto append dot if none exists next
    if (octet.length === 3 && i < 3 && i === rawOctets.length - 1) {
      formattedOctets.push('');
    }
  }

  let result = formattedOctets.join('.');
  const parts = result.split('.');
  if (parts.length > 4) {
    result = parts.slice(0, 4).join('.');
  }

  return result + portPart;
}
