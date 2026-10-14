# PHP array, promise and HTTP-info contracts

Parent: generated-php-static-types-plan.md and milestone2.

Extend model aliases to one-level model arrays, all request method parameter annotations,
sync result/http-info tuples and async promises. Ship a PHPStan-only generic PromiseInterface
wait stub plus config through mature supporting-file generation. Preserve actual Guzzle
runtime and optional user template/output precedence; product never installs/runs PHPStan.

Write code/consumers first: list/echo Cat/Dog arrays, async direct/array methods, HTTP-info
and async-info status/header tuples, model/mixed/static negative arguments. Verify PHPStan
using shipped config and exact wait/tuple/item types; execute actual async/wait/list calls
for3.0/3.1 and original tag-name/file/core regressions. Run override/generation/API/lint/
format gates, document one-level/wait scope and remaining nested/callback/other-analyzer/
nominal/platform work, commit/merge/push verified slice.


Verified one-level arrays, all request parameter forms, sync/async info tuples and wait
results with PHPStan2.3.1 positive/exact-six-negative consumers on both dialects.28 real
async/list/tuple requests plus24 core/file regression requests passed.13 override cases
include explicit stub/config source/destination priority.78 generation/11 API cases and
strict scoped Clippy/formatting passed. Nested/map/callback/chaining/rejection generic
inference, other analyzers/hierarchy/platforms remain open in parent scope.
