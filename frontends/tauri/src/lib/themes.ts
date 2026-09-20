import type { ITheme } from "@xterm/xterm";
import builtin from "@conn/themes/builtin-themes.json";

export type ThemeId = string;
export type Theme = {
  id: ThemeId;
  name: string;
  kind: "dark" | "light";
  cursor?: string;
  blurb: string;
  tokens: { bg: string; surface: string; surface2: string; fg: string; muted: string; line: string; warn: string; ok: string; danger: string; selection: string };
  xterm: Omit<ITheme, "cursor" | "cursorAccent" | "selectionBackground">;
};

// Interface colors only. The terminal palette itself comes from @conn/themes, the same file the
// native registry compiles in; see packages/themes/README.md.
type Chrome = Pick<Theme, "kind"> & Omit<Theme["tokens"], "bg" | "fg" | "selection">;
const CHROME: Record<string, Chrome> = {
  midnight: { kind: "dark", surface: "#12151c", surface2: "#1a1e28", muted: "#7d8590", line: "#232833", warn: "#f5a524", ok: "#34d399", danger: "#f87171" },
  paper: { kind: "light", surface: "#ffffff", surface2: "#efede7", muted: "#6b6f76", line: "#e2dfd8", warn: "#b45309", ok: "#047857", danger: "#b91c1c" },
  solar: { kind: "dark", surface: "#073642", surface2: "#0b4552", muted: "#93a1a1", line: "#0e4f5c", warn: "#b58900", ok: "#859900", danger: "#dc322f" },
  nord: { kind: "dark", surface: "#3b4252", surface2: "#434c5e", muted: "#9aa5b8", line: "#4c566a", warn: "#ebcb8b", ok: "#a3be8c", danger: "#bf616a" },
  mono: { kind: "dark", surface: "#0d0d0d", surface2: "#171717", muted: "#737373", line: "#262626", warn: "#e5e5e5", ok: "#a3a3a3", danger: "#ffffff" },
};

export const ANSI = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white", "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightMagenta", "brightCyan", "brightWhite"] as const;

export const THEMES: Record<ThemeId, Theme> = Object.fromEntries(builtin.map((p) => {
  const id = p.id.replace(/^conn\.theme\./, "");
  const { kind, ...chrome } = CHROME[id];
  return [id, { id, name: p.name, kind, cursor: p.cursor, blurb: `theme.${id}`,
    tokens: { bg: p.background, fg: p.foreground, selection: p.selection, ...chrome },
    xterm: { background: p.background, foreground: p.foreground, ...Object.fromEntries(ANSI.map((name, i) => [name, p.ansi[i]])) } }];
}));

const AGENT_COLORS: Record<string, string> = { claude: "#8b7cff", copilot: "#22d3ee", codex: "#34d399", gemini: "#f472b6" };
const PALETTE = ["#8b7cff", "#22d3ee", "#34d399", "#f472b6", "#fb923c", "#a3e635"];

export function agentColor(id: string | undefined | null): string {
  if (!id) return "#8b7cff";
  const k = id.toLowerCase();
  for (const key of Object.keys(AGENT_COLORS)) if (k.includes(key)) return AGENT_COLORS[key];
  let h = 0;
  for (const c of k) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return PALETTE[h % PALETTE.length];
}

export function applyTheme(t: Theme, agent = "#8b7cff"): ITheme {
  const r = document.documentElement.style;
  for (const [k, v] of Object.entries(t.tokens)) r.setProperty(`--${k}`, v);
  r.setProperty("--agent", agent);
  document.documentElement.dataset.theme = t.id;
  document.documentElement.dataset.kind = t.kind;
  return { ...t.xterm, cursor: t.cursor ?? t.tokens.fg, cursorAccent: t.tokens.bg, selectionBackground: t.tokens.selection };
}
