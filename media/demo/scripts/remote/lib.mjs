// Shared helpers for the remote-takeover film: start the real Conn web harness, the real
// Svelte frontend and a dedicated headless Chrome, and drive them over CDP. Node 22+, no dependencies.
import { spawn } from "node:child_process";
import { mkdir, writeFile, access, readFile } from "node:fs/promises";

export const sleep = ms => new Promise(r => setTimeout(r, ms));
export async function until(what, fn, ms = 20000, every = 120) {
  const end = Date.now() + ms;
  while (Date.now() < end) { try { const v = await fn(); if (v) return v; } catch {} await sleep(every); }
  throw new Error("timeout: " + what);
}

export async function launch({ repo, state, uiPort = 1491, cdpPort = 9347, width = 1920, height = 1080, scale = 1 }) {
  const kids = [];
  const run = (cmd, args, opt = {}) => { const c = spawn(cmd, args, { stdio: "ignore", ...opt }); kids.push(c); return c; };
  await mkdir(state, { recursive: true, mode: 0o700 });
  run(`${repo}/target/debug/conn-web`, ['serve', '--state-dir', state, '--port', String(uiPort), '--ui-dir', `${repo}/frontends/web/dist`, '--setup-home', `${state}/setup-home`]);
  await until("frontend", async () => (await fetch(`http://127.0.0.1:${uiPort}/`)).ok);
  const connection = JSON.parse(await readFile(`${state}/connection.json`, 'utf8'));
  run("google-chrome", ["--headless=new", "--no-sandbox", "--hide-scrollbars", `--force-device-scale-factor=${scale}`, "--font-render-hinting=none", `--remote-debugging-port=${cdpPort}`, `--user-data-dir=${state}/chrome`, `--window-size=${width},${height}`, "about:blank"]);
  const target = await until("chrome", async () => (await (await fetch(`http://127.0.0.1:${cdpPort}/json`)).json()).find(t => t.type === "page"));
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise(r => (ws.onopen = r));
  let seq = 0; const waiting = new Map(); const listeners = new Map(); const errors = [];
  ws.onmessage = e => {
    const m = JSON.parse(e.data);
    if (m.id && waiting.has(m.id)) { waiting.get(m.id)(m.result ?? m.error); waiting.delete(m.id); return; }
    if (m.method === "Runtime.exceptionThrown") errors.push(m.params.exceptionDetails.text);
    listeners.get(m.method)?.(m.params);
  };
  const cdp = (method, params = {}) => new Promise(r => { const id = ++seq; waiting.set(id, r); ws.send(JSON.stringify({ id, method, params })); });
  const on = (method, fn) => listeners.set(method, fn);
  const js = async expr => (await cdp("Runtime.evaluate", { expression: expr, returnByValue: true, awaitPromise: true })).result?.value;
  await cdp("Runtime.enable"); await cdp("Page.enable");
  await cdp("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: scale, mobile: false });
  const stop = async () => { try { ws.close(); } catch {} for (const k of kids) k.kill("SIGTERM"); await sleep(600); };
  return { cdp, on, js, errors, stop, url: `http://127.0.0.1:${uiPort}/#token=${encodeURIComponent(connection.bootstrapToken)}`, sock: `${state}/conn.sock` };
}

// The human role: real key and mouse events delivered to the page, never a test-only API.
export function human({ cdp, js }, log) {
  const key = async (k, code, vk, text) => {
    await cdp("Input.dispatchKeyEvent", { type: text ? "keyDown" : "rawKeyDown", key: k, code, windowsVirtualKeyCode: vk, text });
    await cdp("Input.dispatchKeyEvent", { type: "keyUp", key: k, code, windowsVirtualKeyCode: vk });
  };
  return {
    async type(text, { delay = 55, jitter = 35, label } = {}) {
      log?.({ event: "human_type", label: label ?? text });
      for (const ch of text) { await cdp("Input.insertText", { text: ch }); await sleep(delay + Math.random() * jitter); }
    },
    enter: () => key("Enter", "Enter", 13, "\r"),
    async click(selectorExpr, label) {
      const box = await js(`(() => { const el = ${selectorExpr}; if (!el) return null; const r = el.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; })()`);
      if (!box) throw new Error("nothing to click: " + label);
      log?.({ event: "click", label, x: box.x, y: box.y });
      await cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: box.x, y: box.y });
      await sleep(140);
      await cdp("Input.dispatchMouseEvent", { type: "mousePressed", x: box.x, y: box.y, button: "left", clickCount: 1 });
      await sleep(90);
      await cdp("Input.dispatchMouseEvent", { type: "mouseReleased", x: box.x, y: box.y, button: "left", clickCount: 1 });
    },
  };
}

export const shot = async ({ cdp }, path) => writeFile(path, Buffer.from((await cdp("Page.captureScreenshot", { format: "png" })).data, "base64"));
export { readFile, writeFile, access, mkdir };
