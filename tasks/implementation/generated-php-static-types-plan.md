# PHP mapped-model static annotations

Parent: generated-project-php-plan.md and milestone2.

Preserve runtime classes/signatures and fix mapped-oneOf static contracts with mature
PHPStan local type aliases. Every generated model exports ModelValue (self for ordinary
models, mapped class union for mapped oneOf). API class imports aliases per operation/
parameter with unique names, avoiding Mustache parent-context discriminator leakage.
Direct non-container model parameters/returns use PHPStan tags; existing standard PHPDoc
and runtime serializer remain unchanged. Arrays/async/other analyzers and nominal
parent inheritance remain explicit gaps until separately covered.

Write templates and typed consumers before tests. Prepare an isolated Composer PHPStan
QA tool; product generation never installs it. Verify Cat/Dog typed arguments/returns,
ordinary Pet/Failure models, mixed-model request/response (no mapping leakage), original
core/file/union wire fixtures and explicit header/API overrides. Run generation/API/lint/
format gates and record exact PHPStan/template scope before merge/push.


Verified both dialects with PHPStan2.3.1 level7 positive assertions and exact2 negative
argument.type errors per project, plus14 real union requests.12 explicit overrides,
78 generation/11 API regressions and strict scoped Clippy/formatting passed. Runtime
signatures/hierarchy unchanged; direct PHPStan annotations close this contract slice.
Arrays/async/http-info/other analyzers and nominal hierarchy remain open explicitly.
