export const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

/** Reserve Ctrl+Shift on PC keyboards, leaving ordinary terminal Ctrl keys alone. */
export function appShortcut(e: KeyboardEvent): boolean {
  return isMac ? e.metaKey : e.ctrlKey && e.shiftKey;
}

/** Physical keys keep shifted punctuation and number shortcuts usable. */
export function shortcutKey(e: KeyboardEvent): string {
  if (/^Key[A-Z]$/.test(e.code)) return e.code.slice(3).toLowerCase();
  if (/^Digit[1-9]$/.test(e.code)) return e.code.slice(5);
  return ({ Comma: ",", BracketLeft: "[", BracketRight: "]" } as Record<string, string>)[e.code] ?? e.key;
}

export function shortcutLabel(label: string): string {
  return isMac ? label : label.replace(/⌘/g, "Ctrl+Shift+").replace(/⌥/g, "Alt+").replace(/⏎/g, "Enter");
}
