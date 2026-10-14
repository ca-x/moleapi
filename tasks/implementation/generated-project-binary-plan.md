# Generated Project Binary Runtime Plan

Parent: `docs/specs/generated-project-runtime-fixtures.md`.

1. Add an explicit binary QA profile to the trusted fixture generator and runtime harness, retaining the existing core profile. Include authenticated octet-stream download and upload with an integer receipt; use bytes containing NUL, CR/LF, invalid UTF8 and high-bit bytes.
2. Author real default-library JavaScript/Python/Go/TypeScript/C# upload/download consumers. Check exact raw bytes and Content-Type/Bearer settings on the loopback wire, plus existing JSON/query/header/enum/nullable/error checks. Go retains race checking. Generated code remains mature engine output and is never run during application preview/download.
3. Execute finished cases for3.0/3.1, diagnose concrete target/integration failures, and fix necessary engine/template issues without changing imported source or switching to easier libraries. Record exact target/profile/tool evidence and remaining file/multipart/streaming/union limits, run generation/lint/format gates, then commit/merge/push verified work.

Finding/fix: actual JavaScript File responses were not recognized by upstream ApiClient binary parsing and fell through text handling. Added a pinned Apache-2.0 default-library ApiClient compatibility copy using the existing transport/parser. Explicit templates still override it. C# consumers use the emitted FileParameter API rather than passing a bare Stream.

Verification: all10 client/dialect binary-profile combinations passed (50 actual HTTP calls); Go race checks passed. Generation regression60 passed/8 configured external-tool cases ignored; JavaScript ApiClient and ASP.NET Startup override cases passed explicitly; scoped strict Clippy and format/diff gates passed. Browser/multipart/streaming/range/error-binary and remaining target/keyword/platform work stay open.
