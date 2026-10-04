# Independent MCP backend review

Reviewed the frozen, unstaged Rust/core/protocols/server/Cargo/rmcp changes against base `6ea9b18`, `docs/specs/mcp-client.md`, the implementation plan and backend brief. Frontend/formats remain root-owned. No source edits, commits, pushes, Docker or existing QA-process changes.

## Material findings

1. **P1 — MCP configuration variables prevent connection.** `crates/server/src/protocols.rs:98–108` exempts MQTT/Socket.IO/GraphQL/gRPC from comparing the entire resolved protocol to its original, but omits MCP. `prepare_live` resolves MCP command, args, env, name and URI through `resolve_request`; a saved STDIO env value `{{token}}` therefore differs after resolution and returns `Pre scripts cannot change the live protocol` before launch. HTTP name/URI templates suffer the same problem. This violates actual scoped-variable transport support. Compare protocol identity for MCP too, while retaining the prohibition against changing the protocol kind; add real SDK connection coverage with templated STDIO env/args and preserved originals. Static call-path finding; existing tests do not exercise templated MCP config fields.

2. **P2 — The documented 512-command session quota is not enforced.** `crates/protocols/src/mcp.rs:346–360` validates only the accumulated byte budget, queues the command and increments `grpc_input_messages`; it never compares that counter to 512. Arbitrarily many small `mcp_cancel` commands can exceed the stated work quota while remaining well below 20 MiB, with no result events to exhaust the event quota. Enforce a per-session count before enqueue and verify that failed enqueue does not consume admission. Cover the exact limit and the 513th tiny command. The current report's claim of 512 commands is incorrect.

3. **P2 — Some server-controlled event text bypasses private-value masking.** `crates/protocols/src/mcp.rs:59–65` writes a custom notification's method straight into the event even though the SDK allows a server-controlled method; a method containing a captured credential appears unredacted in the console/status label. Additionally `safe_value` at lines 34–47 scrubs only string values: object keys (and reflected private numeric/boolean scalars) remain raw in tool structured content/error data/callback schemas. Apply the mask to server-controlled event labels and screen JSON keys/private scalar representations with an explicit collision/withholding policy. Extend the official SDK privacy fixture beyond ordinary string-value echo. This is a scoped MCP finding, not a broad audit of the existing generic redactor.

## Verified boundaries

Official rmcp 3.5.0 transport/model/client handling is used. Checked HTTP pins every POST/GET/DELETE through the shared DNS/address policy, preserves hostname/TLS configuration, disables proxy discovery and rejects redirects. Resource identifiers are forwarded through MCP without independent fetch. Hosted STDIO is denied unless exact administrator-allowed executable path; native launch is explicit, with separate argv, cleared environment, discarded stderr and owned process group/job cleanup. Catalog pagination and response/input size/depth bounds, manual callback pending limits and owner-linked session cancellation are present. The supplied SDK fixtures use their own ephemeral listeners and child processes.

Intentional exclusions accepted: absolute executable paths; rejected redirects; no OAuth/Apps/server publication/legacy transport or 2026 discovery claims. These are documented scope decisions, not findings.

## Fresh verification

- `cargo test --workspace --exclude moleapi-desktop --no-default-features`: exit 0, 176 passed / 0 failed / 20 ignored, 41 suites. Independent log `/tmp/moleapi-mcp-review-workspace.log`.
- `cargo test -p moleapi-server --test mcp --no-default-features -- --include-ignored`: 11 passed / 0 failed.
- Strict workspace all-target Clippy: exit 0.
- `cargo fmt --all --check` and `git diff --check`: exit 0.

Passing fixtures do not cover the three findings above. Review-directed fixes and focused regression evidence are required before approval.

## Scoped round 1 re-review

**Approved for the three review-directed backend fixes.** No remaining finding within this fix scope.

- MCP now compares protocol kind and transport identity, permitting connection-field variable resolution. The official STDIO regression uses templates for executable, argv, env, name and URI, verifies actual declared child environment and checks preserved originals. Hosted executable authorization still applies to the resolved command.
- Command admission checks `< 512` before enqueue. Count and byte accounting occur only after enqueue succeeds. The focused unit exercises queue saturation without consuming admission, exactly 512 accepted commands and rejection of 513.
- Notification methods and JSON keys use the captured-private-value mask. Matching number/boolean/null representations are withheld; colliding masked keys withhold the entire object instead of overwriting fields or leaking originals. Capability extraction uses contextual errors when withholding removes expected arrays, avoiding a panic.

Fresh independent evidence: `cargo test -p moleapi-protocols --no-default-features mcp::review_tests -- --nocapture` passed both quota/collision units. Direct execution of `target/debug/deps/mcp-cf9983c5d3d5c150 --include-ignored` passed all 12 official SDK integration tests, including templated STDIO and private labels/keys/scalars. That binary's modification time is newer than the reviewed backend source/test files.

Fresh Cargo integration compilation was blocked by a concurrent root-owned formats edit at `crates/formats/src/redact.rs:465` (`matcher.is_match(value.to_string())` requires a borrowed input). No formats source was edited by this reviewer. Consequently this re-review approves the backend fixes and fresh runtime results; final fresh workspace compilation/strict gates remain root-owned after the formats edit is complete. Implementer-reported 180 workspace passes and red-before-green runs were read, not independently rerun here.
