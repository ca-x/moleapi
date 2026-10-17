# Bounded Mock response delay coverage

Scope: `mock-007` and the delay portion of `mock-013`, `docs/specs/mock-response-delay.md`, branch `feat/mock-delay` stacked on the Mock call log branch. Existing examples default to zero delay.

Saved examples accept `delay_ms` from 0 to 30,000. Both direct and routed Mock responses are constructed before the configured wait; dynamic scripts keep their own one-second worker deadline. Up to 32 waits run concurrently on one server; excess requests receive 429 without the saved body. A workspace revision/owner recheck after the wait prevents serving stale static or dynamic responses. Mock call logs include the wait in `elapsed_ms`. Full and default MoleAPI backups preserve the delay; external formats reject it because they cannot preserve latency behavior.

Local evidence: an invalid delay was rejected and old example JSON retained a zero default; the native backup roundtrip preserved 120 ms while external exports failed explicitly. Real HTTP tests measured static and dynamic delays, verified logged elapsed time, showed 48 concurrent requests admitting at most 32 and logging each 429, and rejected a workspace edit during an 800 ms static wait. Core 146, formats 89 and server 320 tests passed with eight existing ignores and one local Java catalog test excluded because the pinned OpenAPI Generator JAR was not prepared. Frontend 361 tests, production web/embedded server builds, scoped strict Clippy and formatting passed.

Actual hosted browser at 390px: imported a saved example, set and saved 180 ms, reopened the workbench and saw the same value; an authenticated Mock GET returned status 202 and body `delayed` in about 188 ms. English/Chinese editor labels rendered at document width 390px without horizontal overflow. Independent review, configured three-database CI and current-head native package checks remain open.

Remaining Mock goals include schema-driven fake data, public permissions and richer failure scenarios. This delay slice does not close the full Mock module or the 395-row product matrix.
