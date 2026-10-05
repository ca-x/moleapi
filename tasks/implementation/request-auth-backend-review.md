# Independent request authentication backend review — 2026-10-05

## Fix re-review of current source

**Final scoped review:** all definite findings recorded in this report are resolved in the current source snapshot. Generic copy screening now uses the workspace-wide `auth_private` gate, so No Auth requests also screen known API/JWT copies while retaining typed-file-body handling. The extended two-request regression source covers request B with `auth.kind=none`, no API settings, and a copied body value. No further definite important issue was found in this final focused source pass. This is scoped code-review approval of the reported corrections, not full authentication capability, live/platform verification, or parity approval. The parent's latest full-suite rerun is still running; this reviewer supplies no new execution claim. The previously noted RSA resource-bound question remains unverified, not an open confirmed finding.

### Latest focused pass

**Subsequent precise-fix pass:** the three paths reported in this subsection are resolved in current source. API and JWT credential sources share applicable collection/environment/native scope resolution; key_base64 capture no longer depends on a literal algorithm spelling. Captured usage also taints otherwise unmarked source variable values. Changed/withheld URLs use `{{redacted_url}}` rather than invalid empty strings, and generic screening exempts typed/dynamic selected-file source so the typed path preserves parts and `base64:null`. No definite new issue was found in those scoped decode/budget/reselection edits.

**Remaining P1 — copied credentials in other requests:** generic HTTP source screening is still gated by `request.auth.api_key.is_some() || request.auth.jwt.is_some()` on each individual request. The matcher contains credentials workspace-wide, but a separate No Auth request without those optional settings does not use it. A valid workspace with request A API key `actual-api-secret` and request B body `{"note":"actual-api-secret"}` or `X-Copy: actual-api-secret` therefore still exposes that known credential in default native/Postman export. This is the same copied-known-secret obligation across saved requests, not a cryptographic or speculative-encoding claim. Apply the workspace-wide auth-private gate/matcher to applicable request source surfaces while retaining typed-body and valid URL safeguards; cover two-request default/private roundtrips.

Parent reports fresh full tests and strict Clippy passing; this reviewer did not rerun them. Approval is limited to the precise fixes above; the cross-request copied-value case remains open, and full auth/platform parity is not claimed.

The previously reported ws/wss query failure, snippet duplicate query, literal copied HTTP credentials, and JWT key-template decode example are now fixed in current source. URL query preparation preserves the live scheme; snippet query rewriting uses the mature form codec without requiring a concrete base URL. JWT source keys resolve across saved collection/environment/native override scopes, and literal/decoded credentials now screen HTTP bodies/examples/value rows/scripts. Parent reports latest full tests and strict Clippy passing; this reviewer did not independently execute those commands. Three concrete privacy/source edges remain:

- **P1 — API-key templates lack credential scope capture.** The new scope-resolution loop exists only for JWT. An API key value `{{credential}}` sourced from an unmarked global/collection/environment variable named credential is not added as its resolved secret. Default native export still includes that variable's actual value and copied `{"note":"actual-api-secret"}`/public header values, although the dedicated API field is cleared. Use the same applicable scope capture for API values as JWT keys and verify a nonsecret-flag source variable is tainted by its actual auth usage.
- **P1 — Templated JWT algorithm skips decoded-key capture.** Both the direct HMAC decoder and the later captured-secret decoder still require the saved algorithm spelling to start with HS. For `algorithm="{{alg}}"` (alg resolves to HS256), `key_base64=true`, and a correctly captured encoded key, its decoded key is absent from the matcher. Copied decoded text in claims or HTTP body survives default export. Decode based on the resolved applicable algorithm, or conservatively capture bounded decoded forms whenever key_base64 is configured. Runtime resolved request capture is separate and already knows the concrete algorithm.
- **P2 — Privacy-budget withholding makes HTTP native backups invalid and can erase multipart source.** The new generic API/JWT copy-screening block runs before typed body redaction. When its matcher chooses withhold (e.g. a valid API key/private JWT key longer than 4096 bytes), `screen_generation_text(request.url)` returns empty, so native export/import fails concrete URL validation. For multipart, generic `screen_generation_json` returns `{}`; typed deserialization then produces an empty parts list, erasing active missing-file/reselection metadata rather than preserving it. Use a valid explicit withheld URL/source contract or fail export clearly; preserve typed file-part structure before generic screening. Add a large-credential native export/import fixture with an active multipart file.

These are source-established paths, not newly executed router results. No definite new live redirect/cryptographic panic was found in this focused pass. The RSA resource-bound question remains unverified and full authentication parity remains outside this slice.

The original dormant Digest, finite-request URL override, explicit-byte preparation, and unsupported external-export findings are resolved. Dedicated API/JWT credentials now enter the base matcher; literal Base64 HMAC forms decode for export and runtime capture; copied JWT kid/prefix/name are screened with bounded fallbacks. Existing JWT claims copying with a literal key is fixed. Parent reports core 7/formats 31 passing in `/tmp/moleapi-auth-regressions.log`; this reviewer did not rerun the tests. The following important issues remain or arise in the fixes:

