# Independent desktop tray source review

2026-10-04; application worktree; frozen desktop tray slice. Authority: docs/specs/desktop-tray.md and docs/superpowers/plans/2026-10-04-desktop-tray.md. Source-only API/lifetime/threading/failure review against exact locked Tauri2.12.1, tray-icon0.25.1, libappindicator/libappindicator-sys0.9.0 and single-instance2.5.2 sources. No source edits, package installs, graphical/process mutations, commits or pushes by reviewer.

## Material fallback finding and correction

**Medium — Linux AppIndicator absence panics rather than returning tray install Err.** Original desktop/src/tray.rs TrayIconBuilder::build enters tray-icon GTK TrayIcon::new, which calls AppIndicator::new before any Result-returning operations. libappindicator-sys uses a Lazy Library loader: libayatana-appindicator3.so.1, libappindicator3.so.1, and default-enabled backcompat unsuffixed .so alternatives, then panic. desktop main's if-let Err fallback does not contain that panic. A Linux system with GTK/WebKit available but no AppIndicator shared library can lose the workbench before startup instead of graceful visible-window fallback.

Root added Linux-only mature libloading preflight (`desktop/src/tray.rs:24` calls the helper before menu/tray construction; helper starts line79) matching all four exact upstream loader names. Successful native Library handle remains in scope through construction; missing libraries return a static controlled error so existing setup fallback continues with the visible workbench. No custom FFI symbols or arbitrary input library names. Reviewed Linux-only manifest dependency and deb libayatana-appindicator3-1/rpm libappindicator-gtk3 runtime declarations. This finding resolved at source level; native runtime outcome remains untested here.

## Other reviewed behavior

- Script-worker dispatch occurs before building/registering any plugin/native runtime, avoiding singleton termination of execution workers.
- Main label matches Tauri config default main. Restore attempts show, unminimize and focus independently; errors cannot suppress later steps. Explicit hide does not close; Quit uses Tauri app.exit so plugin Exit cleanup runs. Existing close policy is unchanged.
- Tauri exact window show/hide/unminimize/focus implementations queue runtime window messages, so single-instance background callbacks do not directly touch GTK/Cocoa APIs.
- Tray construction/muda menu APIs marshal creation to main thread through Tauri. TrayIcon<R> has documented Send/Sync enforcement; SDK registers a resource-table clone and explicit app.manage retains another handle. Menu callbacks, tray left-up/left-double events, template icon/tooltip and menu-on-left-click signatures match exact2.12.1 API. Linux pointer-event absence is documented and context menu remains usable.
- macOS RunEvent::Reopen exists and calls the same restoration helper; packaged transparent window image is reused with template rendering. No browser-simulated tray or external request execution.
- Callback log context is static and no request/configuration values are captured. No startup hiding, autostart or background-close policy introduced.

## Adjacent prerequisite boundary

single-instance2.5.2 Linux setup calls zbus blocking Builder::session().unwrap() (platform_impl/linux.rs:56–57), and name/serve_at setup also unwrap. Malformed/unsupported session-bus address construction can panic before desktop setup/tray fallback. Missing DBUS_SESSION_BUS_ADDRESS alone is not a failure: exact zbus5.19 defaults to XDG_RUNTIME_DIR/bus or /run/user/<uid>/bus. Root notified. This is a separate single-instance/native-session prerequisite; AppIndicator preflight does not fix it. Needs root source guard or explicit supported-native-environment documentation before claiming every startup prerequisite degrades gracefully. No native GUI reproduction performed.

## Independent verification

- `cargo metadata --no-deps --locked --format-version 1` exited0; worktree desktop dependency resolution valid.
- `cargo tree --locked -p moleapi-desktop -i tauri-plugin-single-instance` exited0, actual2.5.2 dependency reaches desktop.
- `cargo tree --locked -p moleapi-desktop -i libappindicator-sys -e features` exited0, confirms tray feature and actual default backcompat loader path.
- Direct rustfmt check of desktop/src/main.rs+tray.rs and git diff --check exited0 on reviewed source before latest preflight addition; fresh post-fix source check follows below.
- `cargo check -p moleapi-desktop --locked` exited101 in webkit2gtk-sys/javascriptcore-rs-sys build scripts: missing webkit2gtk-4.1.pc and javascriptcoregtk-4.1.pc. This occurs before desktop source typechecking; classify environment prerequisite, not a tray compile failure. No native compilation or GUI success claim.

Root owns final full-source/platform build/runtime gates. Source/API review is otherwise clean; final adjacent D-Bus correction and verdict follow below.

## Final native source verdict

Status: native tray source/API/lifetime/lifecycle review approved. AppIndicator fallback finding and adjacent address-construction panic guard are resolved at source level.

`desktop/src/tray.rs` can_configure_single_instance uses the exact mature zbus blocking Builder::session Result before optional plugin registration. Worker dispatch still comes first. Other platforms return true; Linux only obtains/parses the bus address and adds no connection attempt/startup wait. Exact zbus5.19 missing-address environment falls back to default runtime socket; valid address with absent running bus enters upstream .build Err branch and omits singleton locking rather than promising second-launch restoration. Native supported-session and tray-shell prerequisites are documented; root notified to word the guard precisely as address-construction failure.

Final source directly inspected, API signatures checked against cached locked sources, Linux-only libloading/zbus dependencies reach desktop, runtime package declarations match tray prerequisites. Fresh direct rustfmt(main+tray), git diff --check and locked cargo metadata exited0 after both guards. Independent native cargo check remains environmental exit101 before desktop code, missing WebKitGTK4.1 and JavaScriptCoreGTK4.1. This approval is a source review, not compilation/platform/GUI evidence.

No remaining material source finding in requested scope. Native Windows/macOS/Linux builds and actual tray/menu/restore/quit GUI testing remain root/platform gates. No source edits by reviewer; only this evidence report.
