# OpenAPI project generation coverage

Project generation is implemented separately from request snippets. It uses Progenitor0.15.0 for native Rust3.0 clients and a SHA256-pinned, embedded, unmodified OpenAPI Generator7.26.0 JAR for upstream clients/server frameworks. Documented compatibility templates fix ASP.NET Core authentication/authorization middleware, JavaScript binary response parsing and C# generichost exploded form arrays; explicit user templates take precedence. Hosted API and offline desktop IPC use the same service. No target program or package publishing runs during preview/download.

Data models have a separate category covering Apifox's19 language families with pinned Quicktype26.0.0 (18 targets) and MySQL/PostgreSQL DDL. See [MODEL-GENERATION-COVERAGE.md](MODEL-GENERATION-COVERAGE.md) for the exact selection list and compile/runtime evidence; model generation does not complete SDK/server/custom-template coverage.

Explicit Mustache overrides now reuse the embedded OpenAPI Generator and mature Rust Mustache parser. Directory/ZIP/JSON/snapshot import, template/output editing, all seven upstream output roles, static text/binary assets, owner/privacy fences and preserved source/checksums are implemented. See [TEMPLATE-GENERATION-COVERAGE.md](TEMPLATE-GENERATION-COVERAGE.md); broader dialect/target compatibility remains open.

| Target | Current evidence | Limits |
| --- | --- | --- |
| rust-progenitor | Generated3.0 reference/nullable model SDK compiled; async client called a real HTTP fixture; owned API/default private-example screening preserved original source | Native engine supports3.0; broader union/auth/file SDK fixtures remain required; generated CLI evidence is separate below |
| rust-progenitor-cli | Generated3.0 CLI projects compiled/called real core, binary, optional body, upgraded/WebSocket, typed pagination and96MiB streamed-file fixtures; owned capped worker/privacy gates passed | Native3.0 only; other pagination conventions, interactive CLI, broader body/protocol/platform behavior remain required |
| typescript-fetch | Generated3.0/3.1 npm projects compiled for CommonJS/ESM; real GET/POST/Bearer/query/header/nullable/enum/error fixtures passed | Broader union/file/dynamic-keyword and library-option compatibility remains required |
| go-server | Generated skeleton compiled after `go mod tidy`; generated router/service started on a controlled loopback test harness and returned its expected501 business-handler placeholder | A skeleton intentionally needs application handler implementations; dependency installation is a consumer build step |
| javascript, python, go, csharp | Generated3.0/3.1 clients installed/built and passed real GET/POST/Bearer/query/header/nullable/enum/error fixtures; Go also passed race checking; C# default generichost hosting/token/typed400 APIs were called | Broader union/file/dynamic-keyword and library-option compatibility remains required |
| php | Default Guzzle3.0/3.1 core, binary/multipart and mapped Cat/Dog tagged-oneOf decode/echo passed; original wire discriminator names retained | Static parent annotations/inheritance, advanced unions/constraints/files, other libraries/platforms remain required |
| dart | Default HTTP3.0/3.1 core/file and mapped Cat/Dog oneOf wrappers/array echoes passed with analysis/AOT | Untagged/advanced unions, files/abort/options, SDK ranges and platforms remain required |
| swift6 | Default URLSession3.0/3.1 release packages and core/binary/multipart/mapped tagged-oneOf calls passed on Linux | Apple/other SDK platforms, advanced files/unions, encoding/options remain required |
| python-fastapi | Generated3.0/3.1 packages installed; unchanged generated app started, exposed routes/OpenAPI and returned its explicit500 missing-business-handler response | Authentication/business implementations remain consumer work; broader framework/keyword compatibility remains required |
| aspnetcore | Default .NET10 projects built and started for3.0/3.1; consumer-configured authentication reached generated sample handlers (200) and rejected anonymous requests (401) | Real authentication scheme/validation and business handlers remain consumer work; broader framework/keyword compatibility remains required |
| kotlin | Default jvm-okhttp4 Gradle3.0/3.1 core/binary/multipart fixtures passed; explicit Retrofit/Gson and Retrofit/kotlinx typed tagged-oneOf cases passed both dialects | Default Moshi unions, advanced union shapes, other libraries/platforms and broader keyword/file coverage remain required |
| java | Default okhttp-gson3.0/3.1 SDKs compiled and called core, raw binary, multipart array/file and referenced tagged-union fixtures | Broader/nested/custom/streaming/advanced keyword/options runtime remains required |
| spring | Default spring-boot3.0/3.1 executable projects compiled/started; generated OpenAPI/routes and501 placeholder responded | Consumer business/security implementations and broader framework/keyword compatibility remain required |
| Other upstream targets | Pinned CLI catalog and captured naming/library defaults | Catalog availability is not a MoleAPI support claim |

The upstream catalog contains165 non-deprecated targets across client/server/schema/docs/config categories. The UI currently selects client/server projects; native Rust is a separate entry. Swift's current non-deprecated target is swift6, not the older swift5 ID. Naming/library inputs are checked against actual target option catalogs, not silently ignored. Native Rust package/interface fields and multi-language package/namespace/library/version fields are supported where upstream exposes them.

Release builds run `python scripts/prepare_openapi_generator.py` to fetch and verify the pinned build asset, then embed it. Runtime never downloads an engine. Deployment/native generation uses an explicitly configured Java17+ path via `MOLEAPI_CODEGEN_JAVA`; there is no PATH discovery. Java/JRE installation/bundling/platform distribution remains required deployment work, while native Rust requires no Java. Capture the upstream defaults with `node scripts/capture_codegen_catalog.cjs /absolute/path/to/java` after frontend dependencies are installed.

Artifacts contain exact generated file content, byte counts, executable flags and SHA256. `moleapi-generation.json` records engine, source digest, effective options, private-inclusion mode and generated-file hashes; ZIP preserves safe executable permissions. No authored files are automatically overwritten. Worker/process/file/reply budgets, local-reference checks, symlink/path escape rejection, explicit cancellation/kill/reap and logout/workspace deletion fences apply. Default generation screens credential examples, scoped copies and naming-option copies; explicit private inclusion preserves original selected source. Source definitions remain unchanged.

Actual embedded-binary UI was exercised in English/Chinese and at390px without horizontal viewport overflow. Owner/content/open fences discard late results, including identical close/reopen state. Files can be previewed, copied and downloaded individually or as a ZIP with browser/native file pickers.

Remaining complete-module work includes broader per-language compile/run fixtures,3.1/reference/union/nullable/auth/file coverage, broader CLI/upgrade-response coverage, protobuf stubs, advanced source-bundle references, custom templates, native platform picker evidence and runtime/platform packaging. The full395-feature objective remains open; this is not a complete-parity release.

## Native generated CLI and regeneration

`rust-progenitor-cli` uses Progenitor’s builder SDK and clap CLI emitter, not a custom operation/argument emitter. A small executable adapter supplies base URL/Bearer configuration and JSON output. Generated CLI projects compiled and called real HTTP fixtures for GET and POST with path/query/body parameters. CLI help exposes operation arguments, whole JSON-body files and nonzero API-error behavior. Finite raw responses now use a small, MPL-preserving Progenitor emitter patch (see `vendor/progenitor-impl/MOLEAPI-PATCH.md`): success bytes stream unchanged to stdout or `--output-file`, which uses create-new semantics and refuses overwrite. Raw error responses retain the SDK error and exit nonzero. `--body-file` streams regular binary files with known length into the upstream builder; validated UTF8 text files remain buffered. A generated standalone project compiled and passed real HTTP byte-identical stdout/download/upload, existing-file preservation and raw HTTP400-error checks. Declared raw success pages/incompatible Dropshot pagination remain unsupported and reject during CLI generation; typed-page raw errors are supported by the verified pagination slice below; upgraded CLI connections now have the separately verified transport slice below; optional raw request-body requiredness and text input are repaired in the separately verified slice below. These gaps remain full-module work.

Save generated snapshot JSON from the project dialog. Regeneration compares the original snapshot, current edited snapshot and next generated files using diffy0.5.2 in a capped, cancellable application worker. Original/generated hashes, duplicate paths, file/aggregate/line/diff limits and owner/workspace admission are checked. Nonoverlapping changes merge, unchanged local files update, edited/deleted/authored files are preserved or become explicit conflicts. Conflicts block ZIP download until the user chooses working/generated/deletion; no filesystem overwrite occurs. Merged ZIPs include actual merged-content checksums in `moleapi-regeneration.json` and preserve safe executable flags. Snapshot import, conflict-choice and archive gating have frontend/API tests. Original `moleapi-generation.json` describes generator output; the regeneration manifest describes the edited/merged result.

## Native protobuf/gRPC projects

`rust-tonic` accepts saved protobuf source bundles (including bundled imports) or descriptor sets. The shared protox/prost-reflect compiler validates and resolves only provided sources/well-known descriptors; tonic-prost-build0.14.6 owns Rust messages and generated client/server interfaces via `compile_fds`, without invoking protoc or loading imports from the host filesystem. Generation runs in the capped application worker and existing owner-bound cancellation service. Original bundles remain unchanged, private source copies/options are screened by default, and generated artifacts include descriptor bytes/Base64, source bundle and normal hash/ZIP manifest.

Actual generated Rust project compilation and client/server unary plus server-streaming roundtrips passed. Bundled multi-file imports, descriptor re-generation, missing/traversal imports and the saved-spec API route have necessary fixtures. Both hosted API and native IPC share this route; the specification page exposes protobuf generation. Other language protobuf plugins, full4-mode representative target validation and runtime/platform distribution remain required; native Rust evidence is not relabeled as full language coverage.

