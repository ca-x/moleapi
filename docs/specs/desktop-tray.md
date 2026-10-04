# Desktop system tray

User requested2026-10-04. Tauri desktop only; independent server does not create OS tray UI.

Use mature Tauri2 tray/menu APIs and tauri-plugin-single-instance. Tray image is the existing transparent supplied MoleAPI icon from packaged context, with macOS template rendering. Provide Show main window, Hide main window and Quit; left release/double click restores/unminimizes/focuses the main window on platforms that deliver those events. Linux uses the context menu because Tauri does not deliver tray pointer events there. Second launch and macOS Dock reopen restore the existing window; script workers must dispatch before the single-instance plugin.

No automatic background/startup policy change: normal window close follows existing lifecycle, hiding is an explicit menu action. Retain native tray handle for app lifetime. Tray creation failure must not prevent launching the API workbench or hide the only visible window. Callback errors are logged with static operation context, without user request/configuration data.

Use existing supported OS package icons and mature native behavior, not browser-simulated tray. No local Docker. Source gates and independent review before commit/push; actual native graphical/platform testing remains needed, with Linux local missing WebKitGTK4.1/JavaScriptCoreGTK4.1 noted. Existing Actions already install AppIndicator build dependencies; preserve features-first sequencing.

Linux prerequisites/fallback: AppIndicator native loading is checked through mature libloading before entering the upstream panic-prone loader; all four supported SONAMEs match the pinned driver. deb/rpm declare their actual runtime packages. Single-instance session-address configuration is preflighted with mature zbus before entering the plugin's unwrap; address-construction failure skips that optional plugin. A valid bus address with no running session bus may make the upstream plugin omit its single-instance lock; it does not establish second-launch restoration. Actual tray visibility needs a desktop shell supporting AppIndicator/StatusNotifier.
