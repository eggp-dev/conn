import type { Terminal } from "@xterm/xterm";

/** A view of the displayed viewport, never the terminal's hidden history. */
export type SurfaceFrame = {
  surfaceId: string; generation: number; revision: number; outputSeq: number;
  rows: number; cols: number; cursor: { row: number; col: number } | null;
  screen: string[]; alternateScreen: boolean; visible: true;
  image?: { mimeType: "image/png"; data: string }; imageUnavailable?: boolean;
};
export type SurfaceStamp = Pick<SurfaceFrame, "surfaceId" | "generation" | "revision" | "outputSeq">;

/** Public xterm cells preserve display width while suppressing concealed text. */
export function viewportFrame(term: Pick<Terminal, "buffer" | "rows" | "cols">, stamp: SurfaceStamp): SurfaceFrame {
  const buffer = term.buffer.active;
  const screen: string[] = [];
  for (let row = 0; row < term.rows; row++) {
    const line = buffer.getLine(buffer.viewportY + row);
    let text = "";
    for (let col = 0; col < term.cols; col++) {
      const cell = line?.getCell(col);
      if (!cell) { text += " "; continue; }
      const width = cell.getWidth();
      if (!width) continue;
      // A wide concealed glyph must occupy two blank columns, not leak its text.
      text += cell.isInvisible() ? " ".repeat(width) : cell.getChars() || " ";
    }
    screen.push(text.replace(/ +$/, ""));
  }
  const cursorRow = buffer.baseY + buffer.cursorY - buffer.viewportY;
  return { ...stamp, rows: term.rows, cols: term.cols, screen,
    cursor: cursorRow >= 0 && cursorRow < term.rows ? { row: cursorRow, col: Math.min(buffer.cursorX, term.cols - 1) } : null,
    alternateScreen: buffer.type === "alternate", visible: true };
}

/** Export the DOM renderer's painted rows. Its computed style is the visibility authority.
 * Cells in the buffer alone cannot account for conceal or foreground=background colors.
 * Return null for another renderer rather than fall back to hidden backing text.
 */
export function renderedViewportFrame(term: Terminal, host: HTMLElement, stamp: SurfaceStamp): SurfaceFrame | null {
  const rows = host.querySelector(".xterm-rows");
  if (!rows || rows.children.length !== term.rows) return null;
  const buffer = term.buffer.active;
  const cursorRow = buffer.baseY + buffer.cursorY - buffer.viewportY;
  // Buffer metadata locates the cursor only; frame text comes solely from painted DOM.
  const frame: SurfaceFrame = { ...stamp, rows:term.rows, cols:term.cols, screen:[],
    cursor: rows.querySelector(".xterm-cursor") && cursorRow >= 0 && cursorRow < term.rows ? {row:cursorRow,col:Math.min(buffer.cursorX,term.cols-1)} : null,
    alternateScreen:buffer.type === "alternate", visible:true };

  const cellWidth = host.querySelector<HTMLElement>(".xterm-screen")!.clientWidth / term.cols;
  frame.screen = Array.from(rows.children).map(row => {
    let text = "";
    for (const node of Array.from(row.children)) {
      const el = node as HTMLElement;
      const style = getComputedStyle(el);
      const content = el.textContent ?? "";
      const hidden = style.visibility !== "visible" || style.display === "none" || Number(style.opacity) === 0;
      // Resolve a transparent span's background against its row/terminal surface.
      let bg = style.backgroundColor;
      let parent = el.parentElement;
      while ((bg === "rgba(0, 0, 0, 0)" || bg === "transparent") && parent) {
        bg = getComputedStyle(parent).backgroundColor; parent = parent.parentElement;
      }
      const concealed = hidden || style.color === bg || style.color === "transparent" || /^rgba\([^)]*,\s*0(?:\.0+)?\)$/.test(style.color);
      text += concealed ? " ".repeat(Math.max(0, Math.round(el.getBoundingClientRect().width / cellWidth))) : content;
    }
    return text.replace(/ +$/, "");
  });
  return frame;
}

export type SurfaceVisibility = { active: boolean; attended: boolean; shared: boolean; focused: boolean; documentVisible: boolean; obscured: boolean; ready: boolean };
export function canPublishSurface(state: SurfaceVisibility): boolean {
  return state.active && state.attended && state.shared && state.focused && state.documentVisible && state.ready && !state.obscured;
}

