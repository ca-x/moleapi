# PHP promise callback and rejection contracts

Parent: generated-php-container-async-plan.md and milestone2.

Extend PHPStan-only PromiseInterface stub for then/otherwise: callbacks receive TValue,
returned value/promise inference is adopted, absent fulfilled/rejected callbacks retain
original/no-recovery types, otherwise retains fulfilled type or recovery type. Keep actual
Guzzle runtime unchanged unless actual SDK rejection handling reveals a concrete defect.

Finish source and positive/negative/static/runtime consumers before tests. Verify mapped
callbacks, absent handlers, promise adoption, rejection recovery and thrown callback errors;
actual async400 must retain typed ApiException/Failure behavior consistent with sync.
Run both dialects and previous async/list/tuple plus core/file regression, supporting override,
generation/API/lint/format gates. Document exact callback/wait scope, then merge/push.


Verified both dialects with PHPStan2.3.1 callback/adoption/identity/recovery positives and
exact7 argument.type negatives. Actual async400 lacked typed Failure; repaired shared
response metadata partial and no-response transport guards.40 HTTP requests plus closed-
port checks and24 core/file regressions passed.14 overrides/78 generation/11 API cases,
strict scoped Clippy/formatting passed. Cancellation/mixed rejection policy/nesting/other
analyzers/hierarchy/platform coverage remains explicit parent scope.
