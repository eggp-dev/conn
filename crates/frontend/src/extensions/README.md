# Native extension host (API 1)

The host registers declarative terminal themes. It does not discover or execute
third-party JavaScript, native libraries, commands or webviews, and it has no network,
credential or PTY API. An extension manifest is a contribution contract, not a sandbox.

## Responsibilities

- `manifest.rs`: exact version/kind/capability validation and the contribution registry.
- `mod.rs`: theme selection, theme install and `extensions.json` persistence.
- The parent native bridge (`extension_commands.rs`) owns window authorization and exposes
  `extensions_status`, `extension_configure` and `extension_install_theme` to the owner UI.

## Plug-in point

`Kind` and `Capability` already name executable contributions (`provider`, `completion`;
`visible_frame`, `model_request`, `proposal`), so stored manifests and the wire format
stay stable. `Manifest::validate` admits such a kind only for an exact reviewed built-in
ID and capability set. None ships, so every non-theme manifest is rejected with "Only
reviewed built-in executable extensions are supported". A future reviewed integration is
added there and in `Registry::builtin`; it must keep these rules:

- Never accept terminal text, frames or credentials from webview invoke arguments. Build
  context from the core session under its lock.
- Route accepted text through the common human-input path and never append Enter.
- Revalidate sharing and the current frame before delivering or accepting a result.

The native OpenAI command-suggestion provider and its OS credential-store adapter that
first used this contract were removed. Settings files they wrote still load; the extra
fields are dropped on the next save.

Themes contribute only six-digit terminal color tokens. They cannot style authority,
approval, sharing, or plugin-management controls. Dynamic themes must have unique IDs.

## Validation

Run `cargo test -p conn-frontend extensions:: --locked`.
