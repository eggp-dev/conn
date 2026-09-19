// One continuous take of the remote-takeover film.
// Real Conn backend and frontend, a real SSH login, a real Claude Code session over Conn's MCP server.
// This script plays the human: it types in the terminal and clicks the real Allow / Approve / Deny buttons.
//   node direct.mjs <repo> <filmDir> <takeName>
import { spawn } from "node:child_process";
import { createWriteStream } from "node:fs";
import { launch, human, sleep, until, writeFile, mkdir, readFile } from "./lib.mjs";

const [repo, F, take = "take"] = process.argv.slice(2);
const out = `${F}/take/${take}`; await mkdir(`${out}/frames`, { recursive: true });
const t0 = Date.now(); const events = [];
const log = e => { const row = { t: (Date.now() - t0) / 1000, ...e }; events.push(row); console.log(JSON.stringify(row)); };

const PROMPT_1 = "The API on staging is down. Look at my Conn terminal and get it running again.";
const PROMPT_2 = "I cleared the stale lock myself, so keep the cache. Read the terminal and continue.";
// The brief is part of the staging and is disclosed with the film: it makes the wrong turn happen on camera.
const BRIEF = `You are working with a human through the Conn shared terminal (MCP tools named terminal_*). Use ONLY those tools.
The terminal is already logged in to the staging server over SSH, in ~/api. The service is managed by ./api.sh (status|start|unlock|health) and logs to api.log.
Work one short command at a time and give every ENTER a one-line intent. Never chain commands. Expect each command to wait for the human's review; poll terminal_check_approval when pending.
Plan: 1) ./api.sh status  2) tail -n 3 api.log  3) when you see the stale cache lock, your fix is to clear the whole cache with: rm -rf cache/  4) ./api.sh start  5) ./api.sh health, then release control.
Run exactly the commands in the plan, in that order, and no others (no ls, cat, pwd or extra checks): this is a timed recording.
If a command is denied, or the human takes the keyboard, stop immediately, do not retry or work around it, and end your turn with one sentence.
Keep all replies to one short sentence.`;

// A wide window keeps the terminal text and the review card close together, so no zoom has to crop either.
const VIEW = { width: 1200, height: 540 };
const b = await launch({ repo, state: `${F}/state`, ...VIEW, scale: 2 });
const h = human(b, log);
const screen = () => b.js(`document.querySelector(".xterm-rows")?.innerText ?? ""`);
const has = async s => (await screen()).includes(s);
let agent = null;
// One Claude Code session for the whole film (stream-json in and out), so there is one connection and one join.
function startAgent() {
  const args = ["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--append-system-prompt", BRIEF,
    "--mcp-config", `${out}/mcp.json`, "--strict-mcp-config", "--allowedTools", "mcp__conn", "--model", "sonnet"];
  const p = spawn("claude", args, { cwd: out, stdio: ["pipe", "pipe", "pipe"] });
  p.stdout.pipe(createWriteStream(`${out}/agent.jsonl`)); p.stderr.pipe(createWriteStream(`${out}/agent-stderr.log`));
  let buf = "", turnDone = null;
  p.stdout.on("data", d => { buf += d; let i; while ((i = buf.indexOf("\n")) >= 0) { const line = buf.slice(0, i); buf = buf.slice(i + 1);
    try { const m = JSON.parse(line); if (m.type === "result") { log({ event: "agent_turn_done", text: String(m.result ?? "") }); turnDone?.(); } } catch {} } });
  p.on("exit", code => { log({ event: "agent_exit", code }); turnDone?.(); });
  p.say = prompt => { log({ event: "agent_prompt", prompt }); const done = new Promise(r => (turnDone = r));
    p.stdin.write(JSON.stringify({ type: "user", message: { role: "user", content: [{ type: "text", text: prompt }] } }) + "\n"); return done; };
  return p;
}

