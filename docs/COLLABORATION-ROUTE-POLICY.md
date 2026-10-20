# Hosted collaboration route inventory

This inventory covers every `.route` registered in `crates/server/src/lib.rs` at this commit. New routes must be classified here before members may use them. `No shared access` means the existing authenticated actor or anonymous route keeps its existing owner/account checks; membership alone never changes the actor to the workspace owner. Other routers composed from modules need their own audit before member access is granted.

| Route pattern | Member access | Private response / effect |
| --- | --- | --- |
| `/hosted/{*path}` | No shared access | Not assessed for member use |
| `/oauth1/flows` | No shared access | Not assessed for member use |
| `/oauth1/flows/{id}` | No shared access | Not assessed for member use |
| `/oauth1/flows/{id}/cancel` | No shared access | Not assessed for member use |
| `/oauth1/flows/{id}/complete` | No shared access | Not assessed for member use |
| `/oauth1/tokens/import` | No shared access | Not assessed for member use |
| `/workspaces/{id}/oauth1/tokens` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/oauth1/tokens/{token}/secret` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/oauth1/tokens/{token}` | No shared access | Not assessed for member use |
| `/workspaces/{id}/cookies` | No shared access | Not assessed for member use |
| `/dubbo/capability` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/dubbo/metadata/preview` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/dubbo/metadata/import` | No shared access | Not assessed for member use |
| `/workspaces/{id}/webhooks` | No shared access | Not assessed for member use |
| `/webhooks/{id}` | No shared access | Not assessed for member use |
| `/webhooks/{id}/captures` | No shared access | Not assessed for member use |
| `/webhooks/{id}/captures/{capture_id}` | No shared access | Not assessed for member use |
| `/webhooks/{id}/export` | No shared access | Not assessed for member use |
| `/webhooks/{id}/replay` | No shared access | Not assessed for member use |
| `/webhooks/replay/cancel` | No shared access | Not assessed for member use |
| `/webhooks/listener` | No shared access | Not assessed for member use |
| `/webhooks/listener/start` | No shared access | Not assessed for member use |
| `/webhooks/listener/stop` | No shared access | Not assessed for member use |
| `/auth/logout` | No shared access | Not assessed for member use |
| `/auth/me` | No shared access | Current authenticated account identity |
| `/auth/tokens` | No shared access | Not assessed for member use |
| `/auth/tokens/{id}` | No shared access | Not assessed for member use |
| `/workspaces` | No shared access | Not assessed for member use |
| `/workspaces/shared` | GET viewer | metadata |
| `/workspaces/{id}/invitations` | No shared access | Not assessed for member use |
| `/workspace-invitations/accept` | POST hosted session | membership |
| `/owners/{owner}/workspaces/{workspace}/invitations` | GET/POST admin hosted session | invitation metadata / new token |
| `/owners/{owner}/workspaces/{workspace}/invitations/{invitation}` | DELETE admin hosted session | none |
| `/owners/{owner}/workspaces/{workspace}/members` | GET admin hosted session | member identity |
| `/owners/{owner}/workspaces/{workspace}/audit` | GET admin hosted session | actor and action metadata |
| `/owners/{owner}/workspaces/{workspace}` | GET viewer; PUT editor | workspace; viewer projection |
| `/owners/{owner}/workspaces/{workspace}/run` | POST editor | outbound requests, private results and saved report |
| `/owners/{owner}/workspaces/{workspace}/run/cancel` | POST editor, creator only | instance-local cancellation keyed by actor and membership epoch; early cancellation and actor audit |
| `/owners/{owner}/workspaces/{workspace}/requests/{request}/execute` | POST editor | canonical saved request execution and private response; SOAP additionally fences credentials/revision and rejects scope overrides |
| `/owners/{owner}/workspaces/{workspace}/execute-draft` | POST editor | HTTP, GraphQL query/mutation or SOAP draft execution under canonical collection, credential/member/revision fences; SOAP rejects scope overrides |
| `/owners/{owner}/workspaces/{workspace}/sessions` | POST editor | canonical saved SSE/WebSocket/Data/TCP/Socket.IO/gRPC/MQTT/MCP/A2A/GraphQL subscription connection under expected revision |
| `/owners/{owner}/workspaces/{workspace}/sessions/{id}` | GET/DELETE editor, creator only | bound live session summary and removal |
| `/owners/{owner}/workspaces/{workspace}/sessions/{id}/events` | GET editor, creator only | bounded private live events |
| `/owners/{owner}/workspaces/{workspace}/sessions/{id}/send` | POST editor, creator only | WebSocket text/binary/Ping, Data query/schema/cancel, TCP send/half-close, Socket.IO emit/listen/ACK, gRPC message/half-close, MQTT publish/subscribe/unsubscribe/abort MCP request/callback/cancel or A2A request/local-stop; Data/TCP/Socket.IO/gRPC/MQTT/MCP/A2A commands record actor-only audit, canonical session scripts/snapshots and creator fences retained |
| `/owners/{owner}/workspaces/{workspace}/sessions/{id}/close` | POST editor, creator only | close bound live connection |
| `/owners/{owner}/workspaces/{workspace}/mock/{request}/{example}` | GET editor | saved Mock example preview and call record |
| `/workspaces/{workspace}/mcp/config/export` | No shared access | current owner/local saved MCP request export; shared native privacy pipeline screens custom Host names before encoding |
| `/owners/{owner}/workspaces/{workspace}/a2a/cards/import` | POST editor | canonical saved A2A request/dialect/revision-bound original Card candidate, inherited owner privacy, actor audit and credential/member/revision watcher/final fence; no persistence |
| `/owners/{owner}/workspaces/{workspace}/a2a/cards/read` | POST editor | canonical request-linked Card specification/dialect only, same privacy/audit/fences |
| `/owners/{owner}/workspaces/{workspace}/a2a/cards/discover` | POST editor | explicit URL, canonical owner auth/variables/scripts/TLS/timeout/checked network, bounded no-redirect Card fetch and candidate; same fences and actor cancellation |
| `/owners/{owner}/workspaces/{workspace}/a2a/cards/discover/cancel` | POST editor | actor-bound discovery id cancellation after saved request/revision and credential/member admission; action-only audit |
| `/owners/{owner}/workspaces/{workspace}/mcp/config/export` | POST editor | saved MCP request/revision-bound selected request export using native canonical privacy screening or explicit full data; actor audit, credential/member/revision watcher/final fence; no network/launch or persistence |
| `/owners/{owner}/workspaces/{workspace}/soap/import` | POST editor | saved SOAP request/revision-bound bounded WSDL/XSD candidate; canonical owner collection/env/request private screening, actor audit, credential/member/revision fences; no persistence |
| `/owners/{owner}/workspaces/{workspace}/soap/import-url` | POST editor | explicit checked/bounded source URL fetch with same admission/privacy/fences, no automatic imports or local overrides |
| `/owners/{owner}/workspaces/{workspace}/soap/schema` | POST editor | canonical saved SOAP request-linked WSDL schema, expected revision, actor audit and credential/member/revision release fence |
| `/owners/{owner}/workspaces/{workspace}/grpc/import` | POST editor | saved gRPC request-bound Proto candidate, expected revision, actor audit, credential/member/version watcher and final fence; no persistence |
| `/owners/{owner}/workspaces/{workspace}/grpc/schema` | POST editor | saved request-linked protobuf source, expected revision, actor audit and credential/member/version final fence |
| `/owners/{owner}/workspaces/{workspace}/grpc/reflect` | POST editor | canonical saved request/environment reflection, actor credentials with owner resources, empty locals, actor audit and credential/member/version watcher/final fence; candidate only |
| `/owners/{owner}/workspaces/{workspace}/graphql/introspect` | POST editor | saved canonical GraphQL request under expected revision; actor audit and member/revision/credential final fence; returns candidate only |
| `/owners/{owner}/workspaces/{workspace}/graphql/schema` | POST editor | saved request-bound GraphQL specification; expected revision, actor audit and final member/revision/credential fence |
| `/owners/{owner}/workspaces/{workspace}/specifications/preview` | POST editor | bounded OpenAPI import/replacement candidate; expected revision, actor audit and final membership/revision fence; no persistence |
| `/owners/{owner}/workspaces/{workspace}/dataset/preview` | Editor/admin session | Bounded literal CSV/JSON preview; canonical revision and membership admission/final fences, actor audit without source |
| `/owners/{owner}/workspaces/{workspace}/mock-schema/validate` | Editor/admin session | Canonical saved HTTP/spec Schema validation; actor admission audit without source/pointer, membership and workspace revision fences before metadata response |
| `/owners/{owner}/workspaces/{workspace}/mocks/{*path}` | ANY editor | bounded saved Mock routing and selected script execution; actor audit plus membership/revision fences |
| `/owners/{owner}/workspaces/{workspace}/mock-calls` | GET editor | Mock call metadata |
| `/owners/{owner}/workspaces/{workspace}/history` | GET editor | saved redacted response history |
| `/owners/{owner}/workspaces/{workspace}/reports` | GET editor | saved report summaries |
| `/owners/{owner}/workspaces/{workspace}/reports/{report}` | GET editor | saved report detail |
| `/owners/{owner}/workspaces/{workspace}/reports/{report}/export` | GET editor | bounded export of the saved redacted report; admitted actor/report ID audited before read, membership rechecked before release |
| `/owners/{owner}/workspaces/{workspace}/reports/{report}/steps/{position}/response` | GET editor | saved response detail |
| `/owners/{owner}/workspaces/{workspace}/members/{member}` | PATCH/DELETE admin hosted session | member identity |
| `/workspaces/{id}` | No shared access | Not assessed for member use |
| `/workspaces/{id}/versions` | No shared access | Not assessed for member use |
| `/workspaces/{id}/requests/{request}/versions` | No shared access | Not assessed for member use |
| `/workspaces/{id}/requests/{request}/versions/{revision}` | No shared access | Not assessed for member use |
| `/workspaces/{id}/requests/{request}/versions/{revision}/restore` | No shared access | Not assessed for member use |
| `/workspaces/{id}/versions/{revision}/restore` | No shared access | Not assessed for member use |
| `/workspaces/{id}/doc-publications` | No shared access | Not assessed for member use |
| `/workspaces/{id}/doc-publications/{publication}` | No shared access | Not assessed for member use |
| `/workspaces/{id}/ci-preset` | No shared access | Not assessed for member use |
| `/workspaces/{id}/history` | No shared access | Not assessed for member use |
| `/workspaces/{id}/mock-calls` | No shared access | Not assessed for member use |
| `/workspaces/{id}/mock-shares` | No shared access | Not assessed for member use |
| `/workspaces/{id}/mock-schema/validate` | No shared access | Not assessed for member use |
| `/workspaces/{id}/mock-shares/{share_id}` | No shared access | Not assessed for member use |
| `/workspaces/{id}/mock-shares/{share_id}/rotate` | No shared access | Not assessed for member use |
| `/mock-public/listener` | No shared access | Not assessed for member use |
| `/mock-public/listener/start` | No shared access | Not assessed for member use |
| `/mock-public/listener/stop` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/reports` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/reports/{report}` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/reports/{report}/export` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/reports/{report}/steps/{position}/response` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/schedules` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/schedules/preview` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/schedules/{schedule}` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/schedules/{schedule}/run` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/schedules/{schedule}/cancel` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/schedules/{schedule}/runs` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/notifications` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/notifications/{target}` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/notification-deliveries` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/notification-deliveries/{delivery}/retry` | No shared access | Not assessed for member use |
| `/workspaces/{id}/run` | No shared access | Not assessed for member use |
| `/workspaces/{id}/run/cancel` | No shared access | Not assessed for member use |
| `/testing/dataset/preview` | No shared access | Not assessed for member use |
| `/runners` | No shared access | Not assessed for member use |
| `/runners/{id}` | No shared access | Not assessed for member use |
| `/runners/{id}/credentials` | No shared access | Not assessed for member use |
| `/runners/{id}/credentials/{credential}` | No shared access | Not assessed for member use |
| `/runners/{id}/claim` | No shared access | Not assessed for member use |
| `/runners/{runner}/tasks/{task}/heartbeat` | No shared access | Not assessed for member use |
| `/runners/{runner}/tasks/{task}/complete` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/runner-tasks` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/runner-tasks/{task}/cancel` | No shared access | Not assessed for member use |
| `/workspaces/{id}/export` | No shared access | Not assessed for member use |
| `/workspaces/{id}/variables/import-preview` | No shared access | Not assessed for member use |
| `/workspaces/{id}/variables/export` | No shared access | Not assessed for member use |
| `/oauth2/flows` | No shared access | Not assessed for member use |
| `/oauth2/flows/{id}` | No shared access | Not assessed for member use |
| `/oauth2/flows/{id}/cancel` | No shared access | Not assessed for member use |
| `/oauth2/flows/{id}/complete` | No shared access | Not assessed for member use |
| `/oauth2/tokens/acquire` | No shared access | Not assessed for member use |
| `/oauth2/tokens/import` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/oauth2/tokens/{token}/refresh` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/oauth2/tokens/{token}/secret` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/oauth2/tokens/{token}/revoke` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/oauth2/tokens/{token}/introspect` | No shared access | Not assessed for member use |
| `/workspaces/{id}/oauth2/tokens` | No shared access | Not assessed for member use |
| `/workspaces/{workspace}/oauth2/tokens/{token}` | No shared access | Not assessed for member use |
| `/execute` | No shared access | Not assessed for member use |
| `/generation/projects/catalog` | No shared access | Not assessed for member use |
| `/generation/projects` | No shared access | Not assessed for member use |
| `/generation/projects/import` | No shared access | Not assessed for member use |
| `/generation/projects/regenerate` | No shared access | Not assessed for member use |
| `/generation/projects/cancel` | No shared access | Not assessed for member use |
| `/generation/snippets/catalog` | No shared access | Not assessed for member use |
| `/generation/snippets` | No shared access | Not assessed for member use |
| `/soap/import` | No shared access | Not assessed for member use |
| `/soap/import-url` | No shared access | Not assessed for member use |
| `/soap/schema` | No shared access | Not assessed for member use |
| `/soap/template` | No shared access | Not assessed for member use |
| `/a2a/cards/discover/cancel` | No shared access | Not assessed for member use |
| `/a2a/cards/import` | No shared access | Not assessed for member use |
| `/a2a/cards/discover` | No shared access | Not assessed for member use |
| `/graphql/introspect` | No shared access | Not assessed for member use |
| `/graphql/schema` | No shared access | Not assessed for member use |
| `/grpc/schema` | No shared access | Not assessed for member use |
| `/grpc/import` | No shared access | Not assessed for member use |
| `/grpc/reflect` | No shared access | Not assessed for member use |
| `/sessions` | No shared access | Not assessed for member use |
| `/sessions/{id}` | No shared access | Not assessed for member use |
| `/sessions/{id}/events` | No shared access | Not assessed for member use |
| `/sessions/{id}/send` | No shared access | Not assessed for member use |
| `/sessions/{id}/close` | No shared access | Not assessed for member use |
| `/import` | No shared access | Not assessed for member use |
| `/mock/{workspace_id}/{request_id}/{example_id}` | No shared access | Not assessed for member use |
| `/mocks/{workspace_id}/{*path}` | No shared access | Not assessed for member use |
| `/workspaces/{id}/sync` | No shared access | Not assessed for member use |
| `/sync/status` | No shared access | Not assessed for member use |
| `/sync/connect` | No shared access | Not assessed for member use |
| `/public/docs/{id}` | Public / unauthenticated | Existing public contract |
| `/public/docs/{id}/unlock` | Public / unauthenticated | Existing public contract |
| `/health` | Public / unauthenticated | Existing public contract |
| `/auth/status` | Public / unauthenticated | Existing public contract |
| `/auth/register` | Public / unauthenticated | Existing public contract |
| `/auth/login` | Public / unauthenticated | Existing public contract |
| `/hooks/{token}` | Public / unauthenticated | Existing public contract |
| `/docs/{id}` | Public / unauthenticated | Existing public contract |

Inventory: 149 route patterns; 22 explicit member-facing patterns; 8 public patterns. A route without an explicit shared policy is unavailable through owner-scoped member URLs.

The workspace-scoped routes currently supported for members are the explicit owner/workspace pair above. Shared collection, canonical saved-request, finite HTTP draft and saved Mock example execution poll membership every 200 ms and withhold results after a role change or removal; draft execution also fences canonical workspace revision. Saved SSE/WebSocket/Data member sessions bind the creator, poll role/revision/credential changes every 200 ms and recheck before returning summaries/events or acknowledging sends. The actor is recorded before execution. Shared history/report/Mock-call reads recheck membership before returning private data. Other protocol sessions, public Mock share management, full workspace exports, credentials and resource routes remain closed to members until their actor attribution, cancellation and response fencing are implemented and tested.
