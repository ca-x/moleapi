# Interface request code language/library parity

Sources checked2026-10-09 with agent-browser: [Apifox request-code list](https://docs.apifox.com/generate-code.md) and [Postman request-code list](https://learning.postman.com/docs/use/send-requests/create-requests/generate-code-snippets/). Official Postman code-generators2.1.1 provides35 variants; the merged Scalar0.10.5/Postman catalog exposes23 families/55 variants, including extra Scalar adapters and Apifox-compatible JavaScript aliases.

| Language | Available HTTP libraries |
| --- | --- |
| C | Libcurl |
| C# | HttpClient, RestSharp |
| Clojure | clj-http |
| Dart | Http, dio |
| F# | HttpClient |
| Go | NewRequest |
| HTTP | HTTP/1.1 |
| Java | AsyncHttp, java.net.http, OkHttp, Unirest |
| JavaScript | Fetch, Axios, ofetch, jQuery, XHR, Native (Node.js), Request (Node.js), Unirest (Node.js) |
| Julia | HTTP.jl |
| Kotlin | OkHttp |
| Node.js | Fetch, Axios, ofetch, undici, Native, Request, Unirest |
| Objective-C | NSURLSession |
| OCaml | Cohttp |
| PHP | cURL, Guzzle, Laravel HTTP Client, HTTP_Request2, pecl_http |
| PowerShell | Invoke-WebRequest, Invoke-RestMethod |
| Python | http.client, Requests, aiohttp, HTTPX (Sync), HTTPX (Async) |
| R | httr2, httr, RCurl |
| Ruby | net::http |
| Rust | reqwest |
| Shell | Curl, Wget, HTTPie, cURL (Windows cmd.exe) |
| Swift | NSURLSession |
| Postman CLI | Postman CLI |

The reference request-code lists are covered by selectable adapters. Postman CLI output requires the vendor CLI only when that output is selected; native MoleAPI generation does not execute it. Windows cURL uses the official generator double-quote/single-line options and .cmd download. Node Native/Request/Unirest output uses CommonJS/.cjs and may require dependencies shown in its require statements.

All55 variants passed actual bounded Rust QuickJS generation. Node Native/Request/Unirest additionally sent real POST/auth/header/query/Unicode JSON via the saved-request adapter. This exposed upstream double-query-encoding in Node Native, corrected by a reproducible minimal patch using official postman-url-encoder3.0.8. Other runtime cases remain explicitly recorded in SNIPPET-COVERAGE.md; output availability is not every-configuration compilation evidence.

The Apifox business SDK/server130+ framework list and19 model languages are separate generation categories. The19 model families are now integrated via Quicktype and OpenAPI Generator; see MODEL-GENERATION-COVERAGE.md for exact actual evidence and remaining limits. Existing pinned OpenAPI Generator catalog has165 nondeprecated targets; target availability, per-language usable project evidence and full template workflows remain required and must not be relabeled as complete interface-request parity.