## Explicit protoc and gRPC language plugins

The project generator now also accepts explicitly configured official protoc (`MOLEAPI_CODEGEN_PROTOC`) for C++, C#, Java, Kotlin, Objective-C, PHP, Python and Ruby message types. The shared Rust compiler validates provided sources and imports first, then protoc reads only an isolated descriptor set. No compiler/plugin PATH search, runtime download or imported executable selection occurs. Actual protoc36.2 generation passed for all8 built-in language targets; this does not establish all-language compilation/runtime coverage.

Optional explicit `MOLEAPI_CODEGEN_GRPC_<LANGUAGE>` executables generate service interfaces. If the plugin path equals the compiler path, an explicitly selected compiler with an integrated gRPC generator is used (for example official grpcio-tools’ `python-grpc-tools-protoc`). The catalog/UI separately reports compiler presence and configured plugins, distinguishing message-only output from gRPC service output. Official grpcio-tools generated Python messages plus client/server interfaces and completed a real Python gRPC roundtrip. Other language plugins and broader target build/call cases remain required.

Compiler/plugin processes use mature process-wrap process groups/Windows jobs; cancellation/deadlines/file quotas kill/reap the group without fallback. Per-project file/hash/ZIP checks are reused, and the compiler’s actual version is recorded in engine metadata. A process-cancellation fixture validates cleanup. Dependency configuration is local/admin-only, never accepted from imported source/request options. Configuration must be installed/provided for standalone/native distribution until packaging bundles it.

## Project directory and ZIP inputs

The regeneration dialog also reads untouched generated ZIPs as baselines and edited directories/ZIPs as current files through the shared owner/workspace-bound `/api/generation/projects/import` route. Rust rawzip0.5.2 iterates all central-directory entries before zip4 decompression, preventing duplicate entries from disappearing into zip's filename index. Parsing runs in the existing heap/time/output-capped cancellable application worker and never extracts an archive or reads imported paths from the host filesystem. Original ZIP hashes/sizes/executable modes and complete manifest file sets must match; edited bytes get fresh hashes. Binary files and safe executable flags are preserved, and regeneration metadata is removed from working input so merged archives can be imported again.

Browser directory selection removes the chosen root and preserves known baseline executable modes; native dialog selection uses scoped recursive traversal, dotfile support and bounded regular-file reads with handle cleanup. Directory imports skip the documented VCS/dependency/build directories; ZIP imports retain authored content. New/missing files remain authored/deleted, and multiple conflict choices can be recorded before recomputing the merge. Closing/reopening, changing the source/account/workspace or canceling discards stale imports/results. Exact file/expanded/archive/count limits are in specs/code-generation.md; comparison retains its existing combined-input budget.

Necessary evidence: original/wrapped ZIP roundtrips, binary/mode preservation, modified-baseline rejection, duplicate/traversal/collision/link/expanded-size rejection; shared service import-to-merge with added/deleted files and hosted cross-owner/anonymous rejection; browser/native picker byte/traversal/cleanup mocks and stale UI/cancel fixtures. Real production frontend plus local Rust server exercised original ZIP, edited ZIP and directory imports, preserved source edits/authored files and deletions, English/Chinese and light/dark layouts at1360px and390px. Native OS picker execution remains unverified on actual platforms; the source, capability and mock evidence does not replace it.

## Explicit OpenAPI source bundles

The saved `moleapi-openapi-source-v1` envelope contains entry_file and JSON/YAML files. schematools1.1.0 owns reference absolutization/dereferencing/cycle handling and internal-reference creation; its vendored patch adds parsed-document/storage constructors, exposes upstream first-use locations for operation links, preserves repeated-reference annotations and corrects optional codegen/HTTP dependencies. The application never calls its resource loaders. URI membership/pointer/size/depth/node checks and assembly run in the capped/cancellable worker under the existing owned generation route. Common relative file refs, external path items and declared recursive model aliases work. Components resolve before first-use references so native SDK model pointers remain under components; literal payload data is preserved. Discriminator file mappings normalize through declared or promoted component aliases, preventing unresolved external mapping URLs from reaching emitters.

All parsed source documents participate in default semantic/scoped credential screening, including unused password examples copied into naming options. The reserved projection field is removed before code emission. Original sources stay byte-for-byte in the workspace; explicit private backup preserves them, while default native export withholds opaque bundle source. API tests verified default hiding, explicit inclusion, unused-source option rejection and exact original storage. A generated Rust SDK from a real four-file definition compiled and called a real HTTP fixture. Directory/ZIP UI import, entry selection, canonical save and generation ran in the production frontend with the local Rust server; English/light and Chinese/dark README screenshots,390px modal bounds and scoped accessibility checks were exercised. Native platform picker execution remains separately unverified.

Current required gaps include3.1 dynamic anchors, per-target constraint handling and broader union/target compile/run cases. This source-bundle slice does not establish full3.1 or full Apifox/Postman parity.

Source-local/existing included-document operation links relocate to their assembled operation pointers; missing operation targets reject instead of becoming dangling links. Repeated model references keep individual summary/description annotations. Targeted source-link/annotation cases passed. The user requires feature implementation before further formal review; independent review is deferred until functional completion.

## Encoded URI-fragment pointers

The source-bundle adapter and schematools resolver now decode percent-encoded UTF8 exactly once for JSON Pointer lookup, validate percent/tilde escapes, canonicalize equivalent encodings and emit encoded internal/operation links. Unicode, spaces, slash/tilde keys and literal percent sequences resolve without rewriting canonical source files; malformed/invalid UTF8 and missing targets fail before resolver traversal. All eight source-bundle cases and five owned project API cases passed, including native SDK token generation, worker execution, private-example screening and original-source storage. This is resolver and native artifact evidence; broader per-target generated SDK compilation/runtime and resource/dynamic-anchor semantics remain separate gates.


## OpenAPI 3.1 static schema resources

Explicit 3.1 source bundles use referencing0.49.9 with a non-fetching retriever for supplied `$id` resources and static `$anchor` declarations. Nested relative IDs, retrieval/declared-HTTPS aliases, plain and encoded anchors, recursion, and encoded JSON Pointers resolve to original-file pointers before schematools bundles them. OpenAPI schema roots are registered through a bounded temporary projection; resource metadata is removed only from generation copies. JSON Schema2020-12 and the OpenAPI3.1 base dialect are accepted; unknown dialects, ambiguous duplicate identities, malformed IDs/anchors, missing resources and dynamic/recursive keywords reject explicitly. Literal examples/defaults/extensions and original privacy documents remain intact. Legacy `definitions` remains usable for ordinary pointers; new resource declarations must occur under recognized2020-12 schema keywords such as `$defs`.

The generation regression passed48 tests (6 configured external-tool tests ignored). All6 owned project API tests passed, including a saved3.1 resource bundle producing Go models through the capped worker, exact original-source storage, private-example exclusion, unused-source secret-copy rejection and explicit private inclusion. Scoped generation/server strict Clippy and formatting passed. This establishes static resource resolution and artifact generation; it does not establish Go runtime parity or full3.1 support. Progenitor still explicitly accepts3.0 only. Dynamic anchors/references and broader per-language keyword/compile/run evidence remain required slices. Schema reference siblings in explicit3.1 bundles are covered by the continuation below.


## OpenAPI 3.1 schema reference siblings

Recognized2020-12 schema references in explicit3.1 source bundles now retain validation siblings through an appended `allOf` reference branch. All authored sibling keywords stay on the outer schema, preserving the referenced branch's evaluated-property/item annotations for outer `unevaluatedProperties` and `unevaluatedItems`. Existing `allOf` indices remain stable. This uses the same schema-node registry and mature dereferencer, without replacing their graph/cycle logic. Non-schema Reference Objects and3.0 bundle siblings retain explicit rejection of unsupported siblings; ordinary saved3.1 sources now reuse this projection through the single-source worker below. Ordinary3.0 direct parsing and each emitter's keyword limits remain distinct.

Pinned jsonschema0.49.9 instance-validation comparisons proved the bundled projection preserves string length/pattern/type constraints, required properties, outer unevaluated-property/item checks, recursive model constraints and boolean targets. Pointer-index, malformed-allOf, annotation/literal and original-document cases passed. The full generation suite passed55 cases (6 configured external-tool cases ignored), and all7 owned project API cases passed, including Go model artifacts retaining sibling-authored fields plus source/privacy checks. Scoped generation/server strict Clippy and format/diff checks passed. This is resolver/validator and artifact evidence, not complete runtime constraint enforcement across all generators. Dynamic-reference semantics and per-language compile/run gates remain open.


## Reproducible mainstream SDK and server runtime fixtures

`tools/projects/verify-runtime.py` generates trusted fixtures through the shared `ProjectRuntime` and application executable, then performs consumer installation/build and real loopback calls. JavaScript, Python, Go and TypeScript each passed both3.0.3 and3.1.0 definitions:24 actual requests across8 client cases preserved method/path, encoded Unicode query, custom header, Bearer token, quoted Unicode JSON, referenced enum/nullable models and HTTP400 errors. Go tests also passed `-race`. FastAPI installed and started for both dialects, exposed generated OpenAPI/routes and returned the expected500 `Not implemented` before a business subclass exists. All10 target/dialect cases passed; their catalog validation labels now describe this representative evidence.

