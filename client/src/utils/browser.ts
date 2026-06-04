export function getKeySnapshot(event: KeyboardEvent): string {
  const parts: string[] = [];
  if (event.metaKey) {
    parts.push('meta');
  }
  if (event.ctrlKey) {
    parts.push('ctrl');
  }
  if (event.altKey) {
    parts.push('alt');
  }
  if (event.shiftKey) {
    parts.push('shift');
  }
  parts.push(normalizeKey(event.key));
  return parts.join('-');
}

const KEY_ALIASES: Record<string, string> = {
  ' ': 'space',
  Spacebar: 'space',
  Esc: 'escape',
};

const normalizeKey = (key: string): string =>
  (KEY_ALIASES[key] ?? key)
    .replace(/([a-z0-9])([A-Z])/g, '$1-$2')
    .toLowerCase();
