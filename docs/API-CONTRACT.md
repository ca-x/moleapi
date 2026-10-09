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
`expected` for JSON assertions is valid JSON text; `target` is RFC6901 JSON pointer, e.g. `/data/id`. A JSON string is written with quotes. For status/duration expected is numeric string; duration checks <=. Interpolation of all request fields uses enabled environment variables; unresolved variables cause 400. Finite HTTP/GraphQL/SOAP target URLs allow HTTP/S; dedicated live protocol sessions have their own validated transports. Body max 5MiB; response max 5MiB and truncated flagged. Bound maximum timeout to 120s. GET/POST/PATCH/PUT/DELETE/HEAD/OPTIONS supported. History excludes request auth/headers/body and redacts URL queries for secret variables and common key/token/password query keys; response headers redact set-cookie; response body may contain endpoint-returned sensitive content and UI must document this.

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

## Script and scoped-variable extension (additive)

`Pair.local_value?:string|null` is a native-only persisted local override. Hosted workspace writes remove local_value before saving. In hosted browser mode private overrides reside in browser storage partitioned by account/workspace. An explicit empty string is an override; missing/null follows the shared value. Default export and native synchronization remove locals.

`WorkspaceData.global_variables?:Pair[]`, `Collection.variables?:Pair[]` and `pre_request_script?:string` / `post_response_script?:string` on project, collection and request default to empty when reading old workspaces. Lookup precedence: temporary > dataset > active environment > collection > project. Execute and run accept `variables?:Pair[]` (temporary), `data?:Pair[]` (dataset) and `locals?:VariableUpdate[]` (selected scope local overlays). `VariableUpdate {scope:"project"|"collection"|"environment"|"data"|"temporary",key:string,value?:string}`. The scoped `locals` input is execution-only and accepts project/collection/environment scopes. No transient value silently changes shared workspace variables.

`Response.logs?:{level:string,message:string}[]`, `variable_updates?:VariableUpdate[]`, `request_updates?:{field:string,value:string}[]` are live execution results. History omits these fields' values. Frontend applies project/collection/environment updates as local values in the originating scope only while its account/workspace identity is still current; collection runs return ordered request results. Temporary/dataset changes remain within the current execution/run.

Pre scripts run project -> collection -> request; post scripts request -> collection -> project. The limited synchronous `moleapi-pm/1` compatibility surface is specified in docs/specs/execution-extensions.md. Unsupported async/network/flow/Vault APIs return concrete errors. Saved/imported script originals are retained even when they use currently unsupported APIs. This is not a blanket claim of full Postman JavaScript sandbox compatibility.

Execution isolation: private worker mode receives JSON only on stdin and emits bounded JSON only on stdout; credentials never appear in CLI arguments. Each phase has a 250ms cooperative VM deadline and a distinct 1s host deadline including startup/IPC. The host kills and reaps overdue workers; 64MiB VM heap and512KiB VM stack caps remain. Failed phases do not commit request/variable mutations, but attempted private values still taint history redaction. If a worker is terminated before full taint can be recovered, persistence uses conservative redaction. Scoped fix reviews and67 local Rust tests passed; platform release evidence is reported separately.


Custom library embeddings using script execution call `moleapi_server::dispatch_script_worker()` before normal application startup, as the shipped server and desktop mains do. Existing `local(Path)` and `hosted(Config)` constructors use the current application executable for private worker dispatch. Additive `local_with_worker(database_path: &Path, worker: &Path)` and `hosted_with_worker(config: Config, worker: &Path)` accept an explicit trusted absolute worker executable for embeddings/tests. This path is never exposed as an HTTP input and never resolved through PATH. Private worker stdout/stderr is not an application log channel.


## SOAP / WSDL

`Protocol {kind:"soap",version:"1.1"|"1.2",service:string,port:string,operation:string,action:string}` stores protocol metadata; the canonical envelope is ordinary `RequestSpec.body` (`body_kind:"text"`, POST). An incomplete XML draft can save; execution validates after pre-request scripts.

`Specification {kind:"wsdl",dialect:"wsdl1.1",source:string}` preserves a serialized `{entry_file,files:[{path,content}]}` bundle. Relative ASCII paths, supplied imports only, up to32 files/2MiB total; XML node/depth/schema/template limits apply.

Owner-bound `POST /api/soap/import {workspace_id,name,source}` and `/api/soap/import-url {workspace_id,name,url,timeout_ms?,verify_tls?}` return `{specification,schema}` candidates, attached through normal workspace CAS saves. Import may include environment_id/variables/data/locals for private scoped metadata screening. URL imports use checked HTTP GET without redirects or automatic dependency downloads.

