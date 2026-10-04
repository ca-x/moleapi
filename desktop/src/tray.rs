//! Native tray/menu callbacks. Request execution and scripts never enter this module.
use tauri::{
    App, AppHandle, Manager, Runtime,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        // Each step is independent: a platform's unminimize failure must not
        // prevent showing a hidden window or attempting to focus it.
        for (operation, outcome) in [
            ("show", window.show()),
            ("unminimize", window.unminimize()),
            ("focus", window.set_focus()),
        ] {
            if let Err(error) = outcome {
                eprintln!("MoleAPI window {operation} failed: {error}");
            }
        }
    }
}

pub fn install<R: Runtime>(app: &App<R>) -> anyhow::Result<()> {
    // The upstream Linux AppIndicator loader panics when none of its native
    // libraries exists. Check with the mature loader before entering that API.
    #[cfg(target_os = "linux")]
    let _indicator = appindicator_library()?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Packaged MoleAPI tray image is unavailable"))?;
    let show = MenuItem::with_id(app, "moleapi.show", "显示主窗口", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "moleapi.hide", "隐藏主窗口", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "moleapi.quit", "退出 MoleAPI", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &separator, &quit])?;
    let tray = TrayIconBuilder::with_id("moleapi.tray")
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("MoleAPI")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "moleapi.show" => show_main(app),
            "moleapi.hide" => {
                if let Some(window) = app.get_webview_window("main")
                    && let Err(error) = window.hide()
                {
                    eprintln!("MoleAPI window hide failed: {error}");
                }
            }
            "moleapi.quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            ) {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    // Tauri retains a resource-table reference; managed state explicitly ties
    // our own handle to application lifetime as well.
    app.manage(tray);
    Ok(())
}

#[cfg(target_os = "linux")]
fn appindicator_library() -> anyhow::Result<libloading::Library> {
    for name in [
        "libayatana-appindicator3.so.1",
        "libappindicator3.so.1",
        "libayatana-appindicator3.so",
        "libappindicator3.so",
    ] {
        // SAFETY: names are fixed upstream platform library names, not imported
        // request/configuration input. No symbols or arbitrary FFI are called.
        if let Ok(library) = unsafe { libloading::Library::new(name) } {
            return Ok(library);
        }
    }
    anyhow::bail!("AppIndicator runtime library is unavailable; the main window remains usable")
}

pub fn can_configure_single_instance() -> bool {
    #[cfg(target_os = "linux")]
    {
        // The mature plugin unwraps this address-construction step. Missing or
        // malformed session-bus configuration must not stop the workbench.
        if zbus::blocking::connection::Builder::session().is_err() {
            eprintln!("MoleAPI single-instance unavailable: invalid session-bus configuration");
            return false;
        }
    }
    true
}
