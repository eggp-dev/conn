# Conn PRD

> Status: v0.3 (product name Conn settled)
> v0.1 (a transparent proxy under Terminal.app) is extended into an **engine + frontend** structure. Every v0.1 behaviour is preserved.

---

## 0. One line

> **Conn — one shell, one hand on it.** As one says "You have the conn" when handing over the helm, exactly one side holds control, every handover is recorded, and the human can always take it back. The frontend decides what an agent may do, how fast, and how it is shown.

Conn is an **engine**, like the shell backend behind Warp. It owns the PTY, write authority (the lease), policy, approvals, audit and the headless screen model. Drawing the window is the frontend's job, and we build that frontend ourselves (Tauri). An existing terminal (Terminal.app) remains one frontend among several.

---

## 1. Why v0.2

v0.1 ran under Terminal.app on the principle of "never render". That structure could not:

- show **across the whole window** that an agent is in control
- put **time thresholds** on agent control (typing interval, a grace period before execution)
- let the frontend **restrict the agent's affordances** by situation
- let the user take control back by means other than the keyboard (a button, a gesture)

v0.2 keeps the core as it is and opens an **engine API** so a frontend can do these four things.

---

## 2. Product principles (kept from v0.1)

1. **Human input has structural priority.** Whichever path human input arrives on, the agent's lease is revoked before that byte reaches the PTY.
2. **The human owns the policy.** What needs approval is declared in a file. Narrower is better.
3. **Projection is limited to what the human sees.** No scrollback.
4. **Session lifetime is never extended.** No detach/reattach.
5. **A guard against mistakes, not against attackers.**

Added in v0.2:

6. **The engine does not draw. The frontend draws.** The core emits only bytes and events. The one exception is the approval prompt in terminal mode, and it is optional.
7. **A frontend can only narrow the engine's judgement.** It can shrink affordances with a mask and slow things down with pacing, but it cannot overturn a policy verdict (deny) or weaken the priority of human input.

---

## 3. Axes of flexibility

| Axis | Options | Notes |
|---|---|---|
| Deployment | linked as a library (`conn-core::Engine`) / separate process (`conn serve`) | Tauri links; frontends in other languages use the socket |
| Human input path | process stdin (terminal mode) / `Engine::write_input` / socket `input` | all three go through the same `human_input` |
| Output path | stdout / `EngineConfig.output` / subscriber streaming (`Output` events) | several frontends can watch at once |
| Approval UI | engine draws in the terminal (`render_prompt`) / frontend draws from `ApprovalRequested` | |
| Agent affordances | state table ∩ frontend mask | the mask only shrinks |
| Time | `Pacing { minWriteIntervalMs, enterGraceMs, leaseTtlSecs, approvalTtlSecs }` | changeable at runtime |
| Agent connection | MCP stdio adapter → UDS | identical whatever the frontend |

---

## 4. Architecture

```
                  ┌────────────────────────────────────────────┐
   Tauri app      │  conn-core                                 │
  (xterm.js) ──▶  │   Engine ─ Session ─ Authority / Policy /   │ ◀── UDS ── conn mcp ── Copilot / Claude / Codex
   write_input    │            Approval / Audit / Screen / Input│
   ◀── events     │   ipc::serve_in_background                  │ ◀── UDS ── conn status/take/approve
                  └───────────────┬────────────────────────────┘
                                  │ PTY
                                  ▼
                                 zsh ─ ssh ─ …

   Terminal.app ── stdin/stdout ── conn (proxy mode, same Engine)
   any frontend ── UDS (hello kind=frontend, streamOutput) ── conn serve
```

---

## 5. Frontend contract

### 5.1 Events (a frontend receives all of them)

`control_granted` `control_revoked{reason}` `agent_input{len}` `approval_requested{request}` `approval_resolved` `exec_scheduled{execId,cmd,graceMs}` `exec_cancelled{reason}` `agent_exec{cmd,policy}` `screen_changed{revision}` `process_exited` `pacing_changed` `affordance_mask_changed` `output{data}` (when streaming was requested)

### 5.2 What a frontend calls

`input(bytes)` `resize` `take` `approve(id, grant|deny|allow_session)` `cancel_exec(id)` `set_pacing(patch)` `set_affordances(allow|null)` `status`

### 5.3 Handoff UX (v0.3)

A handoff must happen without the human making a separate decision to let go.

| Element | Behaviour | Core |
|---|---|---|
| Mode segment | Observe (look only) / Co-pilot (propose; the human runs with ⏎) / Autopilot (runs itself) | `set_mode` |
| Co-pilot proposal | agent input does not reach the shell; it appears as a ghost bar under the cursor line. ⏎ commits, Esc rejects, any other key rejects and types. A `confirm` verdict counts as approved by the commit; `deny` still blocks | `accept_proposal` / `reject_proposal` |
| Reasoned request | the `reason` from `request_control(reason)` is shown in a banner. With "ask before granting" on: [Allow] [As Co-pilot] [Deny] | `set_control_gate` / `decide_control` |
| Hand-back chip | right after a takeover: "⌘⏎ give `<agent>` the conn back · it was doing: …". Disappears after 60 s | `hand_back` (agent receives `control_handed_back`) |
| Inline approval | a yellow hold bar on the cursor line instead of a modal. Label first; `a/d/A` unchanged | unchanged |
| Session-allow chips | labels allowed with `[A]` stay as chips at the top; click one to ask again | `revoke_session_allow` |
| Grace handshake | during the countdown ⏎ = run now (co-signed), Esc = cancel | `execute_now` / `cancel_exec` |

