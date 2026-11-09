// This build is updated by rebuilding its own source, not by the upstream app updater.

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    available: bool,
    version: Option<String>,
    date: Option<String>,
    notes: Option<String>,
}

#[tauri::command]
pub async fn check_for_update() -> Result<UpdateInfo, String> {
    Ok(UpdateInfo {
        available: false,
        version: None,
        date: None,
        notes: None,
    })
}

#[tauri::command]
pub async fn download_and_install_update() -> Result<(), String> {
    Err("VoicePaste Personal is updated by rebuilding its source.".to_string())
}
