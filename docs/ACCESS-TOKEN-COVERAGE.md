# Personal API tokens

Hosted accounts can create independent named automation credentials, list metadata and revoke them. The API accepts1–365days with90days default,1–80character names and at most100 stored tokens per account, including expired tokens until explicitly revoked. Tokens inherit the account's existing owned-resource permissions; they are separate from saved request API keys and browser login sessions. Offline/native databases do not issue hosted credentials.

- `GET /api/auth/tokens`: account-owned metadata only.
- `POST /api/auth/tokens`: `{name,expires_in_days}`; metadata plus one-time `token`.
- `DELETE /api/auth/tokens/{id}`: owned revocation.

Management requires a login session; PAT-authenticated callers cannot list/mint/revoke credentials. The standard Bearer guard and admission recheck share session/PAT lookup. Expired or deleted PATs fail future requests. PAT logout revokes itself; browser logout removes only its session and does not delete separately managed PATs. Revocation cancels existing owner admissions, jobs and protocol connections on the current instance and propagates to hosted instances sharing the database, matching the established account-wide logout lifecycle; other login sessions remain valid for future calls.

The additive SeaORM migration creates an indexed relational table with UUID ID/owner/name/created/expiry metadata and a unique SHA256 digest. Token plaintext contains32 cryptographic random bytes and an identifying prefix. It is never stored in the database, metadata responses, workspace documents, snapshots or synchronization. Per-account quota counting/insertion follows a transactional account-row update to serialize creators on supported engines. No homegrown cryptography or token parser is introduced.

Hosted settings provide the existing Radix name/expiry form, metadata/expiry list, one-time copy/hide view and explicit revoke confirmation. The view explains that revocation stops active tasks/connections and propagates to other service instances. Query keys separate account/credential generations without storing bearer values. Late creation results are fenced by account, credentials and component lifetime; dismiss/unmount/account or credential changes discard plaintext. UI strings are English/Chinese; dates follow the selected language. No new decorative animation is added to sensitive values.

CLI `tokens list/create/revoke` uses the same authenticated endpoints. Created secrets are written only to atomic private output files; stdout contains metadata. A private temporary output is prepared before issuance, and existing files are refused without explicit overwrite. Persist failures trigger owned cleanup; failed cleanup identifies the token ID for explicit revocation. Native command/output privacy remains distinct from the optional upstream Newman mode.

## Actual scoped evidence

- Server `access_tokens` fixtures: real hosted Bearer access, owner isolation, session-only management, one-time response/metadata/digest storage, full workspace export excluding token credentials, persisted expiry/revocation/logout, offline rejection, invalid input and concurrent100-token quota across two hosted routers sharing SQLite. A real slow owned HTTP run is cancelled on revocation.
- Server migration fixture: upgrade from the actual prior two migrations preserves accounts/sessions and records the additive third migration; repeat initialization does not duplicate migrations.
- CLI `tokens` process fixture: create/no-overwrite,0600 Unix files, metadata-only stdout/list, actual PAT remote command access, management denial and subsequent revoked-token denial.
- CLI `snippets` process fixture reran after shared atomic output preparation changed; private no-overwrite behavior remains intact.
- Frontend component fixtures: actual Radix create/copy/hide/confirm, no raw token in query cache, same-account credential change dropping pending plaintext. TypeScript and translation catalogs pass.

These checks passed locally. PostgreSQL/MySQL migration/quota contention, multi-platform native UI/CLI execution and final hosted-browser integration remain for unified checks after functional completion. Fine-grained/enterprise token scopes/audit trails remain incomplete. No formal review, local Docker build, final main merge or release was performed; the full feature matrix remains active.

## Shared-database revocation propagation

Explicit PAT revoke, PAT logout and session logout now commit credential deletion with an owner revocation revision in one SeaORM transaction. An account-row update serializes revision publishers; the existing internal settings table stores one counter per owner without plaintext credentials. Hosted routers load a startup baseline and poll every second. They apply new revisions under the existing owner admission gate and reuse job/protocol/webhook/OAuth/cookie invalidation. The initiating instance consumes the revision immediately, preventing duplicate cancellation on its later poll. Dropping the router cancels the poller; offline/local routers have no polling loop.

A real three-router/shared-SQLite fixture passed live HTTP run cancellation, WebSocket closure on another instance, continued connectivity for a different account, denied future revoked-token requests, a fresh instance retaining a new connection after a historical event, and subsequent session logout propagating to it. A separate two-router concurrency fixture passed simultaneous revokes with exactly two persisted revisions and no extra revision for a missing token. All three existing token API fixtures and the session digest/expiry unit fixture passed after the change. Propagation is eventual: one polling interval plus DB and cleanup latency; database failures retry without consuming the event. Natural expiry rejects new calls but does not publish an explicit cancellation event. PostgreSQL/MySQL deployment behavior and platform integration remain for unified checks; this does not prove distributed runner or full-matrix completion.
