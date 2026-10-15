# Request snippet coverage

This is usable request-example generation, separate from typed OpenAPI SDK/server/CLI generation. Full Apifox/Postman parity and the complete generation module are not achieved. Engine: pinned Scalar snippetz0.10.5 plus official Postman code-generators2.1.1/Collection SDK5.3.1,23 language families/55 library adapters, embedded in Rust QuickJS with mature core-js/Buffer/path/string decoder/URL polyfills. All55 adapters generate nonempty code for the representative HAR smoke case. That does not imply each variant compiles/runs or preserves every configuration.

| Language family | Libraries exposed by upstream | Actual compile/run evidence |
| --- | --- | --- |
| C | libcurl | libcurl POST fixture |
| C# | httpclient, restsharp | HttpClient and RestSharp114.0.0 POST fixtures compiled with .NET10.0.401/C#11; upstream snippets unchanged inside namespace/project wrappers |
| Clojure | clj_http | Engine smoke only |
| Dart | http, dio |14 original-source analyze/AOT/run cases on Dart3.13.5/http1.6.0/Dio5.11.1: text/form/JSON/files/MIME/multipart/dollar literals |
| F# | httpclient | Engine smoke only |
| Go | native | net/http POST fixture |
| HTTP | http1.1 | Engine smoke only |
| Java | asynchttp, nethttp, okhttp, unirest |24 original-source compile/call cases on JDK21: JSON, repeated form, text/control bytes, raw binary, explicit binary MIME and multipart files |
| JavaScript | fetch, axios, ofetch, jquery, xhr, native, request, unirest | Fetch on Node runtime; browser-specific variants remain unverified |
| Julia | http | Engine smoke only |
| Kotlin | okhttp |7 original-code compile/call profiles on Kotlin2.4/OkHttp4.12, including native files, MIME, multipart and literal dollar/interpolation text |
| Node.js | fetch, axios, ofetch, undici, native, request, unirest | Fetch and Native/Request/Unirest POST fixtures |
| Objective-C | nsurlsession | Engine smoke only |
| OCaml | cohttp | Engine smoke only |
| PHP | curl, guzzle, laravel, httprequest2, pecl_http |54 original-code syntax/native HTTP cases for cURL/Guzzle8.2.0/Laravel HTTP13.36.0 on PHP8.5.11; HTTP_Request2/PECL HTTP remain smoke only |
| PowerShell | webrequest, restmethod | Engine smoke only |
| Python | python3, requests, aiohttp, httpx_sync, httpx_async | 80 original-code syntax/native HTTP cases across all5 clients on Python3.14.8; aiohttp3.14.5, Requests2.34.2, HTTPX0.28.1, urllib3 2.8.0 multipart encoder |
| R | httr2, httr, rcurl | Engine smoke only |
| Ruby | native |8 original-code syntax/native HTTP cases on Ruby3.4.7: JSON/form/text/files/MIME/multipart/dollar and Ruby interpolation literals |
| Rust | reqwest | reqwest POST fixture, generated snippet wrapped in async main |
| Shell | curl, curl_windows, wget, httpie | cURL POST fixture |
| Swift | nsurlsession | Engine smoke only |
| Postman CLI | postman_cli | Generation only; target CLI is an external runtime dependency |

Current official interface-request lists do not include separate TypeScript/C++ adapters. These remain required in business SDK/model generation; JavaScript/C snippets are not relabeled as those languages. SDK/server framework generation, protobuf stubs, generated CLI and repeatable multi-file manifests/diffs remain separate required work.

## Behavior and current limits

The saved HTTP request becomes HAR via the shared Rust adapter. Query encoding, selected auth replacing manual Authorization, transport-owned header removal, text/JSON/urlencoded/multipart bodies and Content-Type defaults have meaningful regressions and real fixture evidence. Multipart conversion preserves enabled parts, repeated names, file references and supplied MIME metadata. Postman supplemental generators receive Collection SDK form-data, not empty raw bodies. Unsupported protocol/body/auth modes return errors. Upstream library source owns formatting and escaping; MoleAPI does not implement language templates.

