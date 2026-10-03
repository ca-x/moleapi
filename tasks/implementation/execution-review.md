# Execution extensions independent review

Status: changes require fixes before the bounded-runtime and history-privacy guarantees are met. Three P1 findings and three P2 findings were reproduced. Review baseline is `477d22968c91b102e116d5f2233d43373678289d`; implementation is the current uncommitted/untracked work, not an absent implementation.

Scope: `crates/core`, `crates/server`, and `crates/script-runtime`, against `docs/specs/execution-extensions.md` and `tasks/implementation/execution-report.md`. This review did not edit product source, create commits, or delegate further work. Frontend, formats, and richer future auth/body families remain outside this review. The versioned pm subset is intentional; unsupported `sendRequest` is not itself a finding.

## Findings

### P1 — Native BigInt work bypasses the advertised 250 ms execution deadline

Location: `crates/script-runtime/src/lib.rs:48-50`, `83-90`, and `120-124`; `crates/server/src/execution.rs:59-73`.

Reproduce through `moleapi_script_runtime::run` with an otherwise valid empty GET request and default scopes:

```js
(2n ** 1000000n).toString();
```

Observed: `Ok(ScriptOutput { ... })` after **3.372437663 seconds**, rather than an interrupted phase. A variant under 100 bytes containing three conversions remained running until an external subprocess timeout forcibly killed it after six seconds:

```js
const n = 2n ** 1000000n; n.toString(); n.toString(); n.toString();
```

The configured QuickJS interrupt is cooperative, not preemptive. The actual dependency source, `rquickjs-sys-0.14.0/quickjs/quickjs.c:13627` (`js_bigint_to_string1`), executes a repeated multi-precision division loop without polling the interrupt. Its normal VM poll also decrements a counter before calling the Rust handler (`quickjs.c:8650-8666`). The Rust wrapper neither checks elapsed time after native calls nor has a mechanism to terminate native work. The operands are below QuickJS's BigInt size cap and allocate far below 64 MiB, so memory limits do not prevent this trigger. Several such phases can occupy all four shared script slots for far longer than the promised budget.

Fix requires an actual bound on these native operations, for example a terminable process boundary or verified runtime/native-operation restrictions. An elapsed-time check prevents incorrectly reporting success, but alone does not reclaim CPU. A timeout around `spawn_blocking` also does not stop that worker. Include native BigInt conversion in deadline tests, not just tight JS loops and regexp backtracking.

### P1 — URL/form-encoded private values are saved in history

Location: `crates/server/src/privacy.rs:10-24`, `crates/server/src/history.rs:43-49`, and `crates/server/src/execution.rs:180-189`.

Reproduction used the actual local router, a temporary database, and a loopback HTTP endpoint returning its raw query string:

1. Store project variable `credential` with shared value `shared` and `local_value = "private value/+?"`.
2. Execute a request with query pair `lookup={{credential}}`.
3. Use post-response script `pm.globals.unset('credential');`.
4. Read `/api/workspaces/script/history`.

Observed history contains the complete private value as `private+value%2F%2B%3F` in `url`, `response.url`, and `response.body`. Without the unset, the body still leaks the same encoding; the URL is then redacted correctly.

The redactor retains literal, JSON-string, and URL-path representations only. Reqwest's query serializer uses form encoding, with `+` for spaces and percent escapes for `/`, `+`, and `?`, so none of those patterns match. URL-aware redaction only receives the effective environment *after* post scripts; after unset/replacement it cannot identify the old value in a nonsensitive query key such as `lookup`. `private_values` retains the old literal, but its encoding patterns remain insufficient.

Redact URL query values against the complete retained private-value set, independently of current scope contents, and cover the form/query encoding used by the transport in generic response/history redaction. Add regressions both with and without post-phase unset and with hosted `locals`.

### P1 — Failed post phases persist newly generated private values in script errors

Location: `crates/script-runtime/src/lib.rs:83-84`, `crates/server/src/execution.rs:150-172`, and `crates/server/src/history.rs:49`.

Use a successful HTTP endpoint and this post-response script:

```js
pm.variables.set('issued', 'generated-' + Date.now());
throw new Error(pm.variables.get('issued'));
```

Observed: execute returns HTTP 200 with the intended failed post-script test, but history stores `response.tests[0].actual = "JavaScript error: generated-1790984205957"` in full. The value is generated during execution, not present in a shared workspace field before execution.

Scope updates are returned and added to the private-value set only on successful phase completion. A failed phase discards its mutations and privacy metadata, yet its raw error is added to persisted test results. Clearing `stored.logs` does not help because the same text is also in `stored.tests`. The ordinary redactor cannot mask a value it never received.

