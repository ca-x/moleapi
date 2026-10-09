# Request snippet coverage

This is usable request-example generation, separate from typed OpenAPI SDK/server/CLI generation. Full Apifox/Postman parity and the complete generation module are not achieved. Engine: pinned Scalar snippetz0.10.5,22 language families/42 library adapters, embedded in Rust QuickJS with mature core-js URL/base64 polyfills. All42 adapters generate nonempty code for the representative HAR smoke case. That does not imply each variant compiles/runs or preserves every configuration.

| Language family | Libraries exposed by upstream | Actual compile/run evidence |
| --- | --- | --- |
| C | libcurl | libcurl POST fixture |
| C# | httpclient, restsharp | HttpClient and RestSharp114.0.0 POST fixtures compiled with .NET10.0.401/C#11; upstream snippets unchanged inside namespace/project wrappers |
| Clojure | clj_http | Engine smoke only |
| Dart | http | Engine smoke only |
| F# | httpclient | Engine smoke only |
| Go | native | net/http POST fixture |
| HTTP | http1.1 | Engine smoke only |
| Java | asynchttp, nethttp, okhttp, unirest | Engine smoke only |
| JavaScript | fetch, axios, ofetch, jquery, xhr | Fetch on Node runtime; browser-specific variants remain unverified |
| Julia | http | Engine smoke only |
| Kotlin | okhttp | Engine smoke only |
| Node.js | fetch, axios, ofetch, undici | Fetch POST fixture |
| Objective-C | nsurlsession | Engine smoke only |
| OCaml | cohttp | Engine smoke only |
| PHP | curl, guzzle, laravel | Engine smoke only |
| PowerShell | webrequest, restmethod | Engine smoke only |
| Python | python3, requests, aiohttp, httpx_sync, httpx_async | python3/http.client POST fixture |
| R | httr2 | Engine smoke only |
| Ruby | native | Engine smoke only |
| Rust | reqwest | reqwest POST fixture, generated snippet wrapped in async main |
| Shell | curl, wget, httpie | cURL POST fixture |
| Swift | nsurlsession | Engine smoke only |

TypeScript/C++ dedicated emitters remain required; JavaScript/C snippets are not relabeled as those languages. SDK/server framework generation, protobuf stubs, generated CLI and repeatable multi-file manifests/diffs remain separate required work.

## Behavior and current limits

The saved HTTP request becomes HAR via the shared Rust adapter. Query encoding, selected auth replacing manual Authorization, transport-owned header removal, text/JSON/urlencoded bodies and Content-Type defaults have meaningful regressions and real fixture evidence. Unsupported protocol/body/auth modes return errors. Upstream library source owns formatting and escaping; MoleAPI does not implement language templates.

Environment placeholders remain static, with no runtime resolution or pre-script execution. Some libraries expect complete runnable programs, others emit snippets for an existing function/project. Required target libraries/runtimes must be supplied by the user. Python/http.client requires a concrete absolute HTTP(S) URL because upstream otherwise invents example.com for a base template. URLs with literal quote/backslash/newline characters are rejected rather than emitting unsafe upstream quoted source. Header/query/body placeholders still require review before execution.

Repeated form/header names are explicitly rejected outside verified shell/cURL; this avoids silent losses in dictionary-based upstream variants. Fetch-family JS/Node GET/HEAD bodies are rejected. Timeouts, TLS verification and redirect options are not yet mapped and this is stated in returned warnings. Shared multipart/files/binary execution is now implemented. Snippet generation and file cURL templates explicitly reject these body modes until their mature generator mappings and actual file semantics are verified.

Default previews/downloads exclude known private values and credential-bearing fields. User-defined JSON keys/scalars and decoded query/form fields are screened; fixed request schema keys remain intact. Literal/base64/form/path variants of known values and the current Basic tuple are matched. Full Standard/URL-safe-no-pad Base64 string values or explicit Basic headers up to16KiB are decoded with the mature crate and withheld if decoded UTF8 contains a private value. Arbitrary transformed encodings, mixed opaque blobs and larger encoded fields are not claimed detectable. Explicit include-secrets is visible and preserves saved values; browser-only overrides are never attached.

Generation does not execute or publish output. Dialog owner/content/target/credential/open fences discard stale results, including equivalent save-key reorder handled by mature stable JSON serialization. Four actual worker-held slots, host-free QuickJS memory/stack/time/input/output caps and compile-time bundle digest/version verification are enforced. Native IPC uses the same service module; native graphical/platform packages still need final integration validation.

## Reproduce

`cargo test -p moleapi-generation --locked` exercises all42 upstream adapters plus privacy/representability regressions. `cargo test -p moleapi-server --test generation --no-default-features --locked` exercises owner/default/private/native routes. `npm --prefix web test -- --run src/features/generation/GenerationDialog.test.tsx` exercises save order, late owner responses and stable content identity.

`python3 tools/snippets/verify-fixture.py` uses only trusted synthetic requests, starts a local fixture, generates through the actual saved-request adapter, compiles/runs seven representative programs and checks real POST/auth/header/query/Unicode JSON. It needs Node/Python/Go/C+libcurl/Rust and cached Rust dependencies. No Docker is used. Generator sources/licenses/reproducible locked build are documented in vendor/snippet-engine/README.md. Fresh independent review is in tasks/implementation/snippets-review.md; broader generation/feature work remains active.


C# validation used the actual saved-request adapter and also the owned saved-request HTTP API. Both emitted variants preserved method, selected Bearer replacing manual Authorization, custom headers, query encoding and Unicode/quoted JSON in real requests. The HttpClient snippet needs System.Net.Http/System.Net.Http.Headers; RestSharp needs its NuGet package/namespace. JSON raw string literals require C#11+. These are snippets for an async method or a compatible top-level-await project, not standalone project archives. Reproduce only these variants with `CARGO_TARGET_DIR=/path/to/cache python3 tools/snippets/verify-csharp.py --dotnet /absolute/path/to/dotnet`; .NET SDK/NuGet restore or cache is a validation prerequisite, not a MoleAPI runtime dependency.

Downloads now use language source extensions (including .go/.cs/.sh) and the shared browser/native file saver. Native dialog selection uses per-file write permission; owner/content/open checks prevent stale writes after the picker resolves. Static warnings and C# dependency hints have English/Chinese UI translations. Timeout/TLS/redirect/body-mode limits above remain real gaps; successful fixtures do not imply every target/library or option has been validated.