`POST /api/soap/schema {workspace_id,specification_id}` reads saved sources. `POST /api/soap/template {workspace_id,specification_id,service,port,operation}` returns `{template,fields,action,version,address}` or an explicit unsupported-template error. Schema has services/ports/bindings/operations, templates and fields; operation metadata includes style/use/input_elements/binding_supported/error.

The finite `/api/execute` pipeline generates version-specific headers, applies shared auth/variables/scripts/network/TLS/timeout/response caps, and validates selected operation plus envelope after pre-scripts. Actual HTTP status/body/headers remain unchanged; optional `Response.soap_fault {version,code,reason,detail,actor?}` describes both200 and error-status Faults. Default history excludes structured Fault and screens private XML; default native exports screen XML payloads and canonical source credential literals/URLs, while explicit include_secrets export preserves originals.


## MCP sessions and original Host configuration

Saved protocol fields: `{kind:"mcp",transport:"http"|"stdio",command,args:string[],env:Pair[],operation:"tools/call"|"resources/read"|"prompts/get",name,arguments_source,uri,config_source?:string}`. `config_source` is the exact selected original Host entry JSON; it is preserved without interpolation. Connection resolves scoped configuration while retaining protocol kind/transport identity. Hosted STDIO requires `MOLEAPI_MCP_STDIO_ALLOWLIST` absolute paths; native/local connects explicitly authorize their selected process.

Existing `/api/sessions` endpoints apply. Send commands: `mcp_request {request_id,method:"refresh"|"tools/call"|"resources/read"|"prompts/get"|"resources/subscribe"|"resources/unsubscribe",name?,uri?,arguments_source?}`, `mcp_callback {callback_id,result?:JSON,error?:{code,message,data?}}`, `mcp_cancel {request_id}`. Events: `mcp_initialized {info}`, `mcp_capabilities {tools,resources,resource_templates,prompts}`, `mcp_result {request_id,method,result}`, `mcp_error {request_id,method,error}`, `mcp_notification {method,params}`, `mcp_callback {callback_id,method,params}`. `moleapi/callback_completed` notification carries `{callback_id,status:"replied"|"expired"}`.

Actual server protocol errors remain event data, not MoleAPI401 authentication expiry. Connection handshake is null for MCP rather than an invented HTTP200; negotiated version/server identity are initialization data. Manual callback responses are validated typed SDK messages, bounded and scoped; no automatic AI calls or filesystem roots collection. SDK body/framing/input quotas are enforced before parse, command512 admission before queueing, and disconnect/logout/delete closes peer tasks and reaps process groups/jobs. See specs/mcp-client.md and protocol coverage for explicit limitations.


## A2A cards and sessions

`Protocol {kind:"a2a",dialect:"0.3"|"1.0",transport:"jsonrpc"|"http-json",operation,params_source:string,card_source?:string|null,interface_url?:string|null}` retains raw invocation/card JSON. Legacy0.3 permits JSONRPC only. Typed SDK invocation resolves parameters separately; generic transport interpolation does not modify raw sources.

`Specification {kind:"a2a-agent-card",dialect:"a2a-0.3"|"a2a-1.0",source:string}` preserves original card text. Owner-bound POST `/api/a2a/cards/import {workspace_id,source,dialect,...scope}` and `/api/a2a/cards/discover {workspace_id,request,environment_id?,locals?,discovery_id?}` return `{specification,card,dialect,interfaces:[{url,transport,version,supported}],warnings}` candidates. Discover uses an exact `.json` URL or the origin `/.well-known/agent-card.json`, without redirects or interface adoption. POST `/api/a2a/cards/discover/cancel {workspace_id,discovery_id}` cancels the owning discovery. Save newly created requests before discovery; attach candidates through normal CAS writes.

Existing sessions accept `a2a_request {request_id,method,params_source}` and `a2a_stop {request_id}`. Methods: message/send, message/stream, tasks/get, tasks/cancel, tasks/resubscribe, current1.0 tasks/list, and explicit tasks/pushNotificationConfig/set|get|list|delete aliases. Parameters use the selected SDK's dialect, with no handwritten version translation. Implicit push configuration inside message/send or message/stream is rejected.

Events: `a2a_ready {dialect,transport}`, `a2a_result|a2a_stream {request_id,method,result}`, `a2a_error {request_id,method,error}`, `a2a_finished {request_id,method}`. Local readiness has null HTTP handshake and does not claim remote authentication. Stop cancels/reaps local request work; remote tasks/cancel returns the actual Agent response. Body/frame/node/depth, method and task/history/page limits are enforced before decoding. Original/decoded card metadata and event keys/scalars use private-value screening. No external file URI fetch or automatic callback/push registration occurs.


## Portable HTTP request snippets

Authenticated GET `/api/generation/snippets/catalog` returns `{engine,targets:[{target,title,clients:[{client,title}]}],validation:"engine-smoke",scope}`. Upstream catalog availability is separate from actual per-target compilation/run evidence in SNIPPET-COVERAGE.md.

