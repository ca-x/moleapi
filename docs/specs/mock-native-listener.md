# Explicit native public Mock listener

Authority: `mock-011`, `docs/specs/mock-public-access.md`, the offline Tauri IPC router, and the explicit native Webhook listener lifecycle. Hosted public Mock behavior and capability semantics remain the shared contract.

## Objective

An offline desktop workspace can deliberately start a local HTTP listener and expose its saved public Mock shares to same-device clients or, after a separate explicit choice, LAN clients. Starting the desktop app, opening a workspace or creating a share never starts a socket. Stopping the listener ends new and in-flight public calls without stopping the desktop IPC router.

## Contract

- The trusted local IPC API adds `GET /api/mock-public/listener`, `POST /api/mock-public/listener/start` with `{host,port,allow_lan}`, and `POST /api/mock-public/listener/stop`. Only local mode accepts these controls. Default host is `127.0.0.1`, port 0 chooses an available port. `0.0.0.0` requires an explicit `allow_lan:true`; other bind hosts reject. The returned origin uses `127.0.0.1` plus the actual port for same-device calls, and reports the actual bind separately. LAN users replace the host with their device address and must provide HTTPS through a trusted proxy outside loopback.
- The listening router contains only the existing public Mock direct and routed paths. It never exposes workspaces, share management, auth, sync, imports, embedded assets or the desktop IPC route. Capability checks, request binding, dynamic opt-in, response budgets, logs, expiry and cross-instance revocation are exactly the hosted rules. The local owner is the offline workspace identity used by the IPC router.
- Local IPC may create/list/rotate/revoke Mock shares without a hosted login session. A network caller cannot reach those management paths. The desktop editor shows listener status/start/stop and the resulting origin; public address creation is disabled while stopped, but revocation remains available. A port-0 restart may choose a different address, so the editor explains that an old copied URL may need a new listener origin or token rotation. Token display remains one-time.
- Stopping cancels active public request futures, including configured delay or script work, then gracefully shuts down the socket with a bounded wait and abort fallback. Listener startup, stop and app/router drop release the bind. The background task cannot own the strong lifecycle hub that owns its handle; use a weak reference in `AppState` and a strong outer-router lifetime. A failed bind changes no listener state.
- Listener responses keep `no-store`, no-referrer and credential-free CORS behavior. Raw share tokens remain absent from server logs and workspace backups. The local listener is plain HTTP; LAN use is an explicit opt-in with a visible HTTPS warning, never implied secure by the UI.

## Acceptance

- Real native `local_with_worker` router plus loopback TCP fixture: start/status, anonymous direct/routed call, no management route on the listening socket, stop/cancellation, occupied-port failure, repeated start, restart, LAN opt-in denial/acceptance and workspace/share revoke. Tests verify that dropping the local router ends the listener.
- Bilingual 390px frontend controls show status, actual origin, LAN/HTTPS warning, one-time URL, rotation and stop. Hosted UI remains unchanged. Local scripts and delayed public calls still use shared budgets.
- `cargo test --locked -p moleapi-server --test mock_native`, relevant existing Mock/database tests, frontend tests, strict Clippy/format, production frontend and embedded server builds, current-head three-database/Docker CI and installed Linux/macOS/Windows packages establish the slice gates.

## Boundaries

Do not open a socket at startup, reuse the account-authenticated `/api/mock` route as a public endpoint, expose the local IPC router over TCP, trust a proxy-provided origin, or persist a raw capability. This listener does not add TLS termination or device discovery; installed deployment instructions must make that limit clear. Organization permissions, custom public domains and unkeyed public servers remain outside this slice.