Exact local tools: OpenAPI Generator7.26.0, Temurin JRE21.0.12.1, Node26.11.1/npm12.2.0, Python3.14.7 and Go1.27.1 on Linux. Consumer dependencies included TypeScript5.9.3, Babel7.29.7/Superagent5.3.1, Pydantic2.14.1/urllib3 2.8.0 and FastAPI0.143.0/Uvicorn0.54.0. The harness preserves package locks/dependency freezes, tool metadata, engine/source digests and per-case evidence in isolated output. See `tools/projects/README.md` for explicit application/Java configuration and subset reruns. This does not add those runtimes to product preview/download. Other ecosystems, full file/union/dynamic semantics, server business/security handlers and broad target options remain required work.


## Default C# and ASP.NET Core runtime fixtures

The runtime harness now also validates the default C# `generichost` SDK and ASP.NET Core .NET10 skeleton for3.0.3 and3.1.0. Both C# projects built and a separate generated-API consumer completed the shared GET/POST/Bearer/query/header/enum/nullable/quoted-Unicode fixture plus typed HTTP400 handling. Generated files remained unchanged. Both server projects built/started and exposed their generated OpenAPI contract; a separate consumer host configured a test authentication scheme and observed200 generated sample-handler output and401 anonymous rejection. All4 new target/dialect cases passed, bringing representative mainstream evidence to14 combinations across5 client and2 server targets.

A real server failure exposed an upstream Startup template missing authorization middleware. A pinned, Apache-2.0 compatibility copy now registers services and orders `UseAuthentication`/`UseAuthorization` between routing and endpoint execution, through the mature template renderer. No permissive scheme/key/business handler is emitted. Consumers must configure their own token validation; the unchanged default host still has no authentication scheme. A real engine regression proved explicit Startup overrides render instead of the bundled default and retain original template sources. The generation regression passed55 cases with7 configured external-tool cases ignored; the explicit Startup override case also passed separately. Scoped strict Clippy and formatting passed.

Actual local tools: .NET SDK10.0.401 and .NET/ASP.NET runtime10.0.12 on Linux; default generated client dependencies include Microsoft.Extensions.Hosting/Http10.0.1, while the server uses Swashbuckle6.4.0. The engine remains7.26.0 and the original JAR is unchanged; compatibility-template provenance is recorded in `vendor/openapi-generator/compat/README.md`. Other ecosystems/frameworks, real business/security configuration, broad keyword/union/file support and native distribution remain open.


## Supplied discriminator subtypes without authored aliases

Explicit3.0/3.1 bundles now promote supplied discriminator subtype resources into derived entry components when no authored alias exists. Names use a readable source/pointer hint and12-digit SHA256 suffix; authored name collisions get a deterministic suffix. Equivalent empty/encoded pointer aliases reuse one model. Existing aliases stay preferred, and bare names in an external OpenAPI document resolve to its own schema scope. All mapping targets must exist in supplied schema objects/booleans; OpenAPI info/response/operation objects are rejected as subtype targets. New aliases are capped at256 with the existing source/output/worker limits; per-emitter top-level model limits still apply.

Ordinary subtype references rebase through these entry components. The existing schematools dereferencer now exposes an exact-root-URI preservation option that retains canonical local model pointers at every depth, keeping cross-resource cycles under their named components. The option defaults off and uses the existing resolver/graph machinery. Original/privacy documents and literal mapping/example/default/extension payloads remain intact; no resource loader is invoked.

Generation regression60 passed with7 configured external-tool cases ignored, and all8 owned project API cases passed. Necessary cases covered recursive3.0/3.1 unions, mapping-only models, explicit/duplicate aliases, authored-name collisions, file-order stability, external-document name scope, encoded/static-anchor targets, malformed/missing/non-schema targets and alias limits. Progenitor generated native3.0 SDK tokens from the assembled recursive union; the owned capped worker emitted Go subtype models while excluding private examples/copied options and storing exact original source. Strict scoped generation/server Clippy and formatting passed. This is resolver and artifact/type-generation evidence; per-language union/discriminator runtime parity, dynamic references and broader file/keyword/platform gates remain open.


## Finite binary SDK upload/download

The trusted runtime harness has an explicit `binary` profile that retains the core3.0/3.1 SDK checks and adds Bearer-authenticated octet-stream upload/download. JavaScript, Python, Go, TypeScript and C# default-library clients passed all10 target/dialect combinations:50 actual requests preserved JSON/query/header/enum/nullable/errors and exact binary bytes `00 01 02 0d 0a ff 80 41`. The server checked raw upload bytes and Content-Type; each SDK decoded the integer receipt and verified its download bytes. Go passed race checking, TypeScript built both package formats, and C# used generated Stream/FileParameter APIs. Original engine-generated project files stayed intact apart from renderer-applied compatibility templates; test consumers are separate files.

A real JavaScript failure exposed that the upstream interface emits a File constructor while ApiClient only recognized the string Blob. The pinned Apache-2.0 ApiClient template now selects the existing SuperAgent binary parser for File/Blob constructors/type strings and returns its binary body before JSON/text fallback. The unchanged7.26.0 JAR and normal transport/model emitters remain in use; the override applies only to the default JavaScript library. Actual engine tests verified user ApiClient and ASP.NET Startup templates still take precedence and retain their canonical source bundles. Generation regression60 passed (8 configured external-tool cases ignored), both template precedence cases passed explicitly, and strict scoped Clippy/format gates passed.

This is finite raw binary evidence on the recorded Linux/Node/Python/Go/.NET tools, not multipart/streaming/range/resume or browser runtime coverage. Upstream download buffering/temp-file behavior remains; consumers must handle their returned files/streams. Other language clients/server frameworks, per-target union/dynamic/schema options and native/runtime distribution remain required work.


## SDK multipart files and repeated form arrays

`--profile multipart` now validates the same default JavaScript/Python/Go/TypeScript/C# clients for3.0.3 and3.1.0. All10 target/dialect cases passed, completing40 actual core/multipart requests. The standard MIME parser asserted byte-identical `00 01 02 0d 0a ff 80 41`, file name `payload.bin`, file-part octet-stream MIME, Bearer authentication, quoted Unicode text, repeated required/optional tags and a single comma-packed non-exploded array. Go retained race checking; consumer files/streams are separate from generated sources.

An actual C# generichost request incorrectly serialized `explode=true` tags as one comma-separated value. A pinned Apache-2.0 upstream API template now emits each required/optional multi-array element separately and leaves other arrays/scalars and FileParameter transport handling intact. Apply only to generichost; explicit API templates retain priority. Both3.0/3.1 C# binary profiles also passed after the template change. The generation regression passed60 cases (9 configured external-tool cases ignored), and all3 JavaScript/C#/ASP.NET template precedence cases passed explicitly. Scoped strict Clippy/format/diff gates passed.

This establishes one finite multipart file with tested string-array encodings. Multi-file/nested/custom part MIME/header/delimiter, huge/nonseekable streaming and real browser/platform cases remain open, as do other ecosystems/frameworks and dynamic/union keyword/runtime coverage. Source/license/digest and the concrete failure are in `vendor/openapi-generator/compat/README.md`; the pinned7.26.0 JAR remains unchanged and the existing renderer/transport/model emitters own generation.


## Referenced tagged union SDK roundtrips

The `union` runtime profile passed all10 default JavaScript/Python/Go/TypeScript/C#3.0/3.1 cases:70 real requests retained the core contract, selected both referenced Cat/Dog oneOf members through explicit discriminator mappings and POSTed generated model instances back with exact JSON/Unicode/boolean fields. Consumers use each mature engine's actual union wrapper/tagged representation rather than a custom serializer. Go race checking and both TypeScript package builds passed. C# generated converters additionally rejected missing/unknown tags and a selected subtype missing its required field.

Two concrete upstream defects were fixed through pinned Apache-2.0 compatibility partials: JavaScript's class-field toJSON needed lexical binding after SuperAgent copied the object, and C# must not parse/recheck a valid preselected subtype's discriminator using a merged parent enum containing the other variant. The existing decoder/constructor/transport logic remains mature engine code; successful subtype decoding proves tag presence while other fields and invalid-subtype paths retain their original checks. The unchanged7.26.0 JAR still owns emission. Explicit JavaScript union and C# converter overrides render with priority and retain source/checksums.

Generation regression60 passed (11 configured external-tool cases ignored), all5 compatibility-template precedence cases passed explicitly, and scoped strict Clippy/format/diff gates passed. JavaScript binary and C# multipart3.0/3.1 regression cases also passed after the partial changes. This is representative typed tag/object roundtrip evidence, not all JSON Schema constraint validation or all union shapes. Nullable/nested/ambiguous/anyOf/dynamic behavior, other ecosystems/frameworks, real browser/platform execution and distribution remain required work.


## Saved ordinary single-file 3.1 resource preparation

Saved JSON/YAML3.1 specifications now use the same static resource/anchor, discriminator subtype and reference-sibling adapter as explicit source bundles. The shared project service prepares them in the existing capped/cancellable child, then screens the derived specification plus original semantic documents before any emitter. Original text/comments remain stored exactly; the reserved privacy field never reaches artifacts. The worker reads the actual source dialect rather than trusting stored metadata. A single document's virtual retrieval URI is `moleapi://source-bundle/openapi.json`; supplied `$id`/static anchors and local pointers resolve there, while missing/outside resources still reject without fetching. Ordinary3.0 inputs retain their previous direct parser/sibling behavior.

