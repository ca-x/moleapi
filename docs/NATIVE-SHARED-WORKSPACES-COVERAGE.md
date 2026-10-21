# Native hosted shared workspaces

The native member workbench operates online against the hosted owner workspace. The local `/api/sync/collaboration` bridge keeps the hosted bearer in Rust, accepts only registered member controls and returns inert JSON. Owned local data and synchronization stay independent. This checkpoint does not complete the full capability matrix.

## Verified behavior

- Explicit method/path/query/mode policy, fixed hosted origin, pinned checked transport, no redirects/proxy/header overrides; bounded JSON and raw Mock replies. Normalized JSON values/keys and one UI-parsed JSON text layer cannot return the current hosted bearer. The encoded JSON-text regression failed before the fix and all8 member integration cases pass afterward.
- Connection generations fence headers, streamed replies, retained actions and account changes without holding the synchronization lock across network waits. Disconnect/replacement attempts bounded remote logout outside the lock; offline revocation cannot be guaranteed. Unknown Save outcomes are never replayed automatically.
- Native shared picker, manual same-server invitation acceptance, owner/admin/editor/viewer controls, HTTP, Mock, Data, source editors and protocol sessions use the member adapter. Local file dialogs retain current-account guards. Actual remote401 clears member scope while local workspaces remain usable; simulated Mock401 remains response data.
- Real local native Router plus production frontend invoke harness and a separate hosted server exercised failed-login retry, invitation acceptance, remote HTTP/CAS saves, canonical dataset Mock201, viewer denial/admin management, GraphQL subscription/Stop, SSE cleanup, disconnect/logout and current-token expiry. The same-ID local workspace remained exactly unchanged. Workspace IDs containing encoded spaces, question marks and hashes worked; raw fragments were rejected.
- Full frontend147 files/796 cases, TypeScript and production38 chunks passed before the packaging version change. Five independent-review findings received failing regressions and fixes; original reviewer rechecked clean, including the final bounded JSON-text privacy filter. Release-specific verification is recorded separately.

## UI and accessibility

English and Chinese390/1280 views had no document overflow. Mock widget audits passed25 rules with0 violations/0 incomplete; invitation audits passed13 rules with0 violations. English invitation contrast had1 incomplete overlap check, manually inspected as readable; Chinese had0 incomplete.

[English Mock](images/native-shared-mock-en-390.png), [Chinese Mock](images/native-shared-mock-zh-390.png), [viewer](images/native-shared-viewer-en-390.png), [invitation](images/native-shared-invite-zh-390.png), [GraphQL](images/native-shared-graphql-en-1280.png).

## Evidence limits

The local environment lacks WebKit2GTK4.1 development files. The invoke harness uses the real native Router but is not an installed Tauri WebView. Installed GUI/file chooser, macOS/Windows installation and signing remain separate acceptance gates. Platform CI builds and Linux installed-package resource smoke must be checked for the release commit. No real EDAS sandbox was available. Docker builds run only in GitHub Actions.

Stable candidate0.2.0 local gates: frontend796/147, production38 chunks/typecheck, affected backend39 plus native route policy1, strict server Clippy and workflow lint pass. Rebuilt server/harness binary hashes matched installed QA copies; reconnect, real HTTP, canonical parameterized Mock201 and unchanged local/remote documents passed on those binaries. Temporary harness source is excluded from delivery. Exact release-tag CI and assets remain mandatory.