/** One ordered channel: invalidation cannot be overtaken by an older publication. */
export function surfaceChannel(send: (command: string, args: Record<string, unknown>) => Promise<unknown>, session: string, surfaceId: string) {
  let queue = Promise.resolve();
  let epoch = 0;
  let disposed = false;
  const enqueue = (job: () => Promise<unknown>) => { const result = queue.then(job); queue = result.then(() => {}, () => {}); return result; };
  return {
    publish(frame: SurfaceFrame) {
      const version = epoch;
      return enqueue(() => !disposed && version === epoch ? send("publish_surface", { session, frame }) : Promise.resolve());
    },
    invalidate() { epoch++; return enqueue(() => send("invalidate_surface", { session, surfaceId })).catch(() => {}); },
    dispose() { disposed = true; epoch++; return enqueue(() => send("invalidate_surface", { session, surfaceId })).catch(() => {}); },
  };
}


/** Rasterize the actual rendered DOM, including xterm's selection/cursor layers.
 * Never send the intermediate SVG: it contains concealed DOM text. Only pixels leave.
 * Unsupported WebKit/canvas combinations return null; text remains explicitly text-only.
 */
export async function rasterizeSurface(screen: HTMLElement): Promise<SurfaceFrame["image"] | null> {
  let url: string | undefined;
  try {
    const bounds = screen.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return null;
    const clone = screen.cloneNode(true) as HTMLElement;
    const originals = [screen, ...Array.from(screen.querySelectorAll<HTMLElement>("*"))];
    const copies = [clone, ...Array.from(clone.querySelectorAll<HTMLElement>("*"))];
    const styles = new Map<string, string>();
    originals.forEach((el, index) => {
      const computed = getComputedStyle(el);
      const copy = copies[index];
      const declarations: string[] = [];
      for (let i = 0; i < computed.length; i++) {
        const name = computed.item(i);
        declarations.push(`${name}:${computed.getPropertyValue(name)}`);
      }
      const css = declarations.join(";") + ";animation:none;transition:none";
      let name = styles.get(css);
      if (!name) { name = `conn-raster-${styles.size}`; styles.set(css, name); }
      // Identical rows/cells share one complete computed-style rule. This keeps
      // the image document small without discarding any visibility property.
      copy.removeAttribute("style"); copy.setAttribute("class", name);
    });
    const stylesheet = document.createElement("style");
    stylesheet.textContent = [...styles].map(([css,name]) => `.${name}{${css}}`).join("\n");
    clone.prepend(stylesheet);
    let background = getComputedStyle(screen).backgroundColor;
    let ancestor = screen.parentElement;
    while ((background === "transparent" || background === "rgba(0, 0, 0, 0)") && ancestor) { background = getComputedStyle(ancestor).backgroundColor; ancestor = ancestor.parentElement; }
    clone.style.backgroundColor = background;
    clone.style.position = "relative"; clone.style.left = "0"; clone.style.top = "0"; clone.style.margin = "0";
    clone.setAttribute("xmlns", "http://www.w3.org/1999/xhtml");
    const markup = new XMLSerializer().serializeToString(clone);
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${bounds.width}" height="${bounds.height}"><foreignObject width="100%" height="100%">${markup}</foreignObject></svg>`;
    url = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg);
    const img = new Image();
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("surface raster timeout")), 1500);
      img.onload = () => { clearTimeout(timer); resolve(); };
      img.onerror = () => { clearTimeout(timer); reject(new Error("surface raster unsupported")); };
      img.src = url!;
    });
    const canvas = document.createElement("canvas");
    canvas.width = Math.ceil(bounds.width); canvas.height = Math.ceil(bounds.height);
    const ctx = canvas.getContext("2d"); if (!ctx) return null;
    ctx.drawImage(img, 0, 0);
    const data = canvas.toDataURL("image/png").split(",")[1];
    return data && data.length <= 4_000_000 ? { mimeType: "image/png", data } : null;
  } catch { return null; }
  finally { if (url?.startsWith("blob:")) URL.revokeObjectURL(url); }
}
