# Extensions

[한국어](extensions.ko.md) · [Trust model](security.md#native-extensions)

Extensions currently contribute terminal themes. The registry behind them is the
plug-in point for future reviewed integrations; no executable extension ships today.
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

## Extension architecture

| Contribution | Capabilities | Execution |
| --- | --- | --- |
| Terminal theme | `theme` | Validated declarative colors |

The host owns registration, settings and common UI. A contribution does not receive a
PTY or an owner bridge. The manifest format also names `provider` and `completion`
kinds and the `visible_frame`, `model_request` and `proposal` capabilities, but a
manifest of those kinds is accepted only for an exact reviewed built-in ID, and none
ships: such a manifest is rejected. The native OpenAI command-suggestion preview from
v0.7.0 has been removed, together with its API-key storage. The manifest is not a
security sandbox: arbitrary native or JavaScript code, unrestricted network, file
access, webviews and dependencies between plugins are deliberately outside this host.

If you saved an API key with that preview, Conn no longer reads or deletes it. Remove
the `dev.eggp.conn.model-provider` entry yourself in macOS Keychain, Windows Credential
Manager or your Linux Secret Service keyring.

The native implementation and bridge contract are in
[`crates/frontend/src/extensions`](../crates/frontend/src/extensions/README.md).
The existing repository `plugin/` directory distributes **external client MCP/skill
integration**, not the new in-app extension runtime.

## Verification boundary

Unit tests cover schema rejection, executable-kind and excess-capability rejection,
theme persistence, and loading an `extensions.json` written by the removed provider.
They do not prove native macOS/Windows rendering behavior. See
[trust boundaries](security.md) for shared terminal data.
