# OAuth 2.0 implementation checkpoint

This development branch extends the v0.1.0 preview. The full Apifox/Postman capability goal remains unfinished.

## Available behavior

OAuth2 5.0 owns authorization code, implicit, client credentials, password and device grants, SHA256 PKCE, CSRF state, request encoding, token parsing, refresh, introspection and revocation. Checked Rust transport pins destinations, rejects redirects and bounds responses/timeouts.

Private tokens are scoped to account/workspace and authorization profile. The manager acquires, imports, selects, renames, reveals/copies, refreshes, inspects, revokes and deletes tokens. Credentials are excluded from ordinary workspace exports and synchronization. Provider inspection exposes only active status, scopes and expiry. Revoked/expired tokens cannot execute.

Browser grants support manual URL completion, hosted automatic callbacks and a native IPv4 loopback listener. Register the exact callback URI with the provider. Hosted URI: your server origin plus `/api/oauth2/callback`. Native default URI: `http://127.0.0.1:49152/api/oauth2/callback`; the port must be available and explicitly configured. The native listener is created only for local mode and stops on completion, cancellation, expiry or workspace deletion. Browser callbacks never reveal private credentials, verifier, account identity or token IDs. Implicit fragments are removed from browser history before posting to the state-bound broker.

In multi-instance hosting, authorization creation, callback and status polling must reach the same instance (for example with sticky routing), because short-lived browser flow state stays in memory. Refresh leases are persisted using revision CAS and work across instances sharing the database.

## Verification and remaining work

At this checkpoint 302 Rust tests pass (20 external tests ignored), 173 frontend tests pass, strict Clippy, formatting and frontend/embedded-server builds pass. Fixtures exercise all grants, device authorization_pending/slow_down/expiry, logout during device startup, workspace cancellation, rotating refresh, multi-instance lease contention/recovery, invalid refresh retry, TLS revocation, imports, secret-safe exports, single-use callbacks and real loopback PKCE exchange. Focused independent backend/UI reviews were completed.

Agent-browser verified hosted browser authorization, automatic token selection, provider inspection and an actual authenticated HTTP request. English/light and Chinese/dark screenshots are included, with a separate Chinese390px overflow check. These web and loopback fixtures do not establish native Tauri opener/tray or signed installer behavior on every OS.

Remaining OAuth work includes provider-specific source import/export mappings, broader cross-database OAuth race coverage and native-platform OAuth/browser QA. Full SDK/server generation, other authentication providers and the broader protocol/testing/Mock/docs/collaboration/integration capability matrix remain separate work. See [the specification](specs/oauth2.md).

## Grant projection and token-profile compatibility

Execution now resolves only fields used by the selected grant and enabled parameter/header rows. Original drafts remain in saved requests; unused endpoint/password/username references do not block unrelated grants. Explicit introspection/revocation resolves that operation's endpoint/client authentication/custom headers rather than every dormant management field. Token binding ignores irrelevant PKCE and implicit-client-auth settings while retaining meaningful grant/environment identity.

This development change introduces token profile version2. Records created by earlier development builds without a version remain private/listable/revealable/deletable, but must be acquired or explicitly imported again before network execution/refresh/introspection/revocation. Existing workspaces and original configuration are preserved. Unknown/old hashes are not accepted as a migration shortcut. The public v0.1.0 preview did not include OAuth2 tokens.

Necessary scoped verification:20core/server OAuth fixtures pass, including inactive unresolved fields, implicit management header/client-auth restoration and legacy-token rejection without network use followed by explicit reimport.12TokenManager tests and TypeScript checks pass. Previously passed unrelated full suites are not rerun for this change; the branch CI remains the complete regression gate.
