# Independent request-auth frontend review

Reviewed `RequestAuthEditor`, its RequestEditor integration, optional shared auth DTOs and English/Chinese labels against base `485dc8f` and `docs/specs/request-authentication.md`. Read-only source review; no Docker/delegation or root QA changes. This covers the current API Key/JWT/Digest slice, not the unfinished auth module or parent goal.

## Important findings

1. **P1 — Switching API Key/JWT to Digest creates a configuration that cannot save or execute.** `web/src/features/authentication/RequestAuthEditor.tsx:12` spreads the entire existing auth when changing kind, leaving `api_key` and/or `jwt` present. Core `validate_authentication` (`crates/core/src/authentication.rs:136–140`) rejects Digest whenever either optional configuration is present, even during draft validation. A normal configure-API-Key → choose-Digest workflow therefore returns `Digest settings cannot contain unrelated credentials`; choosing No Auth before Digest does not help because the retained settings are still spread. Align the mode-switch/source-retention policy with backend validation, preserving exact inactive drafts where appropriate without passing an invalid active Digest configuration. Add real editor mode-switch coverage and a validator/save acceptance check.

2. **P2 — One “advanced” flag applies the wrong protocol eligibility to all three new auth kinds.** `RequestEditor.tsx:448` disables advanced auth for SSE/WebSocket/gRPC/Socket.IO/A2A and all Data sources. Core validation allows API Key/JWT for supported HTTP handshakes and Data remote_file, and shared execution prepares these placements, so their UI cannot configure supported auth. Conversely GraphQL subscriptions keep the flag true and offer Digest, although core rejects Digest for subscriptions at execution (`validation.rs:138–155`). Replace the combined flag with per-kind/placement eligibility: API Key/JWT for checked HTTP transports, Digest only finite HTTP, and correct header/query placement restrictions where necessary. Test HTTP, HTTP-handshake, remote-file, non-HTTP and GraphQL-subscription cases without advertising unsupported paths.

Static call-path findings; no browser or cryptographic correctness approval is claimed. The new editor has no focused tests at this snapshot.

## Checks and limits

- JWT editor keeps the exact original claims source and key as user input; it does not parse/rewrite/sign them during rendering or mode selection. Supported algorithm options match the explicit backend signing families, with no unsigned algorithm option or filesystem loading UI.
- Text inputs support key/name/claims templates; claims use the existing labeled JSON CodeMirror editor and localized configuration. There are no new asynchronous picker/network callbacks in this editor; changes operate directly on current props.
- Existing Basic/Bearer inputs remain present; optional config DTOs preserve old JSON compatibility. RSA/EC/EdDSA PEM editing remains multiline and bounded.
- Fresh independent frontend suite: **147 passed across44 files**, exit0. Typecheck: exit0. These gates do not cover the two new-editor findings above.
- Actual crypto/Digest/API transport fixtures are backend-owned, embedded browser QA/build is root-owned, and locale/owner/native end-to-end success is not inferred from static inspection.

Resolve the two findings and add focused regression evidence before frontend approval.

## Focused fix re-review

Finding1 is resolved by permitting bounded inactive API Key/JWT settings for Digest without resolving/signing them; exact dormant key/claims drafts remain preserved. The new core regression explicitly covers a Digest request with inactive unresolved credentials and invalid dormant JWT JSON. This reviewer inspected that test and the validation change; no new core-test run is claimed here.

Finding2's per-kind eligibility is corrected: API Key/JWT are offered for checked HTTP handshakes and remote-file Data; Digest is restricted to finite HTTP/Soap/GraphQL query/mutation. GraphQL operation selection uses mature `parse/getOperationAST`, safely excluding invalid/ambiguous selections. Fresh independent focused frontend tests pass **3/3**, exit0.

The original placement portion of finding2 still requires correction: gRPC now exposes API Key/JWT while their location dropdowns continue offering `query`. Core validation forbids enabled gRPC query parameters, and its channel requires an origin without a query. Make placement eligibility header-only for gRPC; preserve imported incompatible originals with explicit validation rather than silently changing them. Add focused header/query eligibility coverage. Root was notified. No other definite important issue found within this scoped fix review.

### Final scoped approval

**Both findings are resolved; scoped frontend review approved.** Per-protocol eligibility now passes a separate query-placement flag, and both API Key/JWT location dropdowns exclude query for gRPC. Core rejects selected incompatible query authentication with a metadata-header error while preserving originals. The actual Radix mode-switch regression preserves the API Key draft when entering Digest and renders its username controls.

Fresh independent focused frontend run: **4 tests passed across2 files**, exit0, covering handshake/Digest/query placement, selected GraphQL operation, remote/local Data eligibility and the real editor mode switch. No definite important issue remains in this reviewed fix scope. Backend cryptographic/transport gates and final full-suite/browser/runtime verification remain root-owned; advanced auth/full goal completion is not implied.
