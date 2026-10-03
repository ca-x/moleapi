# Execution fix round 1 — independent re-review

Status: all six original findings are resolved in the reviewed source. One additional privacy-completeness edge case found during this re-review was fixed by the implementer and independently rechecked. The subsequent bounded-history-matcher correction has also been reviewed and verified below. No unresolved findings remain in this focused scope. This is a scoped execution review, not a release/whole-application approval.

Baseline: original findings in `tasks/implementation/execution-review.md`, initial fix package `tasks/implementation/execution-fix-review.diff`, and the live fix source in `/home/czyt/code/rust/moleapi/.worktrees/application` based on `477d22968c91b102e116d5f2233d43373678289d`. The final privacy-completeness guard was added during the review, so the live source and regression test take precedence over the initial diff package.

## Original findings

| Finding | Result | Evidence |
| --- | --- | --- |
| P1 native BigInt exceeds cooperative deadline | Resolved for server/desktop execution | Both paths now use `run_worker`; separate process, 1 second host watchdog, overdue child kill/wait, cancellation kill-on-drop. Native exponent/division regression and router pre/post timeout regression pass. |
| P1 form/query-encoded private values survive history | Resolved for the reported transport encoding | Redactor retains form serialization and percent-space variants from the full retained private-value set; query/body echo regression after post-script unset passes. |
| P1 failed post phase loses private-value tracking | Resolved | Rust callback captures values before mutation; failure IPC carries tracking metadata; stored script errors are withheld, and incomplete tracking withholds response fields. Generated and response-derived secret regressions pass. |
| P2 detached settled/rejected Promise silently succeeds | Resolved | Runtime promise hook records creation independently of final eval value/job queue; all five detached/settled test cases pass. |
| P2 negated property/value assertion checks existence alone | Resolved | Presence and optional value comparison are combined before applying negation; positive, negative, absent, null-target cases pass. |
| P2 `toObject()` loses `__proto__` | Resolved | `Object.fromEntries(Object.entries(...))` creates an own data property; targeted scope regression passes. This uses safe own-entry construction, not a null-prototype return object. |

## Additional edge case found and resolved during re-review

The first fix had `privacy_complete=false` set by the Rust tracking callback when its byte/count limit was exceeded, but only serialized that flag on a failed phase. A script could catch the callback exception and produce an ordinary success. I reproduced this through the real server worker protocol:

```js
for (let i = 0; i < 4; i++) {
  pm.variables.set('x', 'a'.repeat(1048500) + i);
  pm.variables.unset('x');
}
try {
  pm.variables.set('lost', 'new-private-'.repeat(40));
} catch (error) {
  console.log(error.message);
}
```

Before correction: worker response had `status: success`, four retained private values, and a log containing the tracking-limit error. The incomplete flag was missing.

The implementer added the final `privacy_complete.get()` guard in `crates/script-runtime/src/lib.rs` before success returns. The identical byte-limit worker input now returns `status: failure`, `privacy_complete: false`, and `Private variable history exceeds execution limit`.

I also independently exercised the count boundary through a real router, temporary database, and loopback endpoint: five scopes each supplied 1000 distinct secret values, then a post script caught the failure of `pm.variables.set('lost', pm.response.text())`. The live response still contained the endpoint's `new-private-` token, but saved history withheld body/headers/URL and did not contain the token. This confirms the new guard reaches persistence correctly, not merely a unit-level error condition. The implementer also added the byte-limit router regression `caught_privacy_overflow_cannot_commit_mutations_or_save_response_private_data`.

## Worker boundary review

- `worker.rs` starts its deadline before input serialization and spawning, checks elapsed time after those synchronous steps, and wraps stdin/write/close, bounded stdout read, child wait, and protocol decode in `timeout_at`.
- Executable paths must be explicit and absolute. Production defaults use `current_exe`; test/embedding constructors take trusted paths. HTTP input does not select executables and no PATH search is used.
- Children receive JSON through stdin, with no credentials in command-line arguments. Environment is cleared, stderr discarded, and stdout limited to 32 MiB plus a sentinel byte before decoding. Worker input is limited to 24 MiB; the separate VM input/output and heap/stack checks remain.
- The trusted worker closes out one response and exits. Error/oversize/timeout paths kill and reap the child; `kill_on_drop` covers cancellation. The targeted cancellation regression verified process disappearance on this Unix environment.
- Server main dispatches worker mode before Clap, async runtime creation, database access, or HTTP startup. Desktop main dispatches before Tauri initialization. A live server worker invocation with an entirely empty environment produced the requested log/result and exited successfully in approximately 6 ms.
- The worker binary, server binary, and desktop entrypoints all use the same dispatcher. `local_with_worker` / `hosted_with_worker` preserve a supported path for custom embeddings; API documentation explains their required dispatch setup.
- In-process `run` remains cooperative. The fixed boundary for untrusted scripts is `run_worker`, as now documented; the original 250 ms value is not represented as a preemptive native-call deadline. The authoritative host limit is 1 second plus process termination/reaping overhead.

