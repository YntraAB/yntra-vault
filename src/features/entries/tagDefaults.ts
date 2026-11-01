export function normalizeTagUrl(value: string): string {
  const clean = value.trim();
  if (!clean) return '';
  if (clean.length > 2048 || clean.includes('\\') || [...clean].some(char => char.charCodeAt(0) < 32 || char.charCodeAt(0) === 127)) throw new Error('Invalid URL');
  const url = new URL(clean.includes('://') ? clean : `https://${clean}`);
  if (!['http:', 'https:'].includes(url.protocol) || !url.hostname || url.username || url.password) throw new Error('Invalid URL');
  return url.href;
}
