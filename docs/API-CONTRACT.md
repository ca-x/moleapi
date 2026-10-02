# MoleAPI API v1 contract
All JSON keys snake_case. The same paths run in hosted HTTP and standalone Tauri IPC (method/path/body -> JSON). Errors: HTTP status + `{error:string, code?:string}`. API paths never use SPA fallback.

## Shared data (Rust and TypeScript must match exactly)
```
Pair { id:string, key:string, value:string, enabled:boolean, secret?:boolean }
Auth { kind:"none"|"bearer"|"basic", token:string, username:string, password:string }
Assertion { id:string, name:string, kind:"status"|"duration"|"contains"|"json", target:string, expected:string }
Example { id:string, name:string, status:number, headers:Pair[], body:string }
RequestSpec {
 id:string, name:string, method:string, url:string, description:string,
 query:Pair[], headers:Pair[], body_kind:"none"|"json"|"text"|"form", body:string,
 auth:Auth, timeout_ms:number, follow_redirects:boolean, verify_tls:boolean,
 assertions:Assertion[], examples:Example[]
}
Collection {id:string,name:string,description:string,requests:RequestSpec[]}
Environment {id:string,name:string,variables:Pair[]}
WorkspaceData {schema_version:1,collections:Collection[],environments:Environment[],active_environment_id:string|null}
Workspace {id:string,name:string,revision:number,updated_at:string,data:WorkspaceData}
Response {status:number,status_text:string,headers:Pair[],body:string,body_base64?:string,elapsed_ms:number,size_bytes:number,truncated:boolean,url:string,tests:TestResult[]}
TestResult {id:string,name:string,passed:boolean,actual:string,expected:string}
HistoryEntry {id:string,workspace_id:string,request_id:string,request_name:string,method:string,url:string,status:number,elapsed_ms:number,size_bytes:number,created_at:string,response:Response}
```
`expected` for JSON assertions is valid JSON text; `target` is RFC6901 JSON pointer, e.g. `/data/id`. A JSON string is written with quotes. For status/duration expected is numeric string; duration checks <=. Interpolation of all request fields uses enabled environment variables; unresolved variables cause 400. Request protocol allows HTTP/S only. Body max 5MiB; response max 5MiB and truncated flagged. Bound maximum timeout to 120s. GET/POST/PATCH/PUT/DELETE/HEAD/OPTIONS supported. History excludes request auth/headers/body and redacts URL queries for secret variables and common key/token/password query keys; response headers redact set-cookie; response body may contain endpoint-returned sensitive content and UI must document this.

## Server/library constructors
`moleapi_server::local(database_path: &std::path::Path) -> anyhow::Result<axum::Router>` async. Returns only API routes; local mode bypasses auth, exclusively for IPC. Never expose this router over a network.
`moleapi_server::Config {database_url: String, setup_token: String, allow_registration: bool, allow_private_network: bool}`.
`moleapi_server::hosted(config: Config) -> anyhow::Result<axum::Router>` async, including static assets when feature `web` enabled (default). Desktop depends `moleapi-server` with default-features=false.
`moleapi_core` owns serializable shared structs and executor. Core and server own Cargo.toml within their directories. Root owns root Cargo.toml and desktop.

## Auth
GET /api/health -> {status:"ok",version:string}
GET /api/auth/status -> {mode:"server"|"desktop",setup_required:boolean,registration_enabled:boolean}
POST /api/auth/register {username,password,setup_token?} -> {token,username}; require setup_token for first registration and CAS first-user creation; later registration respects setting. Username 3–64 chars; password >=10 chars. Argon2 hashing offloaded blocking. Sessions random 256bit, store only hash, expiry 30d.
POST /api/auth/login {username,password} -> {token,username}
POST /api/auth/logout -> {ok:true}; invalidate bearer session.
All other hosted /api endpoints require bearer token. Local identity fixed `local`. No permissive hosted CORS.

## Workspace operations
GET /api/workspaces -> Workspace[]
POST /api/workspaces {id?:string,name:string,data:WorkspaceData} -> Workspace (revision 1 for a fresh ID; recreated IDs continue their durable revision counter); chosen id scoped per account. Validate data at write time.
GET /api/workspaces/:id -> Workspace
PUT /api/workspaces/:id {name,data,expected_revision:number} -> Workspace; stale ->409, atomic CAS. Preserve prior snapshot before replacing.
DELETE /api/workspaces/:id {expected_revision:number} -> {ok:true}; stale ->409.
GET /api/workspaces/:id/versions -> Workspace[]; prior revisions, most recent first.
POST /api/execute {workspace_id:string,request:RequestSpec,environment_id?:string|null} -> Response; store redacted history.
GET /api/workspaces/:id/history -> HistoryEntry[] (latest100)
DELETE /api/workspaces/:id/history -> {ok:true}
POST /api/workspaces/:id/run {collection_id:string,environment_id?:string|null} -> {results:[{request_id,request_name,response?:Response,error?:string}],passed:number,failed:number,elapsed_ms:number}
GET /api/mock/:workspace_id/:request_id/:example_id -> saved example's actual status/headers/body; authenticated, don't allow unsafe hop-by-hop headers.

## Native-only synchronization (hosted ->404)
GET /api/sync/status -> {connected:boolean,server_url?:string,username?:string}; never expose token.
POST /api/sync/connect {server_url,username,password} -> same status; login remote and persist token natively. Require HTTPS except loopback HTTP and never send credentials on cross-origin redirects.
DELETE /api/sync/connect -> {connected:false}; remove connection and sync bases when switching account/server. Don't delete local workspaces.
POST /api/workspaces/:id/sync {resolution?:"push"|"pull"} -> {status:"synced"|"conflict",workspace:Workspace,remote?:Workspace,message:string}; sync CAS uses durable bases (local revision + remote revision + account/server identity); no divergence =>no-op; only local changed =>push; only remote changed =>pull; both =>conflict without overwrite. Initial identical data =>record bases; initial missing remote =>create; initial differing data =>conflict. Remote token never reaches UI. Preserve snapshot before explicit pull; force push still uses fetched remote revision CAS. Local edits during network waits must be rejected, not acknowledged as synced.

CLI: `moleapi-server [--bind 127.0.0.1:8787] [--database ./data/moleapi.db] [--allow-private-network] [--allow-registration]`; env MOLEAPI_BIND/DATABASE_URL/SETUP_TOKEN/ALLOW_PRIVATE_NETWORK/ALLOW_REGISTRATION. Shutdown SIGINT/SIGTERM. Print generated first-user setup token only while setup is required. SQLite parent directory created. Core integration tests exercise networking and policy; router tests auth isolation/CAS/mock; sync tests ideally two local networked hosted servers under test plus native router.

## Database requirement
Read docs/specs/database.md. SeaORM SQLite (default), PostgreSQL and MySQL. Config.database_url; local(Path) always SQLite. No dialect-specific business logic.

## Formats endpoints
POST /api/import {format,content} -> ImportResult {name,data,warnings}. POST /api/workspaces/:id/export {format,include_secrets} -> ExportResult {filename,content,mime}. moleapi-formats crate owned by root.

## Verification additions
Resolved string expansion is checked during construction (5MiB/string,20MiB aggregate), not after allocation. Saved Mock error examples preserve their actual status/headers/body; only framework/API errors are normalized to JSON. Sync persists remote content fingerprint alongside revisions; deletes retain only ID/revision tombstones so stale clients cannot overwrite a recreated resource. URI path encoding preserves RFC3986 unreserved characters.
