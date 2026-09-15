import type { ITheme } from "@xterm/xterm";

export type ThemeId = "midnight" | "paper" | "solar" | "nord" | "mono";
export type Theme = {
  id: ThemeId;
  name: string;
  kind: "dark" | "light";
  blurb: string;
  tokens: { bg: string; surface: string; surface2: string; fg: string; muted: string; line: string; warn: string; ok: string; danger: string; selection: string };
  xterm: Omit<ITheme, "cursor" | "cursorAccent" | "selectionBackground">;
};

const T = (t: Theme) => t;

export const THEMES: Record<ThemeId, Theme> = {
  midnight: T({
    id: "midnight", name: "Midnight", kind: "dark", blurb: "theme.midnight",
    tokens: { bg: "#0b0d12", surface: "#12151c", surface2: "#1a1e28", fg: "#d7dae0", muted: "#7d8590", line: "#232833", warn: "#f5a524", ok: "#34d399", danger: "#f87171", selection: "rgba(139,124,255,.28)" },
    xterm: { background: "#0b0d12", foreground: "#d7dae0", black: "#1a1e28", red: "#f87171", green: "#34d399", yellow: "#f5a524", blue: "#60a5fa", magenta: "#c084fc", cyan: "#22d3ee", white: "#d7dae0", brightBlack: "#5b6270", brightRed: "#fca5a5", brightGreen: "#6ee7b7", brightYellow: "#fcd34d", brightBlue: "#93c5fd", brightMagenta: "#d8b4fe", brightCyan: "#67e8f9", brightWhite: "#ffffff" },
  }),
  paper: T({
    id: "paper", name: "Paper", kind: "light", blurb: "theme.paper",
    tokens: { bg: "#f7f6f2", surface: "#ffffff", surface2: "#efede7", fg: "#1f2328", muted: "#6b6f76", line: "#e2dfd8", warn: "#b45309", ok: "#047857", danger: "#b91c1c", selection: "rgba(99,102,241,.2)" },
    xterm: { background: "#f7f6f2", foreground: "#1f2328", black: "#1f2328", red: "#b91c1c", green: "#047857", yellow: "#b45309", blue: "#1d4ed8", magenta: "#7e22ce", cyan: "#0e7490", white: "#e2dfd8", brightBlack: "#6b6f76", brightRed: "#dc2626", brightGreen: "#059669", brightYellow: "#d97706", brightBlue: "#2563eb", brightMagenta: "#9333ea", brightCyan: "#0891b2", brightWhite: "#ffffff" },
  }),
  solar: T({
    id: "solar", name: "Solar", kind: "dark", blurb: "theme.solar",
    tokens: { bg: "#002b36", surface: "#073642", surface2: "#0b4552", fg: "#eee8d5", muted: "#93a1a1", line: "#0e4f5c", warn: "#b58900", ok: "#859900", danger: "#dc322f", selection: "rgba(38,139,210,.3)" },
    xterm: { background: "#002b36", foreground: "#eee8d5", black: "#073642", red: "#dc322f", green: "#859900", yellow: "#b58900", blue: "#268bd2", magenta: "#d33682", cyan: "#2aa198", white: "#eee8d5", brightBlack: "#586e75", brightRed: "#cb4b16", brightGreen: "#93a1a1", brightYellow: "#657b83", brightBlue: "#839496", brightMagenta: "#6c71c4", brightCyan: "#93a1a1", brightWhite: "#fdf6e3" },
  }),
  nord: T({
    id: "nord", name: "Nord", kind: "dark", blurb: "theme.nord",
    tokens: { bg: "#2e3440", surface: "#3b4252", surface2: "#434c5e", fg: "#e5e9f0", muted: "#9aa5b8", line: "#4c566a", warn: "#ebcb8b", ok: "#a3be8c", danger: "#bf616a", selection: "rgba(136,192,208,.3)" },
    xterm: { background: "#2e3440", foreground: "#e5e9f0", black: "#3b4252", red: "#bf616a", green: "#a3be8c", yellow: "#ebcb8b", blue: "#81a1c1", magenta: "#b48ead", cyan: "#88c0d0", white: "#e5e9f0", brightBlack: "#4c566a", brightRed: "#bf616a", brightGreen: "#a3be8c", brightYellow: "#ebcb8b", brightBlue: "#81a1c1", brightMagenta: "#b48ead", brightCyan: "#8fbcbb", brightWhite: "#eceff4" },
  }),
  mono: T({
    id: "mono", name: "Mono", kind: "dark", blurb: "theme.mono",
    tokens: { bg: "#000000", surface: "#0d0d0d", surface2: "#171717", fg: "#e5e5e5", muted: "#737373", line: "#262626", warn: "#e5e5e5", ok: "#a3a3a3", danger: "#ffffff", selection: "rgba(255,255,255,.18)" },
    xterm: { background: "#000000", foreground: "#e5e5e5", black: "#171717", red: "#d4d4d4", green: "#a3a3a3", yellow: "#e5e5e5", blue: "#9ca3af", magenta: "#d1d5db", cyan: "#a1a1aa", white: "#e5e5e5", brightBlack: "#525252", brightRed: "#fafafa", brightGreen: "#d4d4d4", brightYellow: "#ffffff", brightBlue: "#d1d5db", brightMagenta: "#e5e7eb", brightCyan: "#d4d4d8", brightWhite: "#ffffff" },
  }),
};

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
  return { ...t.xterm, cursor: t.tokens.fg, cursorAccent: t.tokens.bg, selectionBackground: t.tokens.selection };
}
