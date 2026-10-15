use moleapi_generation::project::ProjectRuntime;
use std::path::PathBuf;
use tauri_plugin_dialog::DialogExt;

pub struct NativeGeneration {
    pub runtime: ProjectRuntime,
    pub preference: PathBuf,
    pub selection: tauri::async_runtime::Mutex<()>,
}

#[tauri::command]
pub async fn select_java_runtime(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeGeneration>,
) -> Result<Option<u32>, String> {
    let _selection = state.selection.lock().await;
    let selected =
        tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_file())
            .await
            .map_err(|_| "Java selection failed")?;
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|_| "Select a local Java executable")?;
    state
        .runtime
        .select_native_java(path, &state.preference)
        .await
        .map(Some)
        .map_err(|_| {
            "Select a working Java17+ executable and check application settings access".into()
        })
}
