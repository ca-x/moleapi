# Progenitor implementation patch

Source: crates.io progenitor-impl 0.15.0, https://github.com/oxidecomputer/progenitor.
Upstream license: MPL-2.0; existing file copyright notices retained.

The CLI emitter is extended (with crate-visible existing WebSocket metadata): finite raw success responses call an asynchronous
`CliConfig::success_raw` hook, and raw error responses return the original typed
SDK error instead of an emitted panic. Raw request bodies get a --body-file
argument consumed by the upstream SDK builder. All operation discovery, arguments, request
construction and response classification remain upstream. Upgrade responses call an async
`CliConfig::success_upgrade` hook with the existing WebSocket classification. Upgrade
errors retain the SDK error. The executable adapter uses Tokio duplex IO for generic
upgrades and tokio-tungstenite over the SDK upgraded connection for declared WebSockets;
incompatible declared/raw-success pagination contracts remain rejected. This emitter patch is used by the generator,
not required by generated projects. Generated CLI projects depend on tokio-tungstenite
for frame handling; they do not depend on this vendored implementation.


Raw-body metadata now retains the original requestBody.required flag. Optional raw
positional/builder methods preserve None and attach a Content-Type/body only for Some;
required raw behavior is retained; typed JSON/form presence support is described below. CLI --body-file follows
requiredness, uses read_to_string for text and sized ReaderStream IO for regular octet-stream files. Generated CLI and
positional SDK projects passed actual omitted/empty/provided binary/text wire cases;
existing raw/upgrade consumers passed. Text/WebSocket buffering and nonseekable input remain explicit limitations.


For recognized typed Dropshot pages, raw error branches retain the SDK error/nonzero
exit instead of todo. The CLI limit uses the exact schema-derived Clap type and positive
integer conversion, fixing an actual NonZeroU64/NonZeroU32 downcast panic. Unsupported
explicit pagination contracts reject before CLI rendering; no alternate page parser is
introduced. Two standalone typed-page limit variants passed real multi-page, limit and
first/later raw-error wire fixtures while preserving upstream lazy token streaming.


Binary CLI file bodies use a64KiB Tokio ReaderStream and a minimal http_body wrapper
with exact remaining size, retaining Content-Length. Opened-handle regular-file checks,
captured-length read caps and early EOF errors prevent growth append/short-body success.
A96MiB real upload preserved digest/length with observed Linux RSS below64MiB; helper
empty/growth/shrink/directory tests and all earlier standalone CLI consumers passed.
The wrapper supplies size/frame metadata to reqwest; request construction/transport stays
upstream. Text file input remains UTF8-buffered. Generated CLIs add http-body1/tokio-util0.7.


Request-body requiredness now also controls typed JSON/form SDK/CLI generation. Optional
positional arguments gain an outer Option; optional builders preserve None, explicit
body/body_map setters create Some and validate provided fields. The upstream RequestBuilder
applies JSON/form encoding only to Some, preserving omitted/empty/null distinctions.
CLI optional body properties/files do not become globally required. Required body fields
still validate on provided content. Generated CLI/positional SDK16-request fixtures plus
all previous standalone consumers passed; broader shapes/precedence/interactive cases stay open.


Generated body property flags conflict with the whole json-body file control using Clap;
query/path/header parameters remain separate. Body properties matching JSON control
names use the existing full-file path rather than duplicate flags. File-only/field-only,
non-body property collisions and reserved body keys passed8 real requests, while6
conflicting-file/field inputs rejected before reading missing paths or sending requests.
This closes silent whole-file replacement of field values; broader control namespaces
and template behavior remain open.


Operation API arguments that collide with json-body/json-body-template/body-file/help
receive deterministic repeated api- prefixes, avoiding all original API names. The
actual SDK setter/wire name is retained and help documents the original parameter.
Body help keys use full JSON files. The adapter disables auto help-subcommand generation
so an operationId help can execute while --help still displays usage. A standalone
3-request fixture and all prior consumers passed; broader parser/template modes remain open.


Offline json-body-template now uses supplied media/named/component/schema examples or
schema/model defaults and validates through the generated DTO plus jsonschema0.49.9
against upstream ToSchema components. Shared component text is emitted once. Named
example lookup extends the existing component resolver. No arbitrary-schema sampler,
network retrieval or alternate schema parser is introduced. Missing/invalid candidates
return errors instead of executing API requests. Exclusive operation parsing permits
required-argument-free offline output. The adapter checks template output before client
configuration; execution still requires an explicit URL. Generated Clap enables string
for the upstream dynamic enum parser. Offline and2 real file roundtrip cases plus all
previous generated consumers passed; arbitrary sampling/selection/platforms remain open.


Optional equals-style JSON template names now select supplied media examples explicitly,
with no fallback/download for missing/external/invalid entries. The default public helper
remains and body_template_named adds library selection. Wrapped named values preserve
null separately from missing entries; original default example order is retained. Internal
helper prefixes avoid an actual named operation collision with the new public helper.
An actual operationId string compile failure also required fully qualified emitted standard
String types. The generator depends on the separately documented openapiv32.2.0 explicit-null
Serde patch; generated clients do not. Named/null/default-priority and2 wire cases plus all
prior consumers passed. Arbitrary sampling/other standard-name/platform gaps remain open.


Builder namespace protection reuses Syn visit-mut on upstream generated builder tokens.
Standard type/variant and parent SDK support paths are qualified; lifetime operation impl
paths and public identifiers stay unchanged. SDK helper traits import as underscores to
avoid duplicate bindings with operation Error/ResponseValue/ByteStream/OperationInfo/etc.
The visitor does not inspect strings/docs or invent operations/types.13 standard/SDK-named
operation requests and a direct public-type consumer compiled/passed, with all earlier
runtime fixtures retained. Other hook/model/tag/identifier options remain separate scope.
