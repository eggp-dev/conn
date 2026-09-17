import { cmd } from './bridge';
import { st } from './store.svelte';
import { THEMES, type ThemeId } from './themes';

export type Extension = { id: string; name: string; kind: string; enabled: boolean; capabilities: string[] };
export type ExtensionTheme = { id: string; name: string; background: string; foreground: string; cursor: string; selection: string; ansi: string[] };
export type ExtensionCatalog = {
  apiVersion: number; extensions: Extension[];
  settings: {theme: string; model: string; completionEnabled: boolean; providerEnabled: boolean};
  keyStatus: 'missing' | 'stored' | 'unavailable'; themes: ExtensionTheme[];
};
export const extensions = $state<{catalog: ExtensionCatalog | null}>({catalog:null});
const ANSI = ['black','red','green','yellow','blue','magenta','cyan','white','brightBlack','brightRed','brightGreen','brightYellow','brightBlue','brightMagenta','brightCyan','brightWhite'] as const;
export async function refreshExtensions() {
  const catalog = await cmd<ExtensionCatalog>('extensions_status');
  if (catalog.apiVersion !== 1) throw new Error('Unsupported extension API');
  for (const theme of catalog.themes) {
    const id = theme.id.replace(/^conn\.theme\./, '') as ThemeId;
    const rgb = parseInt(theme.background.replace('#',''),16);
    const light = ((rgb >> 16) * .299 + ((rgb >> 8) & 255) * .587 + (rgb & 255) * .114) > 160;
    const base = THEMES[id] ?? THEMES[light ? 'paper' : 'midnight'];
    // Registry supplies only supported tokens; trusted status/approval colors stay in Conn.
    THEMES[id] = {...base, id, name:theme.name, cursor:theme.cursor, tokens:{...base.tokens,bg:theme.background,fg:theme.foreground,selection:theme.selection}, xterm:{...base.xterm, background:theme.background,foreground:theme.foreground,...Object.fromEntries(ANSI.map((key,i) => [key,theme.ansi[i]]))}};
  }
  const id = catalog.settings.theme.replace(/^conn\.theme\./, '') as ThemeId;
  if (THEMES[id]) { st.theme = id; localStorage.setItem('ss:theme', id); }
  st.themeRevision++;
  extensions.catalog = catalog;
  return catalog;
}
export async function selectTheme(id: string) {
  await cmd('extension_configure', {id:extensions.catalog?.themes.find(theme => theme.id === id || theme.id === `conn.theme.${id}`)?.id ?? `conn.theme.${id}`,config:{enabled:true}});
  await refreshExtensions();
}