Generation regression65 passed with11 configured external-tool cases ignored; all10 owned project API cases passed; strict generation/server Clippy and format/diff gates passed. Cases covered JSON/YAML comments, nested IDs/anchors, retrieval aliases, recursion, literals, reference constraints, duplicate/malformed/dynamic/missing resources, byte/depth/node limits and3.0 compatibility. API fixtures generated Go models from plain3.1 source, withheld private examples/copied naming values by default, accepted explicit private inclusion, preserved exact source, rejected missing resources and denied other-owner/anonymous preparation. Hosted/native share this route. This extends source preparation; it does not change Progenitor's3.0-only limit or establish all-target constraint/dynamic/runtime/browser parity.


## Java and Spring default-library runtime evidence

The core runtime harness now also builds/calls default Java okhttp-gson SDKs and builds/starts default Spring spring-boot skeletons for3.0.3 and3.1.0. All4 new target/dialect cases passed. Java's generated ApiClient/DefaultApi/Pet preserved actual GET/POST/path/encoded-Unicode-query/custom-header/Bearer/JSON/nullable/enum behavior and exposed typed HTTP400 errors. The runtime driver uses the actual emitted parameter order; no generated file is rewritten. Maven builds the emitted POM, and a separate trusted consumer performs the real calls instead of counting upstream placeholder tests. Spring executables started, exposed generated OpenAPI/routes and returned their declared501 missing-business-handler response. No consumer handler/security implementation was injected.

Exact local tools: pinned OpenAPI Generator7.26.0, Temurin JDK21.0.12.1+1 and Maven3.9.11 on Linux; emitted Java dependencies include OkHttp4.12.0/Gson2.10.1 and the Spring project uses Boot3.3.13/springdoc2.6.0. The isolated downloaded tool archives were verified against their published hashes; the harness requires explicit JDK/Maven paths. This does not add JDK/Maven or downloads to product preview/runtime. Shared catalog labels now describe representative client-call versus skeleton-start evidence, bringing basic mainstream3.0/3.1 coverage to18 combinations across6 clients/3 server targets. Generation regression65 and scoped strict lint/format gates passed. Other language ecosystems, Java extended profiles, consumer server logic/security, advanced keyword/file/browser/platform and installation gates remain open.


## Java default-library extended profiles

Default okhttp-gson consumers passed all6 binary/multipart/union3.0.3/3.1.0 cases:32 actual requests retained the core contract and verified exact non-UTF8/NUL download/upload bytes, named octet-stream file parts, Unicode text, required/optional repeated arrays and non-exploded CSV, plus both generated tagged-union model roundtrips. File/model APIs and the actual escaped parameter names/order are used; no library defaults or assertions were weakened. Tools and emitted dependencies are those recorded in the Java core continuation.

Two actual multipart failures required pinned Apache-2.0 upstream ApiClient/API template compatibility fixes: each loop iteration now sends its current element rather than the full list, and non-multi arrays retain the declared format through the existing joining helper. Other scalar/file/model/transport behavior remains upstream; overrides still apply only to default okhttp-gson and explicit templates retain priority. Binary/union cases were regenerated and rerun after the change, with all4 affected cases passing. Generation65 passed (12 configured external-tool cases ignored), all7 explicitly configured override/rendering tests passed, and scoped strict Clippy/format/diff gates passed.

This is one finite file and simple string-array/two-object-union evidence. Multi-file/nested/custom/delimiter, streaming/nonseekable/resume/range/browser/platform and nullable/ambiguous/anyOf/dynamic or all-constraint behavior remain required work. The JAR stays pinned7.26.0 and unchanged; provenance/digests and concrete failures are in `vendor/openapi-generator/compat/README.md`.


## Actual Chromium generated SDK profiles

The new explicit `tools/projects/verify-browser.py` harness freshly generates JavaScript/TypeScript-fetch projects through the same runtime, installs their consumer dependencies without lifecycle scripts, bundles their actual browser entrypoints with standard esbuild and drives a dedicated agent-browser session against a same-origin loopback fixture. All16 client/dialect/profile combinations passed in Headless Chromium155 on Linux:76 real API requests covered core authentication/parameters/Unicode JSON/nullable/enum/errors, browser File/Blob byte-identical raw and multipart uploads/downloads, repeated/CSV arrays, and both generated tagged-union model roundtrips. No Node transport/model mocks or custom union serializer replaced the SDK. Prior JavaScript binary/oneOf compatibility patches were verified in this actual browser path.

Evidence records preserve OpenAPI Generator7.26.0, actual browser user agent, agent-browser0.38.2/esbuild0.28.2, original normalized source digest and consumer bundle SHA256 per successful case. Named sessions and owned servers close after the run; projects/package locks and test bundles remain isolated. Generation65 regressions (12 configured external-tool cases ignored), strict scoped Clippy and format/diff checks passed. This establishes the recorded Chromium/same-origin behavior, not Firefox/Safari/native-WebView, cross-origin/CORS/cookies, huge/streaming/custom parts or all advanced schema/runtime parity. Those and other ecosystems/dynamic references/installation remain open.


## Default Kotlin project compile/call evidence

Default `jvm-okhttp4` Kotlin projects passed core3.0.3 and3.1.0 builds/calls through their original emitted Gradle build. A separate trusted Kotlin ApiClient/DefaultApi/Pet consumer and init-script execution task preserved real GET/POST/path/Unicode-query/header/Bearer/JSON/nullable/enum behavior and exposed typed HTTP400 responses. Named arguments use the actual generated APIs. Consumer calls use bounded timeouts and close their connection/executor resources; emitted sources/builds remain unchanged.

Exact local tools: hash-verified isolated Gradle8.14.5 (matching the wrapper), Temurin JDK21.0.12.1+1 on Linux; the generated project used Kotlin plugin2.4.0, Moshi1.15.2 and OkHttp5.4.0. The generator library ID remains jvm-okhttp4 despite its newer emitted transport dependency. Both cases passed; basic mainstream3.0/3.1 evidence now covers20 combinations across7 client/3 server targets, with this catalog entry marked representative compile/call. Generation65 regressions and scoped strict Clippy/format/diff gates passed. This does not add Gradle/compiler/downloads to product runtime or validate Kotlin extended profiles/other libraries/Android distribution. Those and remaining ecosystems/dynamic/advanced-schema/platform/install gates stay open.


## Kotlin default-library file profiles

Default Kotlin jvm-okhttp4 consumers passed all4 binary/multipart3.0.3/3.1.0 cases, completing18 actual core/file requests. The unchanged emitted Gradle build compiled actual generated File APIs and models; consumers used the generator's renamed propertySize receipt member. Wire assertions preserved `00 01 02 0d 0a ff 80 41`, octet-stream upload/download, file name/MIME, quoted Unicode, required/optional multi arrays and non-exploded CSV. The same recorded Gradle8.14.5/JDK21/Kotlin2.4.0/Moshi1.15.2/OkHttp5.4.0 defaults were used.

An actual multipart failure showed the API passing an explode=false list into the transport's repeated-part path. The pinned Apache-2.0 API compatibility template now uses the upstream collection helper for non-multi non-file arrays, retaining other form parts and mature transport/model behavior. Default jvm-okhttp4 only; explicit API overrides render with priority and preserve source. Both binary cases were regenerated/rerun after the change and passed. Generation65 passed (13 configured external-tool cases ignored), all8 explicit template rendering/override tests passed, and scoped strict Clippy/format/diff gates passed.

This is bounded one-file/string-array evidence, not Kotlin union/other-library, multi-file/nested/custom part, delimiter/streaming/nonseekable/resume/browser/Android/platform coverage. These and remaining ecosystems/dynamic/schema/install requirements remain open. Source/license/digest and the reproduced defect are in `vendor/openapi-generator/compat/README.md`; the7.26.0 JAR remains unchanged.


## Explicit Kotlin Retrofit/Gson tagged unions

The catalog/dialog expose upstream `serializationLibrary` (default Moshi) and boolean
`generateOneOfAnyOfWrappers` (default false). Union schemas require an explicit wrapper
configuration; upstream supports `jvm-retrofit2` with `gson` or `kotlinx_serialization`.
Unsupported combinations reject before emitter execution. Schema-node detection ignores
literal examples/defaults and dictionary names. Defaults remain unchanged; default
jvm-okhttp4/Moshi unions remain unsupported.

Actual generated Retrofit/Gson wrappers passed3.0.3 and3.1.0 Gradle compile/call cases:
14 real requests preserved the shared core wire contract and typed Cat/Dog decode/echo
roundtrips. Generated Gson adapters rejected missing/unknown tags, missing variant fields
and object-valued boolean fields. A pinned Retrofit API template moves path parameters
before query parameters after reproducing Retrofit's runtime rejection of the upstream3.1
signature. Named consumer arguments handle dialect-specific ordering; generated builds,
serializers and transport stay upstream. Explicit API templates retain precedence.

Tools: Generator7.26.0, Kotlin2.4.0, Retrofit3.0.0, Gson2.14.0, OkHttp5.4.0,
Gradle8.14.5 and Temurin JDK21 on Linux. Generation regressions passed66 cases
(14 configured external-tool cases ignored after adding the Retrofit override test),
9 explicit template tests and10 owned project API tests passed. Four dialog tests,
production frontend build and scoped strict Clippy/format checks passed. The actual dialog
was exercised in Chromium in English and Chinese at390px; wrapper/library values persisted
and the dialog stayed within the viewport. Catalog responses used trusted fixture data.

This proves simple referenced tagged-oneOf with explicitly selected Retrofit/Gson.
Default Moshi support, anyOf/nullable/nested/ambiguous
unions and other Kotlin libraries/platforms remain required work.


## Explicit Kotlin Retrofit/kotlinx serialization tagged unions