The new runtime is an isolation boundary for execution time and JS capabilities; this review does not claim an OS syscall sandbox or a process-wide RSS cap. The configured 64 MiB limit applies to the VM heap, as stated in the updated report.

## Reviewer verification

I did not rerun the full application suite, which the root/implementer was already running. I ran these targeted checks against the current built artifacts:

1. All four worker integration tests: passed, 2.00 s total. Covers two native BigInt workloads, success/failure privacy IPC, cancellation kill/reap, oversized stdout, and stderr suppression.
2. The three repaired runtime semantic test functions: each passed. Covers detached async, prototype-named variable export, and property/value negation.
3. The three focused server tests: each passed. Covers query-encoded history after unset, failed-phase generated/response token tracking, and host-timeout pre-network abort/post-response preservation with conservative history.
4. Independent byte-cap worker reproduction: demonstrated the new defect before its final guard, then confirmed `failure` / `privacy_complete:false` after the guard.
5. Independent 5000-value count-cap router reproduction: live output preserved; persisted private token absent; uncertain saved response fields withheld.
6. Direct server private-worker dispatch with `env={}`: successful valid JSON output, no normal server startup.
7. `git diff --check`: exit 0.

Session-only repro artifacts are in `/tmp/moleapi-execution-review/`, including `taint-limit-input.json`, `taint-count-server.rs`, and `taint-count-server.output`. Product source was not edited by this reviewer. The implementer/root's full-suite and clippy runs are separate evidence; their aggregate counts are not presented as this reviewer's runs.

Platform evidence limit: cancellation and real worker execution were tested here on Linux. The desktop dispatch branch was read, but a packaged macOS/Windows/Linux GUI application was not launched by this reviewer. Root's desktop compilation/package checks remain separate gates.

Completion ledger: six original findings rechecked and closed; one additional edge case found and fixed during review, then independently rechecked; worker startup/IPC/cancellation and desktop dispatch source reviewed; report written; no product edits, commits, subagents, or public actions by this reviewer.


## Final follow-up — history matcher resource limits

Status: the additional matcher/history resource gate is closed. No new unresolved findings were identified in this final scope. The original six findings and the caught tracking-overflow correction remain addressed.

The implementer found that four overlapping approximately 1 MiB private values could make Aho-Corasick construction consume sustained CPU in the host, outside the already-fixed script worker boundary. The new `Redactor::new` checks each raw secret against 4 KiB and aggregate raw secret bytes against 16 KiB **before** producing encoding variants or compiling an automaton. It then caps aggregate encoded-pattern bytes at 64 KiB **before** matcher construction. Encoding work itself is therefore bounded by the smaller raw-input limits. Inputs above any threshold select conservative string withholding rather than partial matching or silently dropping oversized patterns.

I checked both fallback paths in `history.rs`:

- When tracking is incomplete, history withholds URL, body/base64, headers, and script-test name/actual/expected fields before persistence, masks the request name, and uses an empty redactor. It does not compile the retained large/unknown private-value set.
- When tracking is complete but pattern limits are exceeded, the redactor replaces stored response strings with the privacy-limit marker and history masks the request name. Existing binary suppression remains active. The second whole-entry scrub is skipped for this fallback so generated IDs, numeric result metadata, validated HTTP method, and creation timestamp remain usable; response text/URLs/header values/test text have already been withheld. This is a deliberate history-fidelity tradeoff, not lost privacy enforcement.

Fresh focused verification against the current built artifacts (no full-suite or build repetition):

| Check | Result |
| --- | --- |
| `privacy::tests::oversized_overlapping_patterns_are_withheld_without_compiling_an_automaton` | Passed, 0.00 s; four approximately 1 MiB overlapping patterns select withholding immediately. |
| `caught_privacy_overflow_cannot_commit_mutations_or_save_response_private_data` | Passed, 0.21 s; the exact byte-cap router case now completes promptly and preserves conservative persistence/rollback. |
| `history_redacts_query_encoded_local_values_after_post_script_unsets_scope` | Passed, 0.02 s; ordinary bounded matching still redacts encoded URL and echoed body values. |

All three commands exited 0. Root's separately reported 67-test/clippy/fmt verification remains root evidence rather than a duplicated reviewer claim. Only this report was edited by the reviewer. Final completion ledger: matcher input/encoding/build bounds inspected; both conservative persistence paths inspected; focused regression evidence independently confirmed; this additional gate closed; no product edits or commits.