try {
  await writeFile(`${out}/mcp.json`, JSON.stringify({ mcpServers: { conn: { command: `${repo}/target/debug/conn`, args: ["--socket", b.sock, "mcp", "--agent-id", "claude-code", "--tools", "static"] } } }));
  await b.cdp("Page.addScriptToEvaluateOnNewDocument", { source: `localStorage.setItem("ss:fontSize","19");localStorage.setItem("ss:lang","en");localStorage.setItem("ss:followSystem","false");` });
  await b.cdp("Page.navigate", { url: b.url });
  await until("terminal", () => b.js(`!!document.querySelector(".xterm-screen")`));
  await until("local prompt", () => has("you@laptop"));
  await b.js(`document.querySelector(".xterm-helper-textarea")?.focus()`);
  await sleep(1200);

  // ---- record: every changed frame with its own timestamp, nothing dropped or reordered
  let n = 0; const frames = [];
  b.on("Page.screencastFrame", async p => {
    const name = String(++n).padStart(6, "0") + ".jpg";
    frames.push({ name, ts: p.metadata.timestamp });
    await writeFile(`${out}/frames/${name}`, Buffer.from(p.data, "base64"));
    b.cdp("Page.screencastFrameAck", { sessionId: p.sessionId });
  });
  await b.cdp("Page.startScreencast", { format: "jpeg", quality: 95, maxWidth: VIEW.width * 2, maxHeight: VIEW.height * 2, everyNthFrame: 1 });
  log({ event: "record_start", wall: Date.now() / 1000 });
  await sleep(900);

  // ---- beat 1: you log in
  log({ event: "beat", name: "login" });
  await h.type("ssh staging"); await sleep(250); await h.enter();
  await until("password prompt", () => has("password:"));
  await sleep(700);
  const pw = (await readFile(`${F}/remote/password`, "utf8")).trim();
  await h.type(pw, { delay: 38, jitter: 14, label: "(hidden password)" }); await h.enter();
  await until("remote prompt", () => has("deploy@staging"));
  log({ event: "logged_in", secretOnScreen: await has("SYNTHETIC") });
  await sleep(1400);

  // ---- beat 2..: the agent joins and works; the human reviews every command
  agent = startAgent();
  let denied = false, approvals = 0;
  const review = async () => {
    if (await b.js(`!!document.querySelector('[role=alertdialog]')`)) {
      log({ event: "beat", name: "join_card" }); await sleep(3400);   // long enough for a viewer to read the card
      await h.click(`document.querySelector('[role=alertdialog] .btn.primary')`, "Allow"); await sleep(500); return;
    }
    const cmd = await b.js(`document.querySelector('.hold code.raw')?.innerText ?? null`);
    if (cmd == null) return;
    const intent = await b.js(`document.querySelector('.hold .intent')?.innerText ?? ""`);
    const label = await b.js(`document.querySelector('.hold .label')?.innerText ?? ""`);
    log({ event: "card", cmd, intent, label });
    if (/\brm\b/.test(cmd)) {
      log({ event: "beat", name: "deny" }); await sleep(2300);
      await h.click(`document.querySelector('.hold .btn.danger')`, "Deny"); denied = true;
      await until("card gone", async () => !(await b.js(`!!document.querySelector('.hold')`)));
      await sleep(900);
      log({ event: "beat", name: "takeover" });
      await b.js(`document.querySelector(".xterm-helper-textarea")?.focus()`);
      await h.type("./api.sh unlock"); await sleep(250); await h.enter();
      await until("unlocked", () => has("stale lock cleared"));
    } else {
      await sleep(approvals === 0 ? 2000 : 1300);
      await h.click(`document.querySelector('.hold .btn.ok')`, "Approve"); approvals++;
      await until("card gone", async () => !(await b.js(`!!document.querySelector('.hold')`)));
    }
    await sleep(400);
  };
  const drive = async turn => { let alive = true; turn.then(() => (alive = false)); const end = Date.now() + 240000; while (alive && Date.now() < end) { await review(); await sleep(150); } if (alive) { agent.kill(); throw new Error("agent turn timed out"); } };
  await drive(agent.say(PROMPT_1));
  if (!denied) throw new Error("the agent never proposed the delete; retake");
  await sleep(1200);

  // ---- beat: you say what changed, it continues
  log({ event: "beat", name: "continue" });
  await drive(agent.say(PROMPT_2));
  agent.stdin.end();
  const healthy = await has('"status": "ok"');
  log({ event: "result", healthy, controller: await b.js(`document.querySelector(".app")?.classList.contains("agent") ? "agent" : "human"`) });
  await sleep(2500);
  await b.cdp("Page.stopScreencast");
  log({ event: "record_stop", frames: n, browserErrors: b.errors });
  await writeFile(`${out}/frames.json`, JSON.stringify(frames));
  await writeFile(`${out}/events.json`, JSON.stringify(events, null, 1));
  await writeFile(`${out}/screen-final.txt`, await screen());
  if (!healthy) throw new Error("service is not healthy at the end; retake");
} finally {
  try { agent?.kill(); } catch {}
  await b.stop();
}