Both3.0.3 and3.1.0 generated projects also passed their original Gradle builds and
14 actual core/union requests with `serializationLibrary=kotlinx_serialization` and
explicit Retrofit wrappers. The consumer uses generated sealed Animal.CatWrapper and
Animal.DogWrapper values and the generated Serializer.kotlinxSerializationJson,
including unknown/missing discriminator, missing required subtype field and invalid
boolean rejection. Generated serializers/transport/build dependencies are unchanged.
Exact libraries: Kotlin2.4.0, kotlinx-serialization-json1.11.0, Retrofit3.0.0,
OkHttp5.4.0; same Gradle8.14.5/Temurin JDK21 Linux tools as the Gson slice.

The QA driver selects a distinct typed consumer from actual generation-manifest options,
including reuse runs, and fails explicitly for non-wrapper/unsupported configurations.
Both Gson3.0/3.1 regression consumers also passed14 actual requests after this change.
This adds two serializer/dialect cases; default Moshi and broader union/platform/keyword
support remain open. No additional product runtime is downloaded or executed by preview.


## Native CLI upgraded connections

Progenitor's existing101 response classification and SDK upgrade transport now feed
an async generated CLI output hook rather than `todo!()`. Generic upgraded connections
stream `--upgrade-input FILE` bytes, half-close outgoing IO at file EOF and continue
receiving byte-identical output to stdout or a create-new `--output-file`. Declared
`x-dropshot-websocket` operations use tokio-tungstenite0.27 over the SDK's upgraded
connection: file input becomes one binary message, text/binary payloads are concatenated
to output until peer close; the library handles client masking/control frames.
Handshake/request/error construction remains the upstream SDK implementation.

A generated standalone project compiled and passed6 actual HTTP requests: generic
duplex stdout and file output, existing-file preservation, WebSocket binary input plus
Unicode text/binary output/Ping/Pong/close acknowledgement, typed400 rejection and
unexpected200 instead of101 rejection. Bearer headers were asserted on every request.
The prior5-request binary download/upload/error CLI fixture also passed after extending
the hook. Local tools: Rust/Cargo1.99.0, Progenitor0.15.0, reqwest0.13.5,
tokio-tungstenite/tungstenite0.27 on Linux. Generated projects use the published client
crate and do not require the vendored emitter patch at runtime.

Generation regression passed67 cases (15 external-tool cases ignored); explicit upgrade
artifact/process tests passed2 cases. All11 owned project API cases passed, including
capped-worker upgraded CLI generation, unauthorized/cross-owner rejection, original-source
preservation, default credential-example exclusion and explicit inclusion. Scoped strict
Clippy/formatting passed. This does not complete CLI parity: raw pagination, interactive
stdin/terminal, WebSocket message-boundary-preserving output, subprotocol/extension and
malformed-handshake coverage and broader/native-platform
upgrade transports remain required work. File WebSocket input is buffered as one message;
generic upgraded input/output and raw output use streaming IO.


## Native optional raw bodies and text file input

The Progenitor adapter preserves `requestBody.required` metadata for recognized raw
binary/text bodies. Optional positional methods accept Option; optional builder bodies
default to None, and provided values set Some. Omitted bodies do not acquire generated
Content-Type/body data. Explicit empty-file input still sets its declared Content-Type.
The CLI's --body-file requiredness matches the original definition. Text files are read
as UTF8 strings instead of byte vectors, fixing generated text-body CLI compilation;
invalid UTF8/missing files fail before request execution. Binary file bytes remain exact.

Generated CLI and positional SDK projects compiled and passed9 actual POST requests:
optional binary/text omission, explicit empty files, binary and Unicode text input,
required text input, and positional optional binary/text calls. Missing required binary
or text flags, invalid text and missing files rejected; server wire assertions ensure no
extra calls. The existing5-request binary and6-request upgrade/WebSocket CLI fixtures
also passed after the SDK/CLI emitter change. All3 standalone process fixtures passed
with a shared explicitly configured Cargo target and serial execution.

Generation regression passed68 cases (16 external-tool cases ignored); all11 owned
project API cases passed with capped-worker optional-text generation, original source
and sensitive-example/owner boundaries retained. Scoped strict Clippy/formatting passed.
This earlier raw-body slice does not establish arbitrary media types or all-platform behavior; optional JSON/form cases and binary file streaming now have separately verified slices below. At that checkpoint file input was buffered; binary file streaming is now verified below, while text/other transports and typed-body profiles remain required work.


## Native CLI typed pagination and raw page errors

The CLI preserves Progenitor's Dropshot `items`/optional `next_page` convention,
first-page-required query parameters and lazy page-token stream. Generated raw error
branches return the original SDK error/nonzero exit rather than `todo!()`. Pagination
limit lookup now uses the actual schema-derived Clap type instead of a fixed NonZeroU32;
the shared positive-integer serialization check prevents the reproduced NonZeroU64
downcast panic and retains total-item limits. Incompatible declared pagination contracts
(raw success pages, missing token parameter, invalid next_page shape) reject explicitly
before CLI emission instead of becoming a one-shot command.

Two standalone generated CLI builds (bounded integer and explicit uint32 limit schemas)
passed18 real GET requests in total. Each exercised full three-page traversal, a limit
spanning pages, limit1 without extra fetch, reserved/Unicode page tokens, removal of
first-page-only sort, and preserved Bearer/custom headers. First-page raw400 fails with
empty stdout; later raw400 retains previously emitted JSON item lines and exits nonzero
without a panic. Missing initial sort, zero limits and manual page-token arguments reject
without additional requests. The prior body/binary/upgrade fixtures also passed in the
same serial5-fixture batch, for38 actual requests across all standalone consumers.

Generation regression passed70 cases (18 external-tool cases ignored); all11 owned
project API cases passed with capped-worker typed/raw-error pagination generation,
source/privacy/owner checks. Scoped strict Clippy/formatting passed. Raw success-page
transports, non-Dropshot pagination conventions, other streamed-input transports, broader typed-body shapes,
interactive CLI and native-platform transport evidence remain required work. This is
verified native3.0 pagination, not full CLI or OpenAPI3.1 support.


## Native CLI sized streaming binary upload

Octet-stream --body-file now uses Tokio ReaderStream with a64KiB capacity and a small
http_body adapter carrying the opened regular file's remaining exact length. reqwest
preserves Content-Length without reading the full file into memory. Reads stop at the
captured length if the file grows; early EOF after shrink becomes a body error. Empty
files retain zero length/MIME and non-regular files reject. This captures size, not an
immutable content snapshot; text file input and single-message WebSocket file input
retain their documented buffering contracts.

A standalone generated CLI uploaded96MiB to a slow loopback server using bounded buffers.
Content-Length, octet-stream MIME, Bearer token and SHA256 matched. Linux peak observed
RSS was12,940KiB on the final run, below the explicit64MiB ceiling; the prior run observed
13,400KiB. Generated helper tests passed exact size/empty/growth/shrink/directory cases.
All6 standalone runtime fixtures passed, including the previous body/binary/upgrade/
pagination consumers (39 real requests total). Generation regression passed71 cases
(19 configured external-tool cases ignored), and all11 owned project API cases passed
with streamed-file generation/privacy checks. Strict scoped Clippy and formatting passed.
The native CLI catalog now labels its representative3.0 compile/call evidence explicitly.

Toolchain remains Rust/Cargo1.99.0, Progenitor/client0.15.0, reqwest0.13.5 on Linux;
the generated helper uses http-body1 and tokio-util0.7 rather than a new transport engine.
Nonseekable input, text/WebSocket chunking, multipart/other-target large files, concurrent
content-mutation guarantees, interactive and native-platform execution remain required work.


## Native optional JSON and form request bodies

The upstream operation metadata now carries requestBody.required across recognized typed
and raw bodies. Optional positional bodies use an outer Option, and optional object
builders default None. Whole-body setters/body_map explicitly create Some and validate
provided builders before sending. Upstream reqwest JSON/form encoders run only for Some;
no Content-Type/body is created for omission. Nullable JSON preserves a second inner
Option, distinguishing an omitted body from a present JSON null. CLI optional bodies do
not force required property flags/files globally; partial supplied objects still reject
missing required fields. Required-body behavior remains covered by the same fixture.

Standalone generated CLI and positional SDK projects compiled and passed16 POST requests:
object omission/provided Unicode fields, explicit empty JSON object, nullable omission/null,
array omission/provided data, optional form omission/URL-encoded Unicode, and required JSON
objects. Missing required whole bodies, partial optional objects/forms and invalid JSON
objects rejected without additional requests. All7 generated-program fixtures passed55
real requests including raw/binary/pagination/upgrade and96MiB streaming regression; the
large upload's latest observed Linux RSS was12,904KiB. Generation regression passed72
cases (20 configured external-tool cases ignored), all11 owned project API cases passed
with optional JSON/form capped-worker generation and source/privacy boundaries. Scoped
strict Clippy/formatting passed.

This proves the recorded referenced-object/array/nullable-scalar/plain-form shapes on
native OpenAPI3.0/Linux. Complex nested/union/form encoding/media variants, further CLI parameter namespaces, templates/interactive workflows and other native platforms remain required
work. It does not establish full JSON Schema validation or OpenAPI3.1 native support.


## Native CLI body file and field exclusivity

Generated JSON/form body property flags now conflict with --json-body through Clap.
Conflicting inputs reject before file access or network rather than silently replacing
field values with file contents. Path/query/header flags stay separate, including body
property name collisions; full files can carry body values different from those arguments.
Body properties named json_body/json_body_template use full JSON input rather than
creating duplicate control flags. Sole-file/sole-field and required-field validation remain.