Keep privacy tracking independent of committing mutations, including on exception/output-limit/deadline paths, or use a conservative persistence policy that does not save untrusted error details/affected response contents when the phase's privacy metadata is unavailable. Mutation rollback itself should remain intact. The same issue can expose a newly issued response token when a script assigns it to a variable and then fails.

### P2 — Detached rejected async work silently succeeds

Location: `crates/script-runtime/src/lib.rs:83-88` and `120-123`.

```js
(async () => { throw new Error('async failed'); })();
42;
```

Observed: `run` returns `Ok`, with no logs and no failed tests. This is the exact current runtime, not an inferred behavior. The code only rejects a Promise when it is the final evaluation result or when the job queue is nonempty. A detached already-rejected Promise satisfies neither check. A collection containing this pre/post script can therefore be counted as passed and networking proceeds despite the claimed explicit rejection of async scripts.

Either enforce the documented unsupported-async boundary consistently or track/reject unhandled Promise rejections and make the narrower accepted behavior explicit. Add a regression with a trailing non-Promise expression; the existing `.then(...)` case only exercises queued jobs.

### P2 — Negated property/value assertions implement the wrong condition

Location: `crates/script-runtime/src/pm.js:131-134`.

```js
pm.test('different property value', () =>
  pm.expect({x: 1}).not.to.have.property('x', 2));
```

Observed: failed test with `Assertion: expected {"x":1} not to have property x`. The supported property/value assertion should succeed because the object does not have `x` equal to `2`. The implementation applies negation to property existence alone and throws before comparing the supplied value; `check` also consumes the negation before the second check. This can falsely fail imported tests and runner results even though `property` and negation are advertised supported APIs.

Evaluate existence plus the requested value comparison as the relevant assertion condition, with correct handling of absent properties and negation. If some combinations are deliberately unsupported, reject them explicitly rather than returning an incorrect assertion result.

### P2 — `toObject()` drops an accepted prototype-named variable

Location: `crates/script-runtime/src/pm.js:33`.

```js
pm.environment.set('__proto__', 'proto-value');
pm.test('toObject preserves key', () =>
  pm.expect(pm.environment.toObject())
    .to.have.property('__proto__', 'proto-value'));
```

Observed: failed test, with the serialized object `{}`. `get('__proto__')` succeeds, and the mutation is retained correctly, but `Object.assign({}, scope)` invokes the normal object's legacy prototype setter instead of creating an own property. All exposed variable stores share this implementation, including the read-only data scope. Null-prototype internal storage fixes the existing get/set test but not this export path.

Create the returned object with a null prototype or use own-property creation semantics, and test both initialized and script-set prototype-named values through `toObject`/enumeration.

## Verification and covered behavior

Fresh reviewer command:

```sh
cargo test -p moleapi-core -p moleapi-server -p moleapi-script-runtime --no-default-features
```

Result: **47 passed, 0 failed**, exit 0. This includes core bounds/network/scope tests, runtime loop/allocation/regexp tests, actual script/network/history integration, hosted ownership/local overrides, runner chaining, and sync preservation. No configured external PostgreSQL/MySQL service was separately exercised by this review, and prior CI results are not represented as new reviewer evidence. The default-feature suite and clippy results in the implementation report were not rerun here.

Independent repro harnesses were written outside the checkout under `/tmp/moleapi-execution-review/` and compiled against the built current Rust libraries. `runtime.rs` accepts a JavaScript source path; `server.rs` drives the encoding/unset case; `failed-phase.rs` drives the failed-phase taint case. `compile.py` records matching dependency fingerprints for the server repros. Product source was unchanged. The snippets and API sequences above remain the durable reproductions; these temporary binaries are session evidence, not project deliverables.

Additional live checks: `Atomics.wait` errors immediately (`cannot block in this thread`), and a thrown object's looping `message` getter was interrupted around 250 ms. This is not a general native-operation deadline proof. Inspecting QuickJS's allocator confirmed its allocation-limit checks; the confirmed bypass concerns CPU interruption, not a demonstrated memory-limit escape.

Source review found the intended project → collection → request pre order and reverse post order, temporary > data > environment > collection > project precedence, isolated contexts per phase, source-code exclusion from interpolation, post-error HTTP response preservation, owner checks, transport network-policy reuse, successful runner mutation carryover, hosted write scrubbing, native upload scrubbing, and stable-identity native pull preservation. No additional findings are asserted for those paths. Live logs/variable updates intentionally contain private values; that explicit ephemeral behavior is not itself a privacy defect.

Completion ledger: scoped review done; fresh native/no-default suite done; six findings reproduced and reported; report written; source fixes and verification after fixes remain with the implementation owner; no commits or public actions performed.
