#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
};
use serde::Serialize;
use serde_json::Value;
use tauri::Manager;
use tower::ServiceExt;

struct AppState(Router);

#[derive(Serialize)]
struct ApiReply {
    status: u16,
    body: Value,
}

#[tauri::command]
async fn api(
    method: String,
    path: String,
    body: Option<Value>,
    state: tauri::State<'_, AppState>,
) -> Result<ApiReply, String> {
    if !path.starts_with("/api/")
        || path.split('/').any(|part| matches!(part, "." | ".."))
        || path.contains('#')
    {
        return Err("Invalid API path".into());
    }
    let request = Request::builder()
        .method(method.as_str())
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))
        .map_err(|e| e.to_string())?;
    let response = state
        .0
        .clone()
        .oneshot(request)
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), 30 * 1024 * 1024)
        .await
        .map_err(|e| e.to_string())?;
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?
    };
    Ok(ApiReply { status, body })
}

fn main() {
    match moleapi_server::dispatch_script_worker() {
        Ok(true) => return,
        Ok(false) => {}
        Err(_) => std::process::exit(1),
    }
    let builder = tauri::Builder::default();
    let builder = if tray::can_configure_single_instance() {
        builder.plugin(tauri_plugin_single_instance::init(
            |app, _args, _directory| {
                tray::show_main(app);
            },
        ))
    } else {
        builder
    };
    let application = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            let router = tauri::async_runtime::block_on(moleapi_server::local(
                &directory.join("moleapi.db"),
            ))?;
            app.manage(AppState(router));
            if let Err(error) = tray::install(app) {
                // Desktop environments without tray support still get a visible
                // and usable workbench; no window is hidden during startup.
                eprintln!("MoleAPI tray unavailable: {error}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![api])
        .build(tauri::generate_context!())
        .expect("MoleAPI desktop runtime failed");
    application.run(|_app, _event| {
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = _event {
            tray::show_main(_app);
        }
    });
}
