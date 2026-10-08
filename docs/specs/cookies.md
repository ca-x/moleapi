# Private HTTP cookie sessions

The finite HTTP/SOAP/GraphQL request pipeline accepts an optional private cookie jar. `cookie_store 0.22.1` owns parsing, host/domain/path matching and expiry. The static `psl 2.1.240` list rejects public suffix Domain attributes. Cookie prefixes and secure-cookie overwrite protection are checked before SDK insertion. Explicit Domain attributes that name a public suffix are rejected even when identical to the source host; host-only cookies remain supported.

## Scope and lifecycle

- Account + workspace + selected environment (including a separate no-environment scope); collection runner uses its selected environment.
- Disabled by default. Enable through request Settings → Manage cookies. Receiving and sending are controlled together.
- Private in-memory sessions. Logout, successful workspace deletion or process restart clears them. Disabling retains values for explicit reuse; Clear deletes them.
- No workspace schema, synchronization, version snapshots or backup exports contain jar values. This slice does **not** implement durable private cookies or browser Interceptor synchronization.
- Management admission rechecks the authenticated session under its owner gate. Workspace deletion shares the existing workspace gate. Execution only looks up existing jars, never recreates one after logout/deletion.
- A generation fence prevents a request already in flight from restoring cookies after Clear, disable, deletion or manual edits. Existing requests can complete; response cookie values still participate in history privacy.

## Transport behavior

Cookie values are attached to the actual per-hop request **before** AWS/Hawk signing. An explicit Cookie header takes precedence. Redirects and Digest challenge responses contribute Set-Cookie changes before the next hop. URL matching is repeated at every hop, including cross-origin redirects; cookies follow RFC domain/path/secure rules (ports are not cookie scope). Manual headers retain existing cross-origin stripping behavior.

Session cookies, expiry/Max-Age deletion, host-only/domain cookies, repeated names at different paths, Secure, HttpOnly and SameSite metadata are supported. More specific paths are sent first. SameSite is metadata, not a browser navigation policy: this API client has no top-level browser site context. Invalid/rejected/over-capacity response cookies are ignored without failing the HTTP response; invalid manual additions return an explicit error.

Set-Cookie headers remain redacted in responses. Accepted or rejected parsed response values and actual outgoing jar values enter the existing private-value budget and history redaction pipeline. GET management responses use Cache-Control: no-store; masked list is default and revealing values requires an explicit request/UI action.

Limits: 8 KiB per cookie, 512 unexpired cookies and 256 KiB aggregate per jar; 1024 private scopes per process. SDK cleanup removes expired entries. Resource-limit errors for manual insertion are explicit. Expiry order for equal-path cookies has no SDK creation-time guarantee.

## API

`/api/workspaces/{id}/cookies`, optional `environment_id` query; omitted uses the saved active environment:

- GET: `{enabled,cookies}`, values masked; explicit `reveal=true` returns values.
- PATCH: `{enabled,clear?:boolean}`.
- POST: `{url,cookie}`; one complete Set-Cookie value. Reinsert the same domain/path/name to edit.
- DELETE: `{domain,path,name}`.

All operations require workspace ownership; unknown environment IDs reject. The same routes work over native Tauri IPC. Unsaved environment definitions must first be saved.

## Evidence and remaining coverage

Three core tests verify SDK rules, real redirect/HTTP requests and a real in-flight Clear race. Two server tests verify management/actual execution, environment and account isolation, logout cleanup, history privacy and no-store. Two React tests cover masking/explicit reveal/clear and stale close/reopen responses. Translation catalog validation, TypeScript production build, strict core/server Clippy and package fmt passed.

Agent-browser exercised the actual embedded server UI: enable, add, masked list, reveal and delete, including a 390px viewport. Native graphical installers are still part of the final GitHub Actions release gate, not claimed tested by HTTP browser evidence.

Separate incomplete capabilities: durable private cookie storage, script pm.cookies/cookie-jar compatibility, dedicated response-cookie extraction, browser capture/sync, per-request disable overrides, and live protocol cookie adapters. This checkpoint does not mark the whole network matrix or full product parity complete.