Environment placeholders remain static, with no runtime resolution or pre-script execution. Some libraries expect complete runnable programs, others emit snippets for an existing function/project. Required target libraries/runtimes must be supplied by the user. Python/http.client requires a concrete absolute HTTP(S) URL because upstream otherwise invents example.com for a base template. URLs with literal quote/backslash/newline characters are rejected rather than emitting unsafe upstream quoted source. Header/query/body placeholders still require review before execution.

Repeated urlencoded form/header names are explicitly rejected outside verified shell/cURL; this avoids silent losses in dictionary-based upstream variants. Multipart repeated names have separate representative evidence below. Fetch-family JS/Node GET/HEAD bodies are rejected. Timeouts, TLS verification and redirect options are not yet mapped and this is stated in returned warnings. Shared multipart/files/binary execution is implemented. Multipart and raw-file snippet mappings are implemented with the target limits recorded here.

Multipart snippets reference saved filenames rather than embedding selected file bytes. Prepare the corresponding files in the target environment; generation never reads host files. A returned bilingual warning explains these prerequisites and target-library MIME/filename behavior. Browser examples may require a selected File/Blob. Default privacy screens structured fields without changing fixed enum/schema keys, clears file byte sources and hides sensitive text fields/copied values; original canonical requests stay unchanged. Explicit inclusion preserves text/auth values but file bytes still stay outside generated source.

All55 libraries generated nonempty multipart code for a basic HAR/filename case; this is availability evidence rather than compilation or all-body configuration evidence. Nine actual saved-request fixtures sent POST/auth/query/repeated Unicode and quoted text plus byte-identical binary upload, filename and application/octet-stream: Node Fetch, Native, Request, Unirest; Go; cURL; Python Requests; C# HttpClient/RestSharp114. Necessary upstream patches retain escaping, MIME, byte buffers, repeated Request fields and Unirest field names. Custom part MIME combinations, unusual names/paths and remaining languages need broader evidence; these fixtures do not establish universal multipart support.

Default previews/downloads exclude known private values and credential-bearing fields. User-defined JSON keys/scalars and decoded query/form fields are screened; fixed request schema keys remain intact. Literal/base64/form/path variants of known values and the current Basic tuple are matched. Full Standard/URL-safe-no-pad Base64 string values or explicit Basic headers up to16KiB are decoded with the mature crate and withheld if decoded UTF8 contains a private value. Arbitrary transformed encodings, mixed opaque blobs and larger encoded fields are not claimed detectable. Explicit include-secrets is visible and preserves saved values; browser-only overrides are never attached.

Generation does not execute or publish output. Dialog owner/content/target/credential/open fences discard stale results, including equivalent save-key reorder handled by mature stable JSON serialization. Four actual worker-held slots, host-free QuickJS memory/stack/time/input/output caps and compile-time bundle digest/version verification are enforced. Native IPC uses the same service module; native graphical/platform packages still need final integration validation.

## Reproduce

`cargo test -p moleapi-generation --locked` exercises all55 upstream adapters plus privacy/representability regressions. `cargo test -p moleapi-server --test generation --no-default-features --locked` exercises owner/default/private/native routes. `npm --prefix web test -- --run src/features/generation/GenerationDialog.test.tsx` exercises save order, late owner responses and stable content identity.

`python3 tools/snippets/verify-fixture.py` uses only trusted synthetic requests, starts a local fixture, generates through the actual saved-request adapter, compiles/runs seven representative programs and checks real POST/auth/header/query/Unicode JSON. It needs Node/Python/Go/C+libcurl/Rust and cached Rust dependencies. No Docker is used. Generator sources/licenses/reproducible locked build are documented in vendor/snippet-engine/README.md. Fresh independent review is in tasks/implementation/snippets-review.md; broader generation/feature work remains active.


C# validation used the actual saved-request adapter and also the owned saved-request HTTP API. Both emitted variants preserved method, selected Bearer replacing manual Authorization, custom headers, query encoding and Unicode/quoted JSON in real requests. The HttpClient snippet needs System.Net.Http/System.Net.Http.Headers; RestSharp needs its NuGet package/namespace. JSON raw string literals require C#11+. These are snippets for an async method or a compatible top-level-await project, not standalone project archives. Reproduce only these variants with `CARGO_TARGET_DIR=/path/to/cache python3 tools/snippets/verify-csharp.py --dotnet /absolute/path/to/dotnet`; .NET SDK/NuGet restore or cache is a validation prerequisite, not a MoleAPI runtime dependency.

