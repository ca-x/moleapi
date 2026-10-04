# TCP final scoped review

Reviewed 2026-10-04 against HEAD `23131862eee43aff944460ecee30bc8a87daa02f` plus the uncommitted TCP implementation. This is a pre-fix review; subsequent implementer edits require focused re-verification.

Scope: `docs/specs/tcp-client.md`, implementation plan and SDD ledger; core TCP DTO/validation/interpolation; protocol actor, codecs, TLS, pinned addresses, metrics, limits and session integration; server endpoints; native export redaction; TCP React workbench and shared selector/session/events integration; new TCP tests. No source, fixture, process, Docker, or main-scaffold changes were made by this reviewer. Only this requested report was written.

## Findings

1. **P1 — Default export does not screen sensitive fields inside encoded payloads.** `crates/formats/src/redact.rs:60-68` retains any valid Hex/Base64 source whose decoded bytes do not match a value already collected elsewhere in the workspace. A payload encoding `{"password":"only-in-this-payload"}`, with `secret:false` and no copy of that password in other workspace fields, therefore survives `include_secrets=false`. The equivalent text payload is screened by `redact_embedded_json`, so selecting Hex/Base64 bypasses the default-export privacy behavior. `ExportPrivacy::from_workspace` does not parse TCP message sources when collecting secrets. This conflicts with the spec's exclusion of unsafe binary drafts. Conservatively withhold non-template binary drafts, or screen decoded recognized content and withhold opaque content; retain exact originals for explicit include-secrets export. Add Hex and Base64 regression cases with a sensitive JSON field whose value occurs nowhere else. Evidence: direct source trace; the existing export test only covers a literal duplicated in a flagged global variable.

2. **P2 — DNS/connection latency consumes the idle timeout before the connection opens.** `crates/protocols/src/tcp.rs:201` initializes `activity` before `checked_destination` and all connect attempts; successful plain TCP connection at lines 255-258 does not reset it. `exchange` uses an interval whose first tick is immediately ready and compares elapsed activity against the idle timeout. With a 100 ms idle timeout and DNS/connect taking more than 100 ms, a successful new connection immediately becomes an idle error before the user can send. Reset activity on completed connection/TLS establishment so the idle budget measures the open connection. Evidence: direct control-flow trace; current loopback idle test has near-zero connect latency and does not exercise this boundary.

3. **P2 — Newly added focused frontend suite currently fails.** `web/src/features/tcp/TcpWorkbench.test.tsx:17` queries exact accessible tab name `发送报文`; Radix under this jsdom setup exposes `发送报文发送报文`. The late-send test fails before assessing its assertion, even though the DOM shows `aria-selected="true"`. Use a robust query or explicit accessible label and rerun. This is a verification failure, not evidence that stale-send protection itself fails.

## Verification and scope limits

- Independently ran `cargo test -p moleapi-core tcp --locked`: exit 0, two TCP unit tests pass.
- Independently ran `npm --prefix web test -- --run src/features/tcp/TcpWorkbench.test.tsx`: exit 1, two pass and one fails as described above.
- Inspected the real socket test coverage for raw Hex, BE/LE fragmented/coalesced frames, CRLF, measured counters, half-close continuing reads, secret fragmented echo, idle/cancel, declared oversized lengths, empty-frame flood, actual certificate verify/skip, owner/private policy and workspace deletion. Root owns fresh execution of these suites, strict Clippy, frontend build, embedded UI and actual agent-browser QA; their earlier reported passes are not represented here as independent fresh runs.
- Dedicated behavioral tests for the 512-command boundary, 32-command queue saturation, 20 MiB aggregate/wire boundaries, successful Base64 send, scoped-private variable taint without explicit secret-send flag, and cancellation while a send is blocked were not present in the reviewed new test files. These are evidence gaps against the spec, not independently demonstrated implementation bugs.
- No additional concrete failure found in pinned destination use, official framing codec use, cancellation selection, half-close ordering/rejection, successful-write accounting, receive frame quota, secret-send taint ordering, owner/deletion integration, or React identity guards on the reviewed paths. This does not substitute for the final runtime/browser gates.

## Completion ledger

1. Done — requested source/spec/test review and concrete findings sent to implementer.
2. Done — focused independent core/frontend checks and this report.
3. Remaining with implementer — resolve findings, regression checks, refreshed final runtime/build/Clippy/browser gates, and focused post-fix review.


## Implementer fix evidence (not a second independent approval)

Root fixed P1 with native Hex/Base64 sensitive-field/copy regression (failed before fix, passed after), mature decoded JSON screening plus opaque binary withholding; explicit full originals remain exact. P2 idle regression uses real localhost DNS queued behind a occupied blocking worker, fails before open-time reset and passes after. Radix selector now tests active pane with robust accessible-name matching and actual stale ownership/half-close behavior. One fix pass, as required by executing-plans; no second reviewer run.

Evidence gaps closed by meaningful public-service fixtures:512 zero-byte commands,32 queue saturation while writer blocked plus cancel,20MiB aggregate successful Base64/wire and incoming wire cap, scoped private variable without secret-send flag, and GUI Base64/Hex send. Root final225 Rust/98 UI/strict/build/browser gates passed. Empty private variable falsely hid default public packets; regression failed then passed with nonempty-value admission. Equivalent saved JSON key order falsely retained dirty drafts; shared workspace regression failed then passed using existing mature stable serialization, while prior concurrent-edit tests remain green.

Original pre-fix findings remain above as historical review evidence. Native/latest platform distribution and broader TCP/product gaps remain explicit; this addition does not claim independent reviewer re-approval or full parity.