### 5.3b Attention and tabs (v0.3)

**The snapshot an agent sees is what the human is looking at.** With several tabs, exactly one — the one the human is watching — is attended; agents on the others can neither see nor write. This is an input to the affordance table.

| Situation | Agent affordances |
|---|---|
| attended | the usual table |
| unattended, not entrusted | `request_attention`, `switch_tab` (+ `check_approval` for an approval it already has) |
| unattended, entrusted | the usual table, but the mode is capped by the policy's `unattended` (Co-pilot by default) |
| attended (host supports tabs) | the usual table + `open_tab`, `switch_tab` |

Leaving a tab is a state transition. On the way out the app asks "entrust and go?"; the default is pause. Agents cannot change the human's tab; they can only knock. A connection is bound to the session it first attached to and never drifts to another tab behind the human.

**Agents open tabs too — as an affordance.** `open_tab(reason)` spawns a new shell and moves the agent's connection there, but that tab **starts unattended**. On the human's screen the tab strip shows the agent's name tag and reason and the tab knocks. Until the human goes there (⌘⌥→) or entrusts it, the agent can neither see nor write in it. This is how tab creation coexists with the rule "an agent sees what the human sees". `switch_tab(tab)` moves only the agent's own connection and never the human's view. Both can be switched off in the tool mask, and hosts without a tab opener (headless `conn serve`) do not offer them.

### 5.3c Control surfaces (v0.3)

- **Island → control centre.** Clicking the island grows a popover: mode, gate, grace, and each connected agent's current affordances. Eight-tenths of settings use ends here.
- **⌘K knows the state.** What needs an answer now comes first; current values sit on the right; commands take arguments (`grace 3s`, `tab 2`, `theme paper`); bilingual aliases; most recent first.
- **⌘, states its scope.** This tab / all tabs, defaults for new tabs. Tools as groups and presets, policy as a rule list plus tester, a diagnostics tab. The sheet pushes the terminal aside.
- **i18n.** English by default, Korean available. Every UI string goes through the dictionary.

### 5.4 UX requirements (Tauri reference frontend)

- The moment an agent takes control, show a ripple across the window, a rotating frame and a banner at the top ("`<agent>` is in control · press any key to take over"). They disappear on revocation.
- While the agent types, a typing indicator blinks in the banner.
- With `enterGraceMs > 0`, show a countdown bar right before execution; any key or Cancel cancels it.
- Approval requests put the label at the very top. `a` / `d` / `A` keep working.
- Pacing sliders and the read-only toggle are adjustable immediately.
- **In every state** a single keystroke returns control to the human. While a modal is open, keys answer the modal and never reach the shell.

---

## 6. Pacing semantics

| Item | Behaviour | What the agent sees |
|---|---|---|
| `minWriteIntervalMs` | a write within this interval of the previous one is refused (`rate_limited`) | invisible: the socket layer waits and retries on the agent's behalf |
| `enterGraceMs` | an ENTER judged allow is sent after this delay. Human input, `cancel_exec`, interrupt or loss of the lease in between cancels it | the `send_key` response arrives after the grace as `executed` or `cancelled{reason}` |
| `leaseTtlSecs` | lease expiry; every write renews it | `control_revoked{expired}` |
| `approvalTtlSecs` | a pending approval expires → denied | `check_approval` = `expired` |

---

## 7. Affordance mask

`set_affordances(allow)` sets the largest set an agent may hold. The final set is `state table ∩ allow`. `null` means no restriction.

Example: `["snapshot"]` = observe only. An agent that already holds a lease loses its write tools at once (`tools_changed`), and an execution in its grace window is cancelled.

---

## 8. Scope

### v0.2 Required

1. `conn-core` crate: `Engine`, event subscription, output streaming, headless approvals, pacing, mask
2. `conn serve` (headless) + the socket frontend protocol
3. Tauri reference frontend: xterm.js, agent-control effects, approval UI, grace countdown, pacing/mask controls
4. All of v0.1 terminal mode, MCP and CLI preserved
5. Tests: core unit tests + headless e2e

### Non-goals (unchanged)

A rendering engine of our own, multiplexing, session persistence, scrollback management, semantic command analysis, secret redaction, concurrent writes by several agents, Windows/Linux

---

## 9. Open questions

- Q1 (v0.1) Whether the built-in shell tools of the Copilot/Claude harnesses can be disabled — needs checking per harness. Claude Code can be limited to MCP tools with `--allowedTools`.
- Q4 Whether a Tauri app is acceptable instead of Terminal.app in access-controlled (PAM) environments. The session-lifetime principle (4) is unchanged, but product sign-off is needed.
- Q5 Human input mixed into the line during a grace window: today it only cancels and leaves the line as is. Whether an automatic Ctrl-U is better is to be judged from real use.
