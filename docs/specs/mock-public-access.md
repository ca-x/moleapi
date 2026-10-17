# Explicit public Mock access

Authority: `mock-011` in `docs/FEATURE-MATRIX.md`, the owner-only Mock routes, `docs/specs/mock-request-matching.md`, `docs/specs/mock-scripts.md`, `docs/specs/mock-call-logs.md`, and `docs/specs/mock-response-delay.md`.

## Objective and scope

A workspace owner can deliberately share one saved HTTP request as a public Mock endpoint without sharing the workspace, account credential, other requests, or Mock call log. Private Mock routes remain owner-only. The first delivery is for the hosted server; a desktop offline workspace needs a separate explicit listener before its public-access target is complete. A public share is a revocable capability, not an account or a complete workspace backup.

## Hosted contract

- Public access defaults off. An owner creates up to 20 active shares per workspace, each bound to one saved HTTP request. Creation requires an explicit expiry of 1–90 days and an `allow_dynamic` choice that defaults false. Dynamic Mock scripts can read workspace datasets; the UI explains this before enabling them for public callers. A share cannot target a non-HTTP request, a missing request, or an unsaved draft.
- Create returns a cryptographically random 256-bit URL-safe token once. Durable storage holds only its SHA-256 digest, owner/workspace/request binding, expiry, dynamic permission and metadata. List returns metadata with no token. Revoke deletes the share. Rotation creates a new token and revokes the old one in one transaction. Workspace deletion removes every share in the same transaction. Default and full MoleAPI backups/sync omit shares, so restore starts private.
- A share also binds to a digest of the exact saved request at creation. Editing or replacing the request makes the old URL stale and returns 404; the owner rotates the share after saving to explicitly authorize the new request content. List shows that stale state without exposing the digest. Deleting and recreating a request with the same ID cannot silently inherit an old public permission.
- Public routes use a separate unguarded path: `GET /api/public/mock/{token}/{example_id}` and any HTTP method on `/api/public/mocks/{token}/{*path}`. The token fixes the request scope. Routing still applies the existing method, path, query, header, body, priority, script and response limits; no other request can match. The direct route selects only an example of that request. A dynamic example is unavailable unless `allow_dynamic=true` was deliberately selected. Private routes retain their current authentication behavior.
- Missing, expired, revoked, wrong-scope or malformed tokens yield the same generic 404. Public responses and errors never include token, owner, workspace source, private script/dataset diagnostics or another request's data. The token is removed from the synthetic incoming URI before a Mock script sees `pm.request.url`; incoming Authorization, Cookie and sensitive query/header values retain the existing screening. Public call logs remain owner-only and store no token, raw path, caller IP or request/response body.
- Every public call checks the share and current workspace before matching, then rechecks the share after script execution and configured delay before serving content. Revocation, expiry, request deletion or workspace deletion during a wait fails closed. A per-process public admission cap prevents unlimited concurrent matching; the existing script worker and delay quotas still apply. Cross-instance decisions use the database, not a process-local token cache. Storage errors fail closed.
- Public response adds `Cache-Control: no-store` and `Referrer-Policy: no-referrer`. Cross-origin browser calls use `Access-Control-Allow-Origin: *` without credential support; preflight handles allowed request headers/methods without exposing token validity. Deployment guidance requires HTTPS outside loopback because the URL token is a bearer capability. No server or client log persists the raw token.

## Product and desktop behavior

The bilingual request editor shows private/public status and owner-only share management. Creation displays the public URL once with a copy action and the expiry/dynamic permission; reload shows only a masked share ID and revocation/rotation controls. At 390px the modal fits without horizontal page overflow. Offline native workspaces keep Mock private until an explicitly started, bounded local listener is implemented; the editor must not claim that an offline URL is publicly reachable.

## Acceptance and test commands

- `cargo test --locked -p moleapi-server --test mock_public` covers two owners, two requests, direct/routed public calls, token entropy/digest storage, expiry, revoke/rotate, deletion cascade, cross-instance revocation during delay, dynamic permission, URI/token privacy and admission saturation. The real database suite runs the same owner/share lifecycle against SQLite, PostgreSQL and MySQL.
- `cargo test --locked -p moleapi-formats` proves default/full backups do not contain or restore share capabilities. `npm --prefix web test` covers creation, one-time token display, reload, revoke and language labels.
- `cargo fmt -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `npm --prefix web run build`, `cargo build --locked -p moleapi-server`, a real hosted browser and HTTP fixture at 390px, Application CI, Docker and native package checks establish the current-head gates.

## Structure and coding boundaries

Use the existing `documents` table with kind `mock-share` (within its 16-character limit), a digest-derived primary key for indexed lookup and owner/workspace binding in the payload. Use SeaORM transactions and the existing workspace row lock pattern for create, revoke, rotate and deletion. Keep public route admission in `crates/server/src/mock_public.rs`, share common selection/response code in `mock.rs`, and put bilingual UI in the request feature. For example, an invalid capability is always returned through `ApiError::not_found()`; do not format the token into diagnostics.

Always validate owner, expiry, scope, response budgets and token absence from stored/exported data. Do not add an anonymous path to the existing protected Mock routes, cache raw capabilities, perform a database-wide workspace scan, or treat an offline native IPC router as an Internet-facing Mock service.

## Remaining scope

The first hosted slice does not finish native local public listening, organization/team permission models, custom public domains or unkeyed open Mock servers. Those remain explicit work toward the full `mock-011` row and final 395-item goal.