A standalone generated CLI passed8 real requests: required/optional JSON and form
file-only/field-only bodies, different path/header values sharing body property names,
and full-file reserved body keys. Six file/field conflicts using nonexistent file paths
rejected with Clap usage errors before reads; missing required body also rejected with
no extra calls. All8 standalone fixtures passed63 requests including prior JSON/null/
raw/pagination/upgrade/96MiB upload consumers. Latest large-upload Linux RSS13,488KiB
remained below64MiB. Generation regression passed73 cases (21 external-tool cases ignored),
all11 owned project API cases passed with emitted conflict metadata/privacy checks,
and strict scoped Clippy/formatting passed.

Broader template sampling and parser namespaces, nested/union/
custom body encodings, interactive/native-platform workflows and full CLI parity remain
required work. This slice does not claim unsupported template or namespace capabilities.


## Native CLI control/API argument namespaces

Reserved operation controls (`json-body`, `json-body-template`, `body-file`, `help`)
no longer collide with legitimate API arguments. The emitter reserves all original API
names, then assigns colliding parameters an `api-` prefix, repeating until unused.
Help identifies the original API parameter; SDK setters and wire names remain unchanged.
A body property named help uses full-file input, like other JSON control keys. The
executable disables Clap's automatic help subcommand so a generated operationId help
can execute; --help remains available.

A standalone generated CLI passed3 actual requests: JSON control/file alongside colliding
query/path/header names and existing api- names, binary file control alongside colliding
query names, and an operation named help. Exact original query/header/path/body/Bearer
values were asserted, including distinct file and API body_file/json_body values. Help
showed the prefixed argument and original API name without the supplied token; missing
required body failed before network. All9 standalone fixtures passed66 real requests;
96MiB streaming regression observed12,644KiB Linux RSS. Generation74 cases passed
(22 external-tool cases ignored), all11 owned-worker API cases passed with emitted alias/
privacy checks, and scoped strict Clippy/formatting passed.

This covers the specified controls and repeated-prefix avoidance, not every sanitized-name,
short-option, arbitrary template sampling, interactive or native-platform discovery/namespace behavior.
Those plus broader body/protocol/pagination semantics remain required CLI module work.


## Native offline JSON body templates

The existing --json-body-template flag now prints a validated body value and returns
before URL/token/client setup, body-file reads or API execution. It is exclusive within
the operation and bypasses required operation arguments. Regular execution still requires
--base-url/MOLEAPI_API_URL. Library callers can use Cli::body_template for a value result;
Cli::execute also handles the flag before constructing an SDK request.

Candidate order is media example, first supplied named example (including component refs),
resolved schema example/default, then the mature generated model's Default when available.
False/zero/empty/null values are preserved. Model deserialization and jsonschema0.49.9
Draft7 with format validation check the original upstream-converted schema/components.
No network retrieval feature is enabled. Missing or invalid candidates fail with guidance;
no arbitrary-schema sampler or alternate schema parser is added. A compile fixture also
reproduced dynamic enum Clap values requiring the string feature; generated manifests now
enable it rather than removing enum coverage.

A standalone generated CLI passed media/named/component/schema/default/Default templates,
nested/enum/array/nullable and empty scalar values offline without a base URL. Invalid URL
and token values were bypassed during template output. Invalid candidate minLength/minItems,
no-candidate bodies, conflicting file/field inputs and missing execution URL failed without
API calls. Template files then passed2 real JSON/form POST roundtrips, including nested/
enum models and URL-encoded Unicode. All10 standalone fixtures passed68 API requests;
96MiB upload still observed13,004KiB Linux RSS. Generation75 cases passed (23 external-tool
cases ignored), all11 owned-worker API cases passed, including default media-template secret
exclusion and explicit inclusion. Strict scoped Clippy/formatting passed.

Arbitrary schema sampling when no example/default exists, broader conversion/constraint
keywords, further template editing/sampling, interactive workflows and native-platform evidence
remain required module work. This establishes the specified offline candidate contract,
not an automatically valid example for every possible API schema.


## Native named body templates and explicit null fidelity

--json-body-template=EXAMPLE explicitly selects a supplied media example, including
component references. Bare --json-body-template keeps the original media/first-supplied/
schema/model-default precedence. Author order remains intact even though stored named
maps are deterministic. Explicit unknown, external-only and invalid names fail without
fallback/download/API execution; selected null/false/zero/empty strings remain values.
Cli::body_template retains its default contract and body_template_named adds explicit
library selection. Internal template helper names use a private prefix to avoid the
reproduced operationId named/public-method collision.

Runtime investigation reproduced two upstream defects rather than simplifying fixtures:
operationId string shadowed the standard String error type in generated builders; affected
emitted strings are now fully qualified. openapiv3 2.2.0 Option<Value> parsing collapsed
explicit null into missing. A versioned MIT/Apache-2.0 parser patch uses standard Serde
value deserialization for Example.value, MediaType.example and SchemaData.example/default;
missing stays None, explicit null stays Some(Null), and serialization roundtrips both.
Only four upstream source files change; original fixtures/licenses and archive SHA256 are
retained. No separate OpenAPI parser is introduced, and generated projects need no parser patch.

A standalone CLI passed offline original-order, named/component/Unicode/falsy selection,
media/schema null overriding a different default, missing/external/invalid rejection and
exclusive field behavior. Selected templates completed2 real JSON-false/form-Unicode POSTs.
All11 standalone fixtures passed70 real requests; latest96MiB upload RSS13,084KiB on Linux.
Generation77 cases passed (24 external-tool cases ignored), including new parser null-value
roundtrip checks; all11 owned API cases passed with named-template default secret exclusion
and explicit inclusion. Upstream parser18 unit/5 fixture/2 doc tests passed in an isolated
copy. Strict scoped Clippy/formatting passed.

Arbitrary no-example sampling, empty/nonstandard labels, broader reserved standard-library
names, advanced constraints/conversions, interactive/native-platform execution and full CLI
parity remain required work. Empty CLI selector means default mode; the library API can
select an explicit empty name if present. External example values are never fetched.


## Native builder operation names and support type scopes

The mature builder emitter now passes its builder tokens through Syn's AST visitor to
qualify standard Result/Option/String and Some/None/Ok/Err, plus parent SDK Error,
ResponseValue, ByteStream and OperationInfo paths. Operation impl paths with lifetime
arguments, model-qualified paths, public builder identifiers and literal documentation
remain intact. SDK helper traits import anonymously, avoiding duplicate name bindings.
This adds scope qualification to the existing emitter rather than another operation parser.

A standalone generated CLI compiled13 operations named result/option/some/none/ok/err/
error/response_value/byte_stream/operation_info/client_info/client_hooks/request_builder_ext
and completed13 POSTs. Optional body omission/present JSON, query and Bearer values
were asserted. A separate direct Rust consumer compiled the retained public builder::Result,
Option, Error, ResponseValue and Some types. All12 generated-program fixtures passed83
requests, covering prior raw/pagination/upgrade/template behaviors;96MiB upload observed
13,336KiB Linux RSS below64MiB. Generation78 cases passed (25 external-tool cases ignored),
all11 owned API cases passed with standard-named worker artifacts/privacy, and strict
scoped Clippy/formatting passed.

Other identifier/model/short-option and user-hook namespaces, merged/separate tag options,
all-platform execution and broader generator semantics remain required evidence. This
slice proves the specified names and existing default interfaces, not every possible Rust
identifier or customization. Public operation builder names remain stable.


## Default PHP/Guzzle core runtime evidence

The trusted runtime driver now supports php using explicit --php and --composer paths.
Both3.0.3 and3.1.0 projects kept the upstream Guzzle library, composer.json, PHP model/API/
serializer sources and package namespace. Composer installed runtime dependencies without
plugins/scripts, generated PHP files passed syntax checks, and a separate consumer used
actual named API parameters to retain dialect-specific ordering. Both cases completed6
real requests preserving path, quoted Unicode query/body, custom header, Bearer, model
enum/null decoding and typed400 Failure response objects. No custom transport/JSON emitter
or compatibility patch was required. Representative basic coverage is now22 combinations
across8 SDK clients and3 server skeletons; broader shapes remain separate gates.

Local QA tools were extracted only into /tmp from Arch repository packages verified
against local repository SHA256 metadata (no system install). PHP8.5.11, Composer2.10.3,
Guzzle7.15.5, PSR7 2.13.1 and Promises2.5.3 on Linux; Generator7.26.0/JRE21 remained.
PHP package SHA256: `3f009d6c41e36a95ee03ef5af7ac0ae4bf08814c1853171a344f3ca109d01ae6`;
Composer package SHA256: `a37663752abed59a06b9e647863a4e363b6561c41b4cb3b3681711975fd34fee`.
libzip/argon2 extraction hashes, full package URLs, composer.lock, dependency JSON and
engine/tool/options evidence were retained in the isolated QA output/tool directories.

Generation regression passed78 cases (25 external-tool cases ignored), the explicit
configured PHP artifact fixture passed, all11 owned project API regressions and scoped
strict Clippy/formatting passed. Catalog validation now labels representative PHP3.0/3.1
compile/call evidence. Product preview/download still needs only its configured Java
engine and never downloads/executes PHP or Composer. Extended PHP profiles, namespace/
library options, PHP versions/platforms and full keyword/constraint behavior remain open.


## Default PHP raw binary and multipart profiles

