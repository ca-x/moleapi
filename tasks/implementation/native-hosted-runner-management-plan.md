# Native Hosted Runner Management Implementation Plan

**Goal:** Manage the connected self-hosted runner account from Tauri without losing offline behavior or exposing its stored login token.

**Spec:** `docs/specs/native-hosted-runner-management.md`.

1. Add an IPC-only, exact-allowlist local gateway in the shared Rust router. Reuse the sync connection/client and bounded JSON response handling; add a fresh public connection session ID for UI fences.
2. Add a native settings panel that loads remote workspace/runner/task/schedule metadata through the gateway, validates the selected remote source, and supports registry, credentials, dispatch, cancellation, schedule actions and report inspection. Reuse the existing credential dialog by injecting its request function.
3. Write hosted/local two-router and frontend fixtures for scope, disconnect, stale responses, remote revision, one-time secret and direct/label target behavior. Document the setup and limitations.
4. Run tests, lint, browser/native QA and builds only after functionality and test code for the remaining planned slices are complete, per the user's current instruction.