Downloads now use language source extensions (including .go/.cs/.sh) and the shared browser/native file saver. Native dialog selection uses per-file write permission; owner/content/open checks prevent stale writes after the picker resolves. Static warnings and C# dependency hints have English/Chinese UI translations. Timeout/TLS/redirect/body-mode limits above remain real gaps; successful fixtures do not imply every target/library or option has been validated.

Exact selectable library mapping is in [REQUEST-CODE-PARITY.md](REQUEST-CODE-PARITY.md). Trusted fixed engine initialization now has its own2s deadline, retaining64MiB memory/512KiB stack and a separate250ms untrusted generation budget; parallel requests no longer exhaust their budget during fixed bundle loading. A reproducible npm patch corrects Node Native encoded queries through official postman-url-encoder; runtime still has no filesystem/network/Node process access.

Reproduce only trusted multipart cases with `CARGO_TARGET_DIR=/absolute/cache python3 tools/snippets/verify-multipart.py --node-modules /absolute/node_modules --dotnet /absolute/dotnet`. Use a Python environment with Requests2.32.5, Go/cURL/Node and cached RestSharp114/.NET10. `--clients` selects a comma-separated subset to avoid repeating already passing cases. The saved-request API fixture covers owner isolation, default/explicit credential behavior and source preservation. Clean npm installation plus contextual patch replay reproduced the actual verified embedded bundle SHA256.

## Original binary file body examples

The existing BinaryBody source projects filename and effective MIME into a shared file reference. Saved Base64 is never passed to the emitters or embedded in code, including explicit credential mode. Existing Content-Type overrides take precedence over the file's MIME; otherwise its MIME/default application/octet-stream is emitted. Prepare the corresponding local file before running the example.

Twelve adapters expose binary_file in the catalog: Node Fetch; Go Native; Python Requests; C# HttpClient/RestSharp; Rust reqwest; Kotlin OkHttp; cURL/Windows cURL/HTTPie/Wget; Postman CLI. Their mature file modes/writers read bytes without converting them to text. All12 generated filename-based examples without saved Base64 or generic file-content placeholders. Eight actual programs preserved raw bytes, POST/auth/query and MIME: Node Fetch, Go, Python Requests, both C# libraries, Rust reqwest, cURL and Wget. Kotlin, Windows cURL, HTTPie and vendor CLI execution remain unverified. Other adapters lack a usable upstream file reader and stay unavailable for binary bodies; their normal text/JSON/form/multipart language options are unchanged. Broader body/MIME/path/runtime configurations remain required.

The dialog filters binary bodies using the advertised capability and moves invalid selections to an available adapter. Owned API tests cover cross-owner rejection, default/explicit withholding of saved bytes, explicit Content-Type precedence and exact original source retention. Reproduce raw cases with the same trusted script and `--mode binary`; it selects the eight runtime cases by default. Rust uses a separate fixture build cache. Locked clean install/contextual patch replay reproduced the verified bundle; the source carries no runtime host filesystem API.


## Java native HTTP libraries and file bodies

All four original emitted Java snippets compile and make real local HTTP requests
using JDK21.0.12.1+1/Maven3.9.11/Linux.24 cases cover six profiles: JSON Unicode/
quotes/ampersand/plus; repeated URL-encoded fields and an empty value; text with CRLF,
NUL and vertical tab; raw non-UTF8 file bytes; explicit binary Content-Type overriding
the file default; and multipart repeated text fields/file bytes/MIME/filename with a
disabled part omitted. Selected Bearer auth, enabled header and existing/added encoded
query survive; the authored Host/manual Authorization are replaced correctly.

Libraries: JDK java.net.http, OkHttp4.12.0, AsyncHttpClient3.0.2 and Unirest3.14.5.
The QA driver supplies only native imports/main and client shutdown, retains original
snippet bytes and workspace/source hashes, and records resolved dependency trees,
JAR/tool/engine/source hashes and actual wire observations. Node's native HTTP server
handles framing; Python's standard email parser checks MIME parts. No generated source
rewrite, alternate request client or handcrafted multipart encoder is used.

