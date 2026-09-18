# Extensions

[한국어](extensions.ko.md) · [Trust model](security.md#native-extensions-and-model-data)

**Available in v0.7.0 preview.** Terminal themes, a native OpenAI model connection
and opt-in command suggestions use the shared-surface contract.
There is no marketplace or arbitrary third-party executable plugin installation.

## Change a terminal theme

Open **Settings → Extensions**, choose Midnight, Paper, Solar, Nord or Mono, or use
**Install theme…** to import a Conn theme JSON file. Themes change supported terminal
colors. They cannot hide approval controls, change sharing, add scripts, run commands
or replace the app's navigation. Existing appearance settings remain separate.

An imported file has an API version and a single declarative contribution:

```json
{
  "apiVersion": 1,
  "id": "example.theme.indigo",
  "name": "Indigo",
  "kind": "theme",
  "capabilities": ["theme"],
  "theme": {
    "id": "example.theme.indigo",
    "name": "Indigo",
    "background": "#101322",
    "foreground": "#e4e7f2",
    "cursor": "#e4e7f2",
    "selection": "#384166",
    "ansi": [
      "#12151c", "#f87171", "#34d399", "#fbbf24",
      "#60a5fa", "#a78bfa", "#22d3ee", "#d7dae0",
      "#64748b", "#fca5a5", "#6ee7b7", "#fde68a",
      "#93c5fd", "#c4b5fd", "#67e8f9", "#ffffff"
    ]
  }
}
```

`ansi` contains the usual eight colors followed by their eight bright variants. Colors
must be six-digit hex strings. Unknown fields, unsupported API versions, duplicate IDs,
extra capabilities and executable entrypoints are rejected. Up to 32 custom themes are
stored in `extensions.json`. This first host does not edit shell startup files or install
prompt programs. A prompt integration that executes shell code needs a different contract.

## Connect your OpenAI account

1. In **Settings → Extensions**, enter a model ID available to your API account and save it.
2. Enter your API key and choose **Save key**. The input clears after submission; there
   is no API to retrieve the stored value into the webview.
3. Enable **OpenAI**, then **Command suggestions**. This enables requests after a typing
   pause at confirmed local shell prompts.
4. Return to a shared, visible terminal. Place the cursor where you want a command suffix.
5. Pause typing at a supported local shell prompt, or choose **Suggest a command** in
   the command palette. Review the result and choose **Insert**. Press Enter separately
   in the terminal if you want to execute it.

The API account is separate from an external agent client's login/subscription. Conn
does not import Codex/ChatGPT authentication or read `OPENAI_API_KEY` from shell environment.
Use your own API key. Model availability and charges depend on that API account.

When enabled, automatic requests wait for an 800 ms typing pause and require the native
host to confirm an idle local shell prompt under human control. The trigger contains no
typed text; request context still comes only from the presented frame. Authentication,
editors, running commands, external-origin and unconfirmed shells do not automatically
trigger suggestions. A manual request remains available in unconfirmed environments;
the user must ensure this is a command-entry context. Disable **Command suggestions**
to stop both automatic and manual requests.

The response is an append-only suggestion, not a full replacement or auto-execution.
A frame change, human input, lost shared view, configuration change or cancellation
invalidates it. Request a new suggestion after correcting the terminal. A stale result
cannot be applied to another session, surface or frame.

## What is sent and stored

Only the currently authorized visible text is sent to the fixed OpenAI HTTPS endpoint.
No hidden input buffer, files, history, environment, startup arguments or previous model
conversation is added. Content already visible on a shared screen can include secrets;
this feature is not a secret detector. Private sessions cannot request suggestions.

The key is stored using macOS Keychain, Windows Credential Manager or Linux Secret Service.
A locked/missing system service reports an error rather than saving plaintext elsewhere.
Only the native backend reads the key, to check credential status or make the reviewed
provider request. It is not stored in extension JSON, shell environment, timeline or proposal data. Removing the key
also cancels pending proposals. A key removal error means the OS store did not confirm deletion.

Requests have a 5-second connection timeout and 20-second overall timeout, a 16 KiB input
limit, 64 KiB response limit, two in-flight jobs and twenty starts per minute per app.
Suggestions are at most 1 KiB and expire after 30 seconds. No automatic retries or tools
are enabled; redirects and environment proxy overrides are disabled. `store: false` is
sent, but it does not guarantee zero provider retention. Cancelling locally cannot
promise that the provider stops work or billing already in progress.

## Extension architecture

| Contribution | Capabilities | Execution |
| --- | --- | --- |
| Terminal theme | `theme` | Validated declarative colors |
| Built-in OpenAI provider | `model_request` | Reviewed native HTTPS adapter |
| Built-in command suggestions | `visible_frame`, `model_request`, `proposal` | Host-authorized frame and human acceptance |

The host owns registration, settings, authority, cancellation, result expiry and common
UI. A contribution does not receive a PTY or an owner bridge. Model providers and
suggestion logic are separate adapters so another reviewed model provider can reuse
the same proposal lifecycle. The manifest is not a security sandbox: arbitrary native
or JavaScript code, unrestricted network, file access, webviews and dependencies between
plugins are deliberately outside this first host.

The native implementation and bridge contract are in
[`crates/frontend/src/extensions`](../crates/frontend/src/extensions/README.md).
The existing repository `plugin/` directory distributes **external client MCP/skill
integration**, not the new in-app extension runtime.

## Verification boundary

Unit tests use fake providers and an in-memory secret-store adapter. They cover private
context denial, cancellation during a blocked key read, stale/replayed acceptance,
configuration changes, input/output limits, schema rejection and secret exclusion from
serialized settings. They do not perform a paid request, change your system credential
store or prove native macOS/Windows unlock and rendering behavior.

Before shipping, test a user-configured model and native credential store on each target
OS. See [OpenAI text generation](https://developers.openai.com/api/docs/guides/text) for
the request format and [trust boundaries](security.md) for shared terminal data.