Both3.0.3/3.1.0 default Guzzle binary and multipart projects passed Composer install,
generated PHP syntax checks and18 actual requests across4 profile/dialect cases.
Consumers use emitted SplFileObject APIs and delete temporary uploads/downloads. Raw
bytes remained exact, and multipart preserved payload.bin/octet-stream, Unicode note,
required/optional repeated tag fields and packed CSV tags. Core auth/query/model/error
assertions remain in each profile; no handwritten transport or model serializer replaces
PHP SDK calls.

Initial binary failure was a QA framing defect: SplFileObject already produced a correct
unknown-length chunked stream, while the old fixture read absent Content-Length as zero.
The fixture now reuses Python's standard HTTP decoder for chunked framing; focused tests
preserve binary chunks/extensions/trailers, caller stream ownership and reject premature
EOF. Initial multipart calls exposed a real upstream defect: FormDataProcessor flattened
primitive lists to tags[0]/tags[1] and ignored declared CSV. A pinned default-Guzzle API
compatibility template keeps scalar lists for multi and uses the existing collection
serializer for non-multi formats before nested flattening. The change is scoped to selected
multipart/form-data; file/object/nested and other content types retain upstream handling.
Explicit API templates keep precedence; other PHP libraries do not load this default.

All10 configured explicit override tests passed, generation78 cases passed (26 configured
external-tool cases ignored), all11 owned project API regressions and strict scoped
Clippy/formatting passed. Tools remain PHP8.5.11/Composer2.10.3, Guzzle7.15.5/PSR7 2.13.1
on Linux, Generator7.26.0/JRE21. Finite one-file/primitive string-array evidence does not
complete PHP unions, custom/nested/multiple-file/MIME/streaming/resume/platform behavior.
Those and full module parity remain required work.


## PHP mapped referenced tagged-oneOf runtime

The default PHP serializer originally required inheritance and concatenated raw tags as
model names; Cat payloads stayed on merged Animal and failed its Dog-only enum. A pinned
model partial now retains upstream mapped classes and the original discriminator wire
property for oneOf. The serializer selects those ModelInterface implementations and uses
their existing valid/listInvalidProperties checks before returning. Missing/unknown tags
and missing required subtype fields reject. Other model paths retain upstream processing.
No handcrafted transport/JSON serializer or switch to another PHP library is introduced.

Both3.0.3/3.1.0 cases passed actual generated Cat/Dog GET decoding and model POST echoes,
including exact quoted Unicode/name/barks fields. Additional animal-kind and numeric-string
0 wire properties passed both dialects, covering normalized-property lookup and falsy-name
handling. Six union cases passed42 requests; core/binary/multipart regression cases passed
24 more, totaling66 PHP requests across12 cases. Consumers inspect actual generated getter/
attribute metadata for the selected wire field rather than fabricate variants. Named source
fixtures retain original engine/options/source hashes and discriminator-property evidence.

All11 explicit template override cases passed, including model partial and serializer
priority/source retention. Generation78 cases passed (27 external-tool cases ignored),
all11 owned project API regressions, focused framing tests and strict scoped Clippy/formatting
passed. Tools remain Generator7.26.0, PHP8.5.11/Composer2.10.3/Guzzle7.15.5 on Linux.

Cat/Dog implement ModelInterface but do not inherit the generated Animal class; upstream
parent PHPDoc/static return annotations and merged parent models are not complete nominal
oneOf typing. This establishes mapped variant decoding/echo/required checks. Static parent
annotation/interface parity, anyOf/untagged/nested/ambiguous/nullable unions, primitive
coercion/full schema validation, custom libraries and native platforms remain required work.


## PHPStan direct model API contracts

Default Guzzle model headers export a local ModelValue alias: self for ordinary models,
mapped class unions for tagged oneOf. API class imports unique aliases for each direct
non-container model parameter/return and synchronous entrypoints use PHPStan tags.
This avoids borrowing the operation response discriminator to describe a different request
model. Runtime classes, standard PHPDoc, transport and method signatures are retained.
Nominal Animal inheritance is still absent; the PHPStan contract now describes Cat|Dog.

PHPStan2.3.1 level7 consumers passed both3.0.3/3.1.0 projects, asserting Cat|Dog returns,
accepting Cat/Dog arguments, preserving ordinary Pet returns, and distinguishing Pet
requests from Animal responses on the mixed-model operation. Negative consumers reported
exactly two argument.type failures for swapped models. Actual generated SDK union calls
also passed14 requests. Reports/tool versions are retained; PHPStan is an explicitly
configured QA PHAR and is never installed/executed by product generation.

All12 explicit template override tests passed (including model header), generation78
cases passed (28 external-tool cases ignored), all11 project API regressions and strict
scoped Clippy/formatting passed. PHPStan direct synchronous contracts are verified;
arrays/async/http-info tuples, Psalm/other analyzers, nominal hierarchy, advanced schemas/
names and platforms remain required work. Standard PHPDoc alone is not full union typing.


## PHP one-level model arrays, async waits and HTTP-info tuples

The default Guzzle API annotations now retain mapped/ordinary model aliases for one-level
model arrays and parameters of synchronous/http-info/async/request methods. HTTP-info
uses explicit result/status/header tuples. Async return annotations use typed PromiseInterface
with a shipped PHPStan-only generic wait stub and phpstan.neon. Those supporting files
are emitted through the existing generator config; explicit source or destination overrides
retain priority. Runtime Guzzle interfaces/implementations and SDK requirements are unchanged.

PHPStan2.3.1 level7 passed both dialects for direct/array mapped results, promise wait and
wait(false), status/header tuples, ordinary/mixed model behavior and six exact wrong-model
argument.type failures per negative consumer. The QA config includes the delivered stub
config. Actual list/decode/array model POST, async direct/array waits and sync/async HTTP-info
calls passed28 requests across3.0/3.1. Core/binary/multipart regression passed24 requests.
Generated PHP syntax, all13 explicit override tests, generation78 cases (29 external-tool
cases ignored), all11 owned API regressions and scoped strict Clippy/formatting passed.

This proves one-level arrays and wait-result PHPStan contracts, not nested/map containers,
Promise callback/chaining/rejection generic inference, other analyzers, nominal parent
inheritance or all-platform behavior. Those and broad generator semantics remain required.
PHPStan is a QA/user analysis tool; preview/download never installs or executes it.


## PHP Promise callbacks and async rejection parity

The shipped PHPStan-only stub now describes then/otherwise callback input/result types,
retains original values when handlers are absent, and adopts returned typed promises.
Actual Guzzle runtime remains in use. PHPStan2.3.1 positive consumers passed mapped
callbacks, no-handler identity, rejection recovery unions and promise adoption on both
dialects; seven exact argument.type errors were required from negative consumers,
including a callback whose Pet parameter cannot accept Cat/Dog.

Actual callbacks also mapped results, adopted FulfilledPromise values, preserved success
with no fulfilled handler, recovered400 failures and handled thrown RuntimeException.
The initial async400 call exposed missing typed Failure response objects. A shared
upstream-metadata error-deserialization partial now handles both sync/async exceptions,
retaining model/status/body/header parity. Async transport ConnectException and response-
less RequestException branches return ApiException rather than dereference a missing
response. Both dialects verified a closed-port connection failure with status0/null body.

Forty async/list/tuple/callback HTTP requests and24 core/binary/multipart regression
requests passed; local no-response failures were additional checks. All14 explicit override
cases passed, including the shared error partial in both handlers. Generation78 cases
passed (30 external-tool cases ignored), all11 owned API regressions and scoped strict
Clippy/formatting passed. Broader cancellation/race/mixed rejection policies, nested/map
containers, other analyzers, nominal hierarchy and native-platform coverage remain open.


## Default Dart HTTP core SDK

Both3.0.3/3.1.0 default Dart projects kept upstream pubspec/model/API/serialization output,
resolved original dependencies, passed warning-free lib/consumer analysis and compiled
native executable consumers. Actual generated positional signatures determine argument
order rather than altering APIs. Eight HTTP requests preserved path, Unicode query/header/
Bearer, quoted JSON, enum/null decoding and400 exceptions; SDK public transport also
rejected truncated bodies, and closed-port checks verified no-response conversion.
A pinned api_client template awaits three Response.fromStream futures inside the existing
try/catch, fixing analyzer warnings and preventing response-read errors escaping SDK
exception conversion. Explicit client templates retain precedence; Dart-Dio is separate.

Dart3.13.5 Linux SDK was repository-SHA verified and extracted only in /tmp (package
SHA256 `b4cc5ad1ee604909245c7e413af2b1167b7ee4c16a55fb68a03afcf5308853cb`).
Resolved dependencies/lock included http1.6.0, intl0.20.3, collection1.19.1, meta1.19.0,
test1.21.7; the emitted manifest was not retargeted. Generator7.26.0/JRE21 remained.
Catalog status now names representative3.0/3.1 compile/call evidence; basic mainstream
coverage is24 combinations across9 clients/3 server skeletons. Product preview never
installs or executes Dart. All15 configured override cases, generation78 cases (31
external-tool cases ignored), all11 API regressions and scoped strict Clippy/format passed.

Only this recorded modern SDK/dependency/Linux core contract is established. Advertised
older SDK range behavior, dependency lower bounds/abort types, files/multipart/unions,
strict keyword/model validation, other libraries/browsers/platforms remain required work.


## Default Dart binary and multipart API

Both3.0.3/3.1.0 default HTTP binary/multipart projects passed original pub resolution,
warning-free analysis and AOT consumers. Four cases completed22 HTTP requests (including
core/truncated transport checks); separate closed-port failures remained covered. Downloads
now create the existing MultipartFile type directly from response.bodyBytes rather than
passing arbitrary binary through text/JSON decoding. Raw upload keeps the upstream file
stream path. Byte-identical payloads were verified.

