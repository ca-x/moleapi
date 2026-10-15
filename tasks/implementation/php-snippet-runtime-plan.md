# PHP request snippets: cURL, Guzzle and Laravel

Continue milestone2 using the pinned Scalar emitters; HTTP_Request2 and PECL HTTP remain later native targets.

- PHP8.5.11 environment inspection proves curl_mime_init and CURLOPT_MIMEPOST do not exist. Retain ext-curl transport, replacing only its unusable multipart block with Guzzle PSR7 MultipartStream. Disclose that multipart Composer dependency explicitly. Do not write a MIME encoder.
- Reuse mature PHP literal formatting for URL/headers/auth/raw JSON/text, native file reads and multipart part metadata. Raw JSON strings must preserve scalar/null/empty object semantics; raw text that parses as JSON stays verbatim. PHP $ interpolation and apostrophe/backslash escaping must stay literal.
- Guzzle and Laravel retain native HTTP clients and PSR7 multipart facilities. Preserve repeated form names through mature URLSearchParams encoding and original ordered multipart arrays, with explicit MIME/filename and rb file streams. Keep raw-file generation filename-only and privacy/ownership invariant.
- Write all implementation and native validation code before batch tests. Trusted canonical fixtures cover3 libraries×16 existing Python body profiles plus root empty object and quoted header profiles; original generated code is only wrapped with Composer autoload/real Laravel facade bootstrap. Native Node HTTP observes exact bytes/framing, with syntax checks and immutable source hashes.
- Pin current Guzzle8.2.0/Illuminate13.36.0 and full Composer lock in isolated QA, recording runtime/library/tool hashes. Other dependency/OS ranges remain distinct.
- Apply ui-ux-pro-max and emil-design-eng to post-generation PHP dependency/namespace/file hints using existing typography/theme/focus and natural wrapping.
- Batch native runtime, upstream failure contrasts, generation/API/UI regressions, deterministic and clean upstream patch checks, scoped fmt/Clippy/build, actual owned-API bilingual UI. Commit explicit files, cherry-pick verified changes to main, push both branches. Preserve existing drafts and pending Apple CI commits. Full395-feature objective remains active.

## Verified checkpoint

54 unchanged-source syntax/native HTTP cases passed on the actual PHP8.5.11/ext-curl8.22/Guzzle8.2.0/Illuminate13.36.0/PSR7 3.1.0 clients.10 previous defects reproduced; current Laravel13 scalar false already worked, so no false failure assertion was retained. A verifier profile classification mistake was corrected using the actual Content-Type observation; all54 cases were then rerun.96 generation/3 API/44 UI-i18n cases, scoped strict Clippy/fmt and frontend/server builds passed. Entire clean upstream patch matches exact emitter sources and engine rebuild is deterministic;52 other representative outputs stay byte-identical. Actual owned-API bilingual desktop/mobile UI and inspected screenshots preserve hints/privacy/reflow; scoped axe0 verified violations/1 manual contrast check. Evidence: `/tmp/moleapi-php-snippet-final-2` and `/tmp/moleapi-php-snippet-*.log`.

HTTP_Request2/PECL HTTP, broader dependency/platform/advanced body/transport/response scopes and the full remaining plan are still open. Preserve the independent Apple native CI gates.
