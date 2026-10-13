# Runner Service Installation Implementation Plan

**Goal:** Install and manage a supervised Linux user runner without exposing the scoped bearer.

**Spec:** `docs/specs/runner-service-installation.md`.

1. Add `runner service print/install/status/uninstall` CLI arguments and a dedicated service module. Use the existing server URL and private token-file inputs, validating canonical runner IDs and file permissions.
2. Render a deterministic systemd unit with safe argument escaping and a managed marker. Install atomically in the user unit directory and invoke only `systemctl --user` for reload/enable/start/status/disable. Refuse foreign unit replacement/removal.
3. Write service generator and fake-systemctl CLI process fixtures, update usage/coverage/status. Compile code and test targets without executing tests, per the user's test-last direction.
4. Implement launchd via a per-user LaunchAgent with the same private credential and managed-file rules; compile for a macOS target and write a plist/ownership fixture.
5. Implement Windows Task Scheduler as a current-user logon task with XML argument quoting, LocalAppData credential boundary, managed manifest and cautious replacement/removal. Write platform-independent XML fixtures and defer actual manager runtime verification to Windows CI/native QA.