The pinned API template removes redundant file.field text parts and uses existing
_queryParams collection formats. Repeated fields use the mature multipart encoder's
filename-less string parts; CSV stays joined fields. Null guards emit only for nullable/
optional inputs, resolving analyzer warnings for required file/note/tags. Actual name/MIME,
Unicode note, required/optional repeat order and CSV fields passed unchanged wire assertions.
Explicit API/client templates retain priority; Dart-Dio/other libraries remain separate.

All16 configured override cases passed, generation78 cases passed (32 external-tool cases
ignored), all11 owned API regressions and strict scoped Clippy/formatting passed. Dart3.13.5/
http1.6.0/Linux and Generator7.26.0 are the recorded tools. Finite MultipartFile payloads
don't establish zero/large/resume/custom/multiple/nested file/MIME/SDK-range/platform or
union semantics; these remain required module work. Download filename/content-type
metadata beyond the constructor's default contract is also separately unverified.


## Default Dart mapped oneOf wrappers

The default engine merged Cat/Dog into an Animal requiring both name and barks and a
merged discriminator enum. Mapped oneOf models now use a small Animal(actualInstance:...)
wrapper selected from the upstream discriminator metadata. Actual Cat/Dog fromJson/toJson
codecs and upstream list/map helpers remain in use; API Animal argument/return types stay
nominally consistent. The wrapper constructor rejects unsupported classes, decoder checks
subtype required keys and converts invalid/unknown tagged input to FormatException in
release builds. Ordinary/unmapped models retain native_class; explicit model/wrapper
partials keep priority. Existing merged-field Animal construction is replaced by actual
variant construction and regeneration should be reviewed for that model API change.

Both3.0.3/3.1.0 projects passed warning-free analysis/AOT and20 real HTTP requests including
core/truncated behavior, Cat/Dog model decode/echo and wrapper-array list/POST roundtrips.
Unknown/missing tags, missing subtype fields and object-valued barks reject in release
consumers. Core/binary/multipart regressions passed30 additional requests. All17 explicit
override cases, generation78 cases (33 external-tool cases ignored), all11 owned API cases
and strict scoped Clippy/formatting passed on Dart3.13.5/http1.6.0/Linux, Generator7.26.0.

This proves referenced tagged mapped-object oneOf wrappers and simple arrays, not full
JSON Schema validation. Untagged/anyOf/nullable/nested/ambiguous/duplicate-alias unions,
custom field names, empty/large/advanced files, other serialization/libraries, SDK ranges
and native/browser platforms remain required generation-module work.


## Default Swift6 URLSession SDK core

Both3.0.3/3.1.0 original Swift6 packages compiled in release mode as local dependencies
of a separate typed consumer; no SDK manifest or library switch was used. Six HTTP calls
preserved path, Unicode query/header/Bearer, quoted JSON, enum/null models and400 status/
body decoded through the generated Failure Codable model. Actual Swift argument labels/
order are derived from emitted signatures. The initial call exposed raw plus in URLComponents
queries; a pinned API template escapes + in percentEncodedQuery after upstream query-item
construction. Other encoding/serialization/transport stays upstream and overrides keep priority.

Official Swift6.4.1 Ubuntu24.04 archive was downloaded/extracted only into /tmp, with PGP
signature verified against the official Swift keys file (fingerprint
52BB7E3DE28A71BE22EC05FFEF80A866B47A981F). Archive SHA256:
`7d246c7114077ad01dba4ad897628446ada4c489d6c7dbcf39ae3f25cbe17e8b`.
Arch host lacked matching Ubuntu ncurses/xml2/ICU; official package SHA256s were verified
and shared libraries staged in the toolchain's existing runtime search path so SwiftPM's
compiler subprocess could resolve them. Compiler bytes/system libraries remain untouched.
Tool/source/package integrity and dependency/build metadata remain in isolated QA outputs.
Generator7.26.0/JRE21, Swift language mode6 and original URLSession runtime were used.

All18 explicit override tests, generation78 cases (34 external-tool cases ignored), all11
owned project API regressions and scoped strict Clippy/formatting passed. Catalog labels
representative core evidence; basic mainstream coverage is26 combinations across10 clients/
3 server skeletons. Product preview/download never installs/executes Swift. Apple-device/
Darwin, other toolchain/library/platform execution, advanced files/unions/path/form/abort/keyword
behavior and broad deployment support remain required work. This is Linux core evidence.


## Default Swift6 finite binary and multipart files

Default URLSession3.0.3/3.1.0 release consumers passed four binary/multipart cases with
20 real requests, retaining core path/query/header/Bearer/Unicode/enum/null/400 checks.
The original file URL APIs download byte-identical `00 01 02 0d 0a ff 80 41`, upload a
local URL and decode the generated UploadReceipt. Multipart preserves payload.bin,
octet-stream MIME, quoted Unicode note, repeated required/optional arrays and one CSV
non-exploded array. A second upload omits both optional arrays and verifies absence on
the wire. Temporary consumer files and downloaded cache files are removed after use.

The initial multipart request reproduced packedTags as repeated parts. A pinned
swift6/_param.mustache compatibility partial joins non-multi primitive form arrays
through the existing conversion helper before the unchanged URLSession multipart
transport; required/optional multi arrays and scalar/file/query/path/header parameters
retain upstream behavior. Default URLSession only; explicit API and parameter partial
bundles retain priority. Original SDK manifest, transport and model files remain intact.

Core regression passed six more requests; all19 explicit overrides,78 generation cases
(35 configured external-tool cases ignored),11 owned project API cases, the fixture
parser tests and scoped strict Clippy/format checks passed. Tools are the isolated
Swift6.4.1/Linux and pinned generator7.26.0/JRE21 recorded above. Finite buffering/cache
behavior remains upstream; huge/nonseekable/streaming, nested/custom/multiple parts,
other encoding/library/toolchain/platform and Apple/Darwin evidence remain open.


## Default Swift6 mapped tagged oneOf

Original generated Animal enum cases typeCat/typeDog carry actual Cat/Dog Codable
models. Separate Swift6 release consumers passed six3.0.3/3.1.0 cases for original wire
discriminator fields kind, animal-kind and 0:54 actual core/union requests. Both typed
variant GETs and explicitly constructed enum case echoes preserve Unicode fields and
wire tags; arrays decode and echo through generated [Animal] API types. Required subtype
keys, unknown/missing tags and wrong name/barks types reject through native JSONDecoder
in release mode (six malformed objects per case). The consumer derives original subtype
initializer labels/order, including animalKind and _0, from emitted model sources.

No model/union/serializer template change was needed: pinned generator7.26.0's original
Swift6 URLSession enum/Codable handling passed directly. SDK Package.swift/transport/
models remain unchanged, retaining prior query/form compatibility partials and override
contracts. Existing core/binary/multipart regression passed26 more requests;19 explicit
compatibility overrides,78 generation cases (35 external-tool cases ignored),11 owned
project API cases, fixture parser tests and scoped strict Clippy/format checks passed.
Tool/platform evidence remains the isolated official Swift6.4.1/Linux setup recorded above.
Untyped/untagged/anyOf/nested/nullable/ambiguous unions, advanced keyword constraints,
custom aliases/options, other libraries/toolchains and Apple/other platforms remain open.


## Protoc reachable imported models and four-mode Python gRPC

Non-Rust Proto inputs now emit every reachable non-well-known descriptor, matching the
Descriptor input behavior. Previously only entry_files were generated: a real C++ project
omitted models/types.pb.h, and Python imports failed with ModuleNotFoundError. The shared
protox pool already contained the validated import graph; protoc now receives its complete
bounded target list. Unused supplied files stay out of generated code. GoogleFileResolver
identifies actual built-in types instead of dropping every google/protobuf path; a supplied
custom path under that prefix retains its C++ model output. All generated target paths
receive the original path/flag/32-file limits; a leading-dash imported filename rejects
before compiler launch. Descriptor-only inputs, process group/deadline/output budgets and
explicit compiler/plugin configuration remain in use.

An imported enum/bytes/repeated/oneof/Timestamp model and four-mode service generated in
all8 builtin languages with explicit protoc36.1 on Linux. For each language, source and
descriptor regeneration produced identical code (16 graph-generation cases), including
the imported models; standard Timestamp runtime sources were not copied into projects.
Exact language filenames are asserted, including PHP's PBEcho keyword rename. This is
all-language generation evidence; it does not claim all-language compilation or calls.

Separate Python consumers use unchanged generated modules and actual grpcio1.84.0 /
protobuf6.33.6 with grpcio-tools' integrated protoc31.1. Both source and descriptor outputs
imported successfully and passed real unary/server-streaming/client-streaming/bidirectional
calls over loopback (8 RPCs), preserving Unicode, non-UTF8 bytes, enum/repeated fields,
oneof presence including code=0, and Timestamp values. QA tool hashes/versions and per-
representation runtime evidence are retained in isolated output. Product preview never
installs runtimes, runs consumers or acquires plugins.

Both explicitly configured generation tests,78 normal generation cases (35 external-tool
cases ignored),11 owned project API cases, fixture parser tests and scoped strict Clippy/
format checks passed. Catalog evidence distinguishes builtin message generation from the
configured Python-plugin four-mode fixture. Other-language compilation/gRPC plugins,
advanced schema/options/dependency/runtime ranges, platform deployment and broader
full-module scopes remain open.