1. **P1 — Templated decoded HMAC export gap.** Export decoding still calls Base64 decode on `jwt.key` itself and only when the source algorithm begins with HS. For `key="{{hmac}}"`, `key_base64=true`, HS256, and a secret environment variable hmac containing `c2lnbmluZy1zZWNyZXQ=`, the encoded variable is collected/cleared but the decoded `signing-secret` is never added. A copied decoded key in claims.sub survives default export. Runtime resolved-request capture fixes history for this example; native source export remains unprotected. Resolve credential templates against applicable saved scopes for privacy capture without signing, or conservatively withhold affected copied source when unresolved. Include templated algorithm spelling as well.
2. **P1 — Copied credentials in ordinary HTTP source remain exposed.** Workspace redaction's generic HTTP body/example handling still only calls `redact_embedded_json`; ordinary headers/query values are only cleared based on secret flag/key name. They do not use the new known-credential matcher. An API-key value `api-secret` copied into body `{"note":"api-secret"}`, example body, or public-looking `X-Note` header survives default native/Postman export while auth.api_key.value is cleared. The same holds for JWT signing keys. Typed multipart and JWT claims source now screen copies, but ordinary HTTP source does not. The authority explicitly calls for copied-known-secret screening. Apply the complete matcher to these concrete source surfaces with valid bounds/schema retention, and add fixtures for an innocently named copy.
3. **P2 — Query placement now rejects live ws/wss URLs.** `prepare_authentication` uses `crate::valid_url(&r.url)` to remove matching URL parameters. That helper accepts only HTTP/HTTPS (`core/src/policy.rs:102-108`). `server::execution::prepare_live` materializes authentication before the protocol URL adapter normalizes WebSocket schemes. Thus an otherwise supported `wss://example.test/socket` request with query API key/JWT fails with `Only HTTP and HTTPS are supported`. Header placement is unaffected. Use protocol-aware checked URL validation, preserve the original live scheme, and test actual supported live query handshakes.
4. **P2 — Snippet URL-query override remains unfixed.** `crates/generation/src/request.rs` still replaces only RequestSpec.query rows. Its URL starts as the original string, so a same-name URL query parameter remains before the selected auth row. Runtime finite requests now remove it correctly, but generated API-key snippets still send stale+selected duplicate credentials. Cover a concrete same-name URL query in a generated snippet; retain static templated URL support explicitly rather than invoking runtime signing.

No definite panic/arithmetic regression was identified in the inspected corrections. The RSA resource-bound question below remains unverified; this source pass supplies no hostile-key benchmark or new live/platform evidence.

Read-only review against `docs/specs/request-authentication.md`, uncommitted slice at base `485dc8f89accf9ead5ba713619bcbf974a20eba4`. Only this report was written; no source edits, commits, Docker, subagents or new execution claims.

## Definite important findings

### P1 — Default export misses copied API/JWT secrets in JWT claims and metadata

Locations: `crates/formats/src/redact.rs:28-40`; `ExportPrivacy::from_workspace`, API/JWT collection inside `if generation`.

Workspace redaction clears the dedicated fields but screens JWT claims with `privacy = ExportPrivacy::new(source)` (generation=false). API/JWT keys are only added to the secrets set in the generation=true branch, so this matcher never knows those dedicated keys. A JWT configuration with key `signing-secret` and claims `{"sub":"signing-secret"}` exports the original secret in claims.sub after clearing jwt.key. `kid`, `name`, `prefix`, and `algorithm` also remain wholly unscreened; copying the signing key into kid survives independently of the matcher bug. A copied API key in JWT claims has the same issue.

Correction: collect dedicated credentials in the shared base secret set, screen source claims and bounded metadata through the complete matcher, and verify default native output excludes copied known credentials while explicit private export preserves originals. Do not sign while exporting.

### P1 — Decoded Base64 HMAC keys are absent from export/history privacy capture

Locations: `crates/formats/src/redact.rs`, JWT credential capture; `crates/server/src/privacy.rs:249-251`; `crates/core/src/authentication.rs`, `EncodingKey::from_base64_secret` branch.

With `key_base64=true`, signing uses decoded HMAC key bytes, but privacy only captures jwt.key's encoded source spelling. Neither helper decodes the key and adds its actual UTF-8 form. Existing pattern generation encodes captured strings; it does not derive the decoded HMAC key. For key `c2lnbmluZy1zZWNyZXQ=` and copied text `signing-secret` in a multipart field/JWT claim/example/server response, default export or history can retain the actual signing secret even while withholding the encoded field and generated token.