Pinned Scalar compatibility diffs extend javaBody's existing byte-array construction
with native Files.readAllBytes/Path.of for raw files, and use native RequestBody(File)
for OkHttp. Existing native form/multipart/escaping/HTTP logic stays upstream. Java's
four binary_file capabilities are now exposed, bringing raw-file adapters to16. Only
these four verified native form adapters additionally accept repeated form fields;
unverified adapters keep their explicit rejection. Java OkHttp GET/HEAD with any body
reject before generation because OkHttp cannot send such bodies; no silent omission.

Product generation references filenames only and never reads saved file bytes or
executes target code. Default credential masking and explicit filename prerequisites
remain covered. The Java-specific localized hint explains method/main placement,
imports and selected-library dependencies; real agent-browser interaction with the
Rust API shows all four file options and credential/byte exclusion in English/light
and Chinese/dark. The request preview's gutter uses semantic gray-11 after an actual
3.86:1 dark-theme contrast failure; the rendered code now has0 verified scoped axe
violations and no375px horizontal document overflow. One manual axe check remains.

51 other adapters' representative sources remain byte-identical; the pinned patch
rebuild is idempotent. Seven prior native request-wire regressions,91 generation cases,
4 snippet/file API cases,33 focused UI/i18n cases, frontend build and scoped strict
Clippy/format/Python checks pass. Library/OS/JDK ranges, other methods/bodies/headers,
timeouts/TLS/redirect/proxy/cookie mappings, response/error handling, other language
runtimes and full generation/product/release scope remain open.

```sh
python3 tools/snippets/verify-java.py --jdk /absolute/jdk --maven /absolute/mvn --node /absolute/node --output /absolute/fresh-output
```

The driver executes only this repository's trusted fixture requests. Optional
--profiles selects core,form,text,binary,binary-mime,multipart; defaults cover all24.
Install dependencies only into explicitly selected developer QA environments; these
compilers/runtimes/dependencies are never downloaded or executed by product generation.


## Kotlin modern OkHttp and original file bytes

The original Kotlin2.4/OkHttp4.12.0 compile failed on MediaType.parse's error-level
legacy API. The pinned mature Scalar Kotlin emitter now uses toMediaTypeOrNull,
toRequestBody and File.asRequestBody. Original form/multipart/escaping/method/header
logic remains upstream. Raw binary now follows this verified Scalar reader instead
of a separate Postman rendering path; the existing binary_file capability count
stays16. Repeated forms are accepted for this native builder, and GET/HEAD with a
body reject before an OkHttp emitter could silently omit it.

Seven actual Gradle8.14.5/Kotlin2.4.0/OkHttp4.12.0/JDK21.0.12.1+1/Linux cases compile
the unchanged original snippet inside imports/main and native cleanup. Native HTTP
observations preserve POST/auth/enabled header/query, Unicode JSON, duplicate form
fields and empty value, text CRLF/NUL/vertical-tab, non-UTF8 raw bytes, explicit binary
MIME, and multipart duplicate text fields/filename/MIME/file bytes with disabled parts
omitted. A separate literal $ and ${1 + 2} profile proves no Kotlin interpolation.
Gradle locks, resolved JAR/tool/engine/source hashes and wire/runtime evidence remain
in isolated QA. Product generation still never reads host files or executes snippets.

The localized Kotlin hint names required companion imports and File; the current
Radix dialog/theme/keyboard/privacy and filename guidance are reused. Other54 adapters'
representative sources stay byte-identical.8 original Java file/multipart regressions,
92 generation cases,4 related API cases,34 focused UI/i18n cases, frontend build and
scoped strict Clippy/format/Python checks pass; patch rebuild remains idempotent.
Other Kotlin/OkHttp/JDK/OS versions, broader method/header/auth/transport/response and
all remaining language/schema/SDK/product/release requirements remain open.

