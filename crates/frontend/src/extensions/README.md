# Native extension host (API 1)

The first host supports declarative terminal themes and reviewed, bundled native
providers. It does not discover or execute third-party JavaScript, native libraries,
commands or webviews. An extension manifest is a contribution contract, not a sandbox.

## Responsibilities

- `manifest.rs`: exact version/kind/capability validation and contribution registry.
- `secrets.rs`: a narrow OS credential adapter (macOS Keychain, Windows Credential
  Manager, Linux Secret Service). A missing or locked OS store fails safely; there is
  no environment variable, configuration-file or memory-only fallback.
- `provider.rs`: the reviewed OpenAI Responses API adapter. The native host supplies
  the key; UI manifests and completion contributions never receive it.
- `mod.rs`: settings, bounded request lifecycle, cancellation and one-shot proposals.
- The parent native bridge owns window/session authorization and authoritative frame
  acquisition. It is also solely responsible for routing accepted text through the
  common input path. This module has no PTY, filesystem context or command execution API.

## Host integration

`VisibleContext` deliberately does not implement `Deserialize`. Construct it from an
observed shared surface under the session lock, then call `start_completion` while
holding that lock. Registration does not access the keychain or network. Subsequent
input, output, sharing, surface ownership and visibility changes must call `cancel`
under the same state boundary. A manual trusted user action may set `explicit`; an
automatic request needs `prompt_ready` from confirmed shell integration. Both need
`shared` and a current frame. Never accept arbitrary `lines` from invoke arguments.

Jobs expose `pending`, `ready`, `failed` and `cancelled`. Before returning a poll result
or accepting text, the bridge must revalidate sharing and the current frame. Acceptance
calls `take_proposal` with the exact session/surface/generation/frame token and routes
its single-line suffix through human-approved common input. It must never append Enter.
A stale acceptance consumes the proposal; it cannot be retried against a different frame.

The UI supports opt-in requests after an 800 ms typing pause at a native-confirmed
idle local prompt, plus explicit invocation in unconfirmed environments. The trigger
contains no typed text. There is no auto execution. Configuration and key changes
cancel all proposals.
Deleting a key cancels pending jobs even if the OS store later refuses deletion.

## Limits and disclosure

- Disabled by default; the user must select a model, save a key and enable suggestions.
- Current visible text only, at most 16 KiB; no prior requests, hidden input, shell
  history, environment, files or secret metadata.
- Two concurrent jobs and twenty starts per minute across the application instance.
- HTTP connect timeout 5 seconds, total request timeout 20 seconds, result lifetime
  30 seconds, 64 KiB response limit, 1 KiB single-line suggestion limit.
- Fixed HTTPS `api.openai.com/v1/responses`; redirects and environment proxies disabled.
- No tools, automatic retries or response storage (`store: false`). This flag does not
  claim zero retention by the provider. Network cancellation also does not guarantee
  cancellation of processing or billing already started by the provider.
- OS credential access can wait for the system unlock UI. Completion credential reads
  run in their worker, outside the terminal lock; cancellation suppresses any later
  request/result, and in-flight limits remain. Settings status also queries key presence
  outside the terminal lock and never returns its value.
- Provider error bodies are never exposed. The native adapter returns fixed messages.
- Any content visible in a shared terminal may be transmitted when suggestions are enabled.
  This is not a secret detector, nor protection against other tools on the same OS account.

Themes contribute only six-digit terminal color tokens. They cannot style authority,
approval, sharing, or plugin-management controls. Dynamic themes must have unique IDs;
executable kinds are restricted to exact reviewed built-in IDs and capabilities.

## Validation

Run `cargo test -p conn-frontend extensions:: --locked`. Tests use in-memory stores and
fake providers without a real key, network call or native credential-store mutation.
Native keychain prompts, live OpenAI responses and provider billing require a separate
user-configured smoke test on each supported OS.

API format checked against [OpenAI text generation](https://developers.openai.com/api/docs/guides/text)
and [Responses create](https://developers.openai.com/api/reference/resources/responses/methods/create).
Native store features use [keyring 3.6](https://docs.rs/keyring/3.6.3/keyring/).