Correction: capture bounded decoded HMAC forms using the mature Base64 codec (including currently resolved template values). Cover raw and decoded spelling in native export and real response/history fixtures. Arbitrary binary keys need an explicit withholding policy for opaque representations; do not claim universal arbitrary-encoding screening.

### P2 — Digest mode rejects dormant optional credentials preserved by mode switches

Location: `crates/core/src/authentication.rs:136-140`; `web/src/features/authentication/RequestAuthEditor.tsx:12`.

Unlike the other modes, Digest requires both optional api_key and jwt settings to be None. The editor changes auth.kind while retaining both existing optional drafts, so choosing Digest after configuring API Key or JWT creates a request that validation rejects before save/execution. Interpolation already intentionally restores dormant optional settings without resolving them; Digest contradicts that mechanism.

Correction: validate only active Digest username/password semantics and bounded dormant drafts, or explicitly clear unrelated drafts when selecting Digest with settled product behavior. Verify API Key/JWT -> Digest -> previous-mode transitions.

### P2 — Selected query authentication does not override same-name URL parameters

Location: `crates/core/src/authentication.rs`, `prepare_authentication` rows.retain/push; `crates/core/src/transport.rs:61-67`; `crates/generation/src/request.rs`, API-key query mapping.

Placement replaces matching `RequestSpec.query` rows but leaves the URL's existing query untouched. URL `https://example.test/?access=stale` plus selected query API key `name=access,value=secret` sends `access=stale&access=secret`. A server that consumes the first value authenticates with stale rather than the selected key. JWT query placement and API-key generated snippets have the same split representation problem. Current loopback test only covers a conflicting row, not a parameter embedded in the URL.

Correction: replace the chosen decoded parameter across URL and query rows, preserving unrelated/repeated query values and fragment semantics. Add actual wire and snippet fixtures with a same-name URL query parameter.

### P2 — Explicit-byte execution omits API/JWT query authentication

Location: `crates/core/src/transport.rs:38-52`, `execute_inner` and `request_headers`.

`execute()` materializes authentication into the request before URL/query preparation. `execute_bytes()` calls execute_inner with the unmaterialized request. request_headers materializes a temporary clone, which makes header placement work, but its selected query row is discarded. Consequently execute_bytes with query API key/JWT sends the raw bytes without the configured credential. Header signing also lacks the generated-request privacy behavior of the ordinary path.

Correction: prepare auth once before explicit-byte execution, retain the exact raw-body override, and test both placements through this public checked transport. Existing webhook replay is not evidence for authenticated raw-byte behavior because its replay requests use their existing concrete auth marker.

### P2 — External exports silently lose new authentication settings

Locations: `crates/formats/src/lib.rs:67-80`; `crates/formats/src/openapi.rs:267-318`; `crates/formats/src/postman.rs:340-343`.

OpenAPI export accepts API-key authentication, but its exporter only emits stored query/header rows and emits no security scheme or selected API-key placement. The new typed provider is therefore omitted completely rather than mapped or rejected. A saved dynamic auth kind `{{auth_mode}}` resolving to JWT/Digest also bypasses the literal-kind external rejection and Postman's match falls back to noauth. These are silent source/semantic losses contrary to the authority requirement that external mappings be verified or explicitly rejected.

Correction: implement verified OpenAPI API-key security mapping or reject it, and reject unsupported dynamic external auth kinds before conversion. Cover export/reimport semantics rather than merely successful serialization.

## Covered surfaces and evidence limits

Inspected typed configuration, active/dormant interpolation, JWT claims/key/algorithm/TTL/output limits, mature JSON/Base64/PEM/signing paths, Digest challenge/retry/body-target construction, cross-origin secret header/query stripping and Digest disabling, server live/finite materialization and ephemeral answer capture, history privacy, native/Postman/OpenAPI source exports and static snippet mapping.

The mature JWT algorithm parser rejects unsigned/unknown algorithms; signing dispatch explicitly selects supported families. Claims resolve structurally with depth and byte budgets. Digest retries remain inside the transport deadline and hash the retained exact body snapshot and URL request target. Cross-origin redirects remove declared selected secret rows/header credentials and disable future Digest retries. These source observations do not prove every cryptographic-resource or redirect-renaming case.

One remaining verification question, not a confirmed finding: JWT signing is synchronous and occurs before the HTTP deadline; the resolved RustCrypto RSA private-key decoder does not use the public-key constructor's default 4096-bit modulus bound. The adapter's PEM byte limit is finite, but accepted modulus/exponent sizes and worst-case signing work need explicit evidence before describing CPU/deadline bounds as proven. This review did not benchmark hostile-size keys.

Parent reports all advertised signing algorithms verified with mature decoders and real API/Digest/JWT loopback fixtures passing. This reviewer inspected relevant fixture source but did not independently execute tests. Broad Rust/browser/native/platform evidence remains separate; OAuth/inheritance and the rest of the full authentication matrix remain unfinished as stated in the spec.