```sh
python3 tools/snippets/verify-kotlin.py --jdk /absolute/jdk --gradle /absolute/gradle --node /absolute/node --output /absolute/fresh-output
```

Use a fresh output root. Imports/main and cleanup are QA host responsibilities, not
rewrites or replacement clients; dependency downloads belong only to developer QA.

Real agent-browser interaction with the owned Rust generation API confirms the Kotlin
file option, native asRequestBody code and localized import guidance in English/light
1280px and Chinese/dark375px. Saved bytes/credentials are absent from the preview;
scoped axe reports0 verified violations (one manual check per theme). No document
horizontal overflow occurs. These UI observations do not add foreign-OS runtime proof.


## Dart HTTP and Dio native body fidelity

The original Scalar Dart/http text snippet compiled but sent an empty body without
Content-Type. The verified Dart/http route now uses the already integrated pinned
Postman native Request/MultipartRequest path; Dio uses its existing Postman native
request/FormData path. Target catalog identity stays unchanged. Compatibility patches
repair raw text/UTF8, native file byte reads, repeated URL-encoded fields and native
multipart entries while preserving filenames and part MIME. The Dart-only adapter
projects HAR MIME into absent headers; native multipart encoders own their boundaries.

All14 original emitted snippets pass Dart3.13.5 analysis with fatal infos, AOT compile
and real HTTP calls on Linux. Both http1.6.0 and Dio5.11.1 cover seven profiles:
Unicode/quoted JSON, repeated form plus an empty value, text CRLF/NUL/vertical tab,
non-UTF8 raw file bytes, explicit binary MIME override, repeated multipart text/file
parts with disabled-part omission, and literal dollar/interpolation syntax. Native
Node HTTP framing and standard email MIME parsing verify requests; the generated
snippets are not rewritten or replaced. Bare snippets receive only imports/async main
and Dio resource shutdown. Saved source/snippet/engine/tool hashes, pub locks with
package checksums and AOT hashes remain in isolated QA.

Both Dart binary_file capabilities are exposed, bringing raw-file adapters to18.
Repeated form acceptance is scoped to these native paths; default credentials and
saved bytes stay excluded and explicit file preparation warnings remain. The localized
Dart hint names async placement/packages/imports using the existing dialog design.
Other53 adapters' representative source bytes stay identical; pinned package/build
patches are hash-verified and idempotent. Seven prior native wire regressions,
93 generation/4 related API/35 focused UI-i18n cases, frontend build and scoped strict
checks pass. Other SDK/package/OS ranges, arbitrary headers/methods/auth/transport/
response profiles, browser platforms and all larger generation/product/release work
remain open. Latest Dio metadata was pinned to5.11.1 rather than choosing older APIs.

```sh
python3 tools/snippets/verify-dart.py --dart /absolute/dart --node /absolute/node --output /absolute/fresh-output
```

The driver uses an explicit private pub cache and fresh output directory. It executes
only repository-authored loopback fixtures; product generation never downloads these
packages, reads host files or runs the emitted programs.

Real agent-browser owned-API interaction confirms both Dart file adapters and native
readAsBytes code with default credential/saved-byte exclusion in1280px English/light
and375px Chinese/dark. The localized async/package/import hint reuses the current
layout. Neither theme has document overflow or verified scoped axe violations; one
manual axe check remains per theme. These UI cases do not prove other OS runtimes.


## Ruby Net::HTTP literal bodies and native file IO

The original JSON heredoc consumed authored backslash/quote escapes, producing invalid
JSON in the actual HTTP observation. The pinned Scalar Ruby emitter now quotes the
original raw JSON/text through its Ruby literal helper, retaining control escapes and
protecting #{...}, #@... and #$... interpolation sequences. It does not parse/re-encode
user JSON or execute strings. Raw files use native File.binread; existing native
set_form multipart uses binary file handles and authored filename/MIME. Missing
nonmultipart Content-Type uses the supplied HAR MIME. Native repeated forms retain
all rows; other unverified adapters keep their scoped rejection.