POST `/api/generation/snippets {workspace_id,request_id,target,client,include_secrets?:false}` loads the owner's saved request, returns `{engine,target,client,code,warnings,include_secrets}`; unknown fields and unsupported protocol/body/auth shapes are rejected. The frontend saves dirty drafts before generation. The same routes operate through local/native IPC. No arbitrary user HAR/JavaScript, commands, custom templates or output paths enter this API.

Default generation screens literal/known private values, user JSON keys/scalars and recognized encodings; fixed RequestSpec/Auth/Pair keys are not redacted. Generation-only Base64 decoding of full string values/explicit Basic headers is bounded to16KiB. Explicit credential inclusion preserves saved values, but does not attach browser-local overrides or resolve variable references. No pre-scripts or generated requests run. Mature engine owns all source emitters. Each isolated QuickJS call has512KiB input,64MiB memory,512KiB stack,250ms interrupt and2MiB output limits; four actual worker-held service slots prevent unbounded admission. Closing/changing dialog context discards late UI results; blocking generation finishes within its engine budget rather than claiming an external cancel operation.


## TCP byte sessions

`Protocol {kind:"tcp",framing:"raw"|"lines"|"length_be"|"length_le",max_frame_bytes:1048576,no_delay:true,idle_timeout_ms:0,message:{encoding:"text"|"hex"|"base64",payload_source:string,secret:false}}` preserves original payload source and does not send automatically. URL is tcp:// or tcps:// with explicit port and no userinfo/path/query/fragment. GET/None is the connection envelope; HTTP auth/headers/query/post scripts are rejected at invocation. Project/collection/request pre-scripts and scoped transport variables use the common pipeline.

Send through existing owner-bound `/api/sessions/{id}/send`: `tcp_send {message:TcpMessage}` and `tcp_half_close`. Payload resolves against the connection's captured scoped environment at send time; reconnect after changing environment values. Text is UTF8, Hex accepts whitespace via mature decoder, Base64 is standard. Decoded payload/frame1MiB and source2MiB limits apply before queue admission. Half-close rejects further sends but keeps reading until EOF/error/cancel.

Events: `tcp_data {base64,text:string|null,bytes,redacted}` with incoming/outgoing direction; `tcp_half_closed` confirms physical send-side shutdown. Summary handshake remains null. Actual successful socket writes and socket reads account wire bytes, including framing and TLS records/handshake. Idle starts at established connection and resets with actual IO, rather than consuming DNS/connect time.512 sends,4096 inbound frames,20MiB input/wire,32 queued commands and existing owner/lifetime/retention bounds are enforced. Receive quota may include the final bounded read that crosses the limit, then closes immediately.

Known nonempty private values or explicit secret send make subsequent byte events conservatively payload-free while retaining size/direction/status, including fragmented echoes. Default native export screens JSON private fields/copies even inside Hex/Base64; only safely unchanged decoded JSON or exact variable references retain encoded drafts, opaque encoded bytes are withheld. Explicit include-secrets retains exact originals. No custom protocol parser, automatic file/network resource retrieval, post/event scripts or durable stream history is implied.


### Project regeneration file inputs

`POST /api/generation/projects/import` accepts `{workspace_id, job_id, role, source}` for the authenticated workspace owner. `role` is `previous` or `working`. `source` is `{kind:"zip", archive_base64}` or `{kind:"files", files:[{path,encoding,content,executable}]}`; the selected-file form is limited to working input. The reply is `{files:[ProjectFile]}` with actual byte sizes, SHA256, encoding and safe executable flags. Previous ZIPs require an original matching `moleapi-generation-v1` manifest. Current inputs allow edits, additions and deletions and exclude old regeneration metadata. ZIP parsing uses the capped project worker, existing `/api/generation/projects/cancel` jobs, admission/quota checks and final owner/workspace validation; imported paths are never host filesystem paths. Pass the returned baseline files to `/api/generation/projects/regenerate`, and project current files to its working-file shape without hash/size fields. Existing snapshot JSON remains supported.


### OpenAPI project source bundles

A canonical openapi specification may use a JSON source envelope `{format:"moleapi-openapi-source-v1",entry_file,files:[{path,content}]}` with relative UTF8 paths. `/api/generation/projects` detects this format, assembles only supplied documents in the capped/cancellable project worker, then applies default privacy to the assembled document and every parsed source file before emitting. Workspace source stays unchanged. Existing owner/admission/quota/cancel routes apply throughout both worker stages. Source/envelope and parsed/assembled documents remain bounded by1MiB,64 files and existing node/depth/worker limits. Explicit resource/anchor/sibling limitations are in specs/code-generation.md. Ordinary sources cannot use external discriminator mapping URLs to bypass the no-fetch policy.
