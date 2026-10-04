# Desktop tray implementation plan

Spec: docs/specs/desktop-tray.md. Inline implementation with current independent scoped reviewer.

1. Add Tauri tray-icon feature and mature single-instance plugin; regenerate lock with no broad dependency upgrade. Create desktop/src/tray.rs for native menu/icon/lifetime and restoration, using package-context transparent image. Read exact locked Tauri/plugin source signatures before integration.
2. Integrate single-instance callback after worker dispatch, install tray during app setup with graceful fallback, and macOS Dock reopen event. No close-policy changes or browser IPC exposure. Retain handle in managed state.
3. Verify explicit-package rustfmt/diff/metadata, attempt native check only with current local capabilities; record actual system-lib limitation without claiming GUI success. Independent source/API/lifecycle review. Existing GitHub Actions verify supported native builds on push; no local Docker.
4. Update status and source/runtime dependency docs, commit explicit owned files and proxy push. Final main/platform delivery waits for parent feature completion.

Source integration and independent review complete. Linux native prerequisite/address preflights and runtime package declarations added. Locked metadata/rustfmt/diff pass; local native compilation blocked before application source by missing GTK4.1 WebKit/JavaScriptCore packages. Native graphical/platform verification remains pending existing Actions/final integration, not marked complete.