Eight original snippets syntax-check and execute real POST requests on isolated
source-built Ruby3.4.7/Linux: Unicode/quotes JSON, repeated form/empty value, text CRLF/
NUL/vertical-tab, non-UTF8 binary, explicit binary MIME, repeated multipart text and
file bytes/MIME/filename with disabled exclusion, literal dollar text and Ruby
interpolation syntax as data. Only assertions/IO cleanup follow original source; no
request/encoder substitute or snippet rewrite is used. Wire/source/engine/tool hashes
and exact runtime records remain under the trusted developer QA output root.

Ruby file capability brings raw-file adapters to19. Generation only references names
and never reads files or sends the emitted request. Default privacy and filename
preparation remain covered; the localized Ruby hint describes standard-library usage
and binary handle lifecycle. Other54 representative adapter outputs remain byte-
identical, pinned patch rebuild is idempotent, and94 generation/4 related API/36 focused
UI-i18n cases, frontend build/scoped strict checks and prior native wire regressions
pass. Other Ruby/OS ranges, custom methods/headers/auth/transport/response behavior and
all remaining language/schema/SDK/product/release requirements stay open.

```sh
python3 tools/snippets/verify-ruby.py --ruby /absolute/ruby --node /absolute/node --output /absolute/fresh-output
```

The driver accepts only repository-authored loopback profiles and explicitly selected
QA tools. No target runtime or dependency download is added to product generation.

Real owned-API agent-browser UI interaction confirms Ruby raw-file selection/binread,
localized prerequisites and default credential/saved-byte exclusion in1280px English/
light and375px Chinese/dark. Both rendered scopes show0 verified axe violations
(1 manual check each), with no document horizontal overflow. This is not proof of
other Ruby/OS/runtime combinations.


## Python Requests and HTTPX native bodies

The pinned Scalar requestsLike emitter now converts bare JSON true/false/null
without altering quoted strings, including root false. JSON null is sent as raw
UTF-8 bytes because json=None means no body in the native libraries. Other raw
text also uses UTF-8 bytes; its MIME is retained unless an explicit Content-Type
was supplied. Raw files use pathlib.Path.read_bytes(), with data= for Requests
and content= for HTTPX sync/async. Native data=/files= builders preserve repeated
form and multipart fields. HTTPX methods without body shortcut arguments use
its generic request method when a saved body is present.

`verify-python.py` runs39 original-code syntax/native HTTP combinations:
three libraries ×13 profiles (JSON Unicode/quotes, repeated form/empty fields,
raw text/control bytes, binary, explicit binary MIME, multipart duplicate fields
and file bytes/filename/MIME/disabled exclusion, literal dollar text, nested JSON
boolean/null/string lexical traps, root false/null/empty string, GET file body,
DELETE JSON body). Native Node HTTP observes framing and bytes; the driver only
supplies package imports and an async entry point. The source/engine/snippet/tool
hashes, exact installed package versions and real wire observations are recorded.
Old embedded output reproduced nine failures across the three libraries: root
false NameError, modified JSON string contents and omitted raw text.

QA uses Python3.14.8/Linux, Requests2.34.2 and HTTPX0.28.1 in a private venv;
`tools/snippets/python-requirements.txt` pins all installed QA dependencies.
These are consumer/QA requirements, not MoleAPI runtime dependencies. Raw file
support now covers21 adapters. Generation retains filename-only source and never
reads the referenced files or embeds saved file bytes; read_bytes materializes
the file in the consumer process. Multipart open files must be closed after use.
Localized post-generation guidance explains imports/packages/async entry/file
preparation. Real owned-API Chromium checks cover English/light1280×900 HTTPX
async and Chinese/dark375×812 Requests, with no document horizontal overflow or
saved token/base64 exposure. Scoped axe reports0 verified violations and1 manual
line-number contrast check on each; screenshots were visually inspected.

95 generation cases, three owned/local snippet API cases (including all three
Python file libraries),39 focused UI/i18n cases, strict scoped Clippy, formatting,
frontend/server builds and deterministic engine rebuild passed. All50 other
non-Python representative outputs remain byte-identical. The following section records the subsequent http.client/aiohttp extension.
Other OS/dependency ranges, advanced form/file ordering/types,
streaming, remaining methods/auth/transport and full generation parity stay open.

Reproduce with an isolated Python environment and explicit tools:

```sh
/absolute/venv/bin/python -m pip install -r tools/snippets/python-requirements.txt
python3 tools/snippets/verify-python.py --python /absolute/venv/bin/python --node /absolute/node --output /absolute/fresh-output
```


## Python http.client and aiohttp extended bodies

The pinned http.client emitter now sends raw text as UTF-8 bytes, including CRLF,
NUL, vertical tab and text that parses as JSON. Raw files use native pathlib IO.
JSON still uses stdlib json, forms stdlib urllib.parse with doseq, and the native
http.client transport owns the HTTP connection. Its old multipart string joining
is replaced by urllib3.filepost.encode_multipart_formdata and RequestField for
explicit text-part MIME. Only multipart examples require the consumer to install
urllib3; ordinary examples retain standard-library-only dependencies. No file
read, package install or request execution occurs inside MoleAPI generation.

The aiohttp emitter keeps ClientSession and FormData, emits raw files/text as
bytes, preserves effective MIME, and sends root JSON null as literal bytes instead
of the native json=None sentinel. FormData uses quote_fields=False to preserve
Unicode filenames and default_to_multipart=True to keep text-only multipart.
Requests and both HTTPX emitters now use native no-filename file tuples for every
multipart text field; this also preserves source part order across files/text and
prevents text-only forms silently changing to URL encoding. Native libraries own
all boundaries and byte encoding. Canonical request validation continues rejecting
manual multipart Content-Type/boundary headers; that invariant was retained when
an invalid QA source was rejected. Raw binary capability now covers23 adapters.

The current Python driver records80 original-source syntax/native HTTP cases:
all5 Python clients ×16 profiles. The13 preceding profiles are extended with
JSON-shaped raw text, Unicode filename/explicit text+file MIME, and text-only
multipart (repeated names, empty value, disabled exclusion). Observations validate
actual method/auth/query/headers/MIME/body/file bytes through native Node HTTP and
stdlib MIME parsing; wrappers add only imports/async entry points. The pinned QA
environment is Python3.14.8/Linux, aiohttp3.14.5, Requests2.34.2, HTTPX0.28.1 and
urllib3 2.8.0, with transitive versions in python-requirements.txt. Previous output
reproduced10 failures: stdlib Unicode text/multipart errors and raw text rewriting,
aiohttp null omission, text-only MIME changes in aiohttp/Requests/both HTTPX, and
missing raw-file routes in http.client/aiohttp. Other53 representative core
adapter outputs remain byte-identical. This does not claim unchanged output for
the intentionally repaired Python multipart cases.

95 generation cases,3 local/owned snippet API cases (including all5 Python file
adapters with owner/privacy/MIME checks),41 focused UI/i18n cases, strict scoped
Clippy, formatting and production frontend/embedded server builds passed. The
embedded engine rebuild is deterministic, and a complete clean patch application
against the integrity-checked upstream archive produces exact emitter sources.
Real owned-API browser checks cover English/light1280×900 http.client multipart
and Chinese/dark375×812 aiohttp multipart: correct localized package/async/file
instructions, no saved token/base64 exposure or document horizontal overflow;
scoped axe0 verified violations/1 manual line-number contrast check per view.
Screenshots were visually inspected. Evidence: `/tmp/moleapi-python-native-final-3`
and `/tmp/moleapi-python-native-*.log` on the validation host.

Use the preceding reproduce commands with the updated pinned requirements; the
default driver now covers all5 clients/16 profiles. Multipart file resources need
consumer cleanup, and native byte reads/encoders can materialize files in memory.
Empty multipart collections, advanced escaping/part shapes, streaming,
response/error handling, dependency/OS ranges and full transport/auth option
mapping remain open; this is representative runtime evidence for the named cases.


## PHP cURL, Guzzle and Laravel native requests

The pinned Scalar PHP emitters now preserve raw JSON/text literals, including
null, empty JSON objects, dollar signs, apostrophes, backslashes and controls.
The shared mature PHP object literal helper also escapes array keys; Laravel no
longer pre-escapes keys before that helper. Repeated URL-encoded forms use mature
URLSearchParams encoding, preserving duplicate names instead of overwriting or
adding bracket indices. cURL retains ext-curl transport; PHP8.5.11 does not expose
upstream's curl_mime_init/CURLOPT_MIMEPOST functions, so its multipart block uses
Guzzle PSR7 MultipartStream, including native boundary, part headers, duplicate
fields, rb file IO and cleanup. Only multipart cURL snippets require that Composer
package; raw cURL bodies use native file_get_contents. The native handle is
released with unset instead of the PHP8.5 deprecated no-op curl_close.

Guzzle retains its native body/multipart request options and binary file streams.
Laravel retains the Http facade, withBody/asMultipart/send and native PSR7 part
arrays. Explicit filenames, field/file MIME, duplicates, text-only forms and saved
method are preserved; scalar/null/raw bodies no longer depend on array coercion.
All three support filename-only raw binary generation, bringing file-capable
adapters to26. Generation never reads host files or embeds saved Base64 bytes;
those reads occur only when the consumer executes the example. The examples need
a PHP code context (opening PHP tag for a new file), Composer autoload where
applicable, and a bootstrapped Laravel facade/application for that variant. They
are request snippets, not standalone Composer projects. Localized post-generation
hints distinguish each client's dependencies and file preparation.

The trusted native driver verifies54 unchanged-original-source cases:
3 clients ×18 profiles. These cover Unicode/quotes JSON, repeated form/empty value,
raw text/controls, raw file/effective MIME, multipart duplicates/disabled exclusion,
dollar text, JSON boolean/null/string traps, scalar false/null/empty string,
GET file/DELETE JSON, JSON-shaped raw text, text/file MIME plus Unicode/apostrophe
filename, text-only multipart, root empty JSON object and apostrophe header name/
value/backslash/dollar. Actual Node HTTP records framing/headers/body bytes; stdlib
MIME parsing checks parts. Wrappers supply only autoload, actual Illuminate
Container/HTTP Factory facade binding, and status/response assertions; no HTTP
fakes or test responses replace the real libraries. Engine/request/snippet/tool/
Composer-lock hashes and actual installed versions are recorded. Old output
reproduces10 defects: unavailable cURL MIME API, Guzzle/Laravel form repetition,
Laravel null omission, empty-object-to-array changes in all3 clients, cURL raw text
rewriting and cURL/Guzzle literal syntax failures.

QA: PHP8.5.11 NTS/Linux, ext-curl/libcurl8.22.0, Guzzle8.2.0, Illuminate HTTP/
Container13.36.0, PSR7 3.1.0; the complete48-package lock and manifest live under
`tools/snippets/php-runtime`. These are explicit consumer/QA dependencies, not
MoleAPI dependencies.96 generation cases,3 local/owned snippet API cases (including
all3 PHP raw-file libraries with MIME/owner/privacy checks),44 focused UI/i18n cases,
strict scoped Clippy, formatting, frontend/server builds, deterministic engine
rebuild and clean integrity-checked upstream patch application passed. Other52
representative adapter outputs are byte-identical.

Real owned-API browser checks cover English/light1280×900 cURL multipart and
Chinese/dark375×812 Laravel multipart, with correct dependency hints and no
credential/saved-byte exposure or document horizontal overflow. Scoped axe reports
0 verified violations/1 manual line-number contrast check on both; screenshots
were visually inspected. Evidence: `/tmp/moleapi-php-snippet-final-2` and
`/tmp/moleapi-php-snippet-*.log` on the validation host.

To reproduce, copy php-runtime's composer.json/composer.lock into an isolated
absolute directory, install that lock with the selected PHP/Composer and scripts/
plugins disabled, then run:

```sh
python3 tools/snippets/verify-php.py --php /absolute/php --node /absolute/node --dependencies /absolute/composer-project --output /absolute/fresh-output
```

HTTP_Request2 and PECL HTTP, other PHP/framework/dependency/OS ranges, advanced
empty/escaped/nested part shapes, arbitrary headers/methods, response/error/auth/
transport options and streaming remain open. Raw reads and cURL multipart string
conversion may materialize bodies in the consumer; native stream cleanup belongs
to the consumer/client lifecycle. This is evidence for the named cases, not full
PHP or request-generation parity.
