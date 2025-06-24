use serde::Deserialize;
use std::{fs, path::PathBuf, time::Duration};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_http::reqwest;

use crate::error::AppError;

/// Sets up the local appdata directory for the application.
///
/// # Arguments
///
/// * `app` - The app handle.
pub fn setup_local_appdata(app: &AppHandle<Wry>) -> Result<(), AppError> {
    let container_dir = get_app_container_dir(&app)?;
    ensure_container_data_dir_exists(&container_dir)?;

    Ok(())
}

/// Shows an error dialog for local appdata setup failure.
///
/// # Arguments
///
/// * `app` - The app handle.
/// * `error` - The error to show.
pub fn show_setup_local_appdata_error(app: &AppHandle<Wry>, error: &AppError) {
    app.dialog()
        .message(format!(
            "Failed to setup local app data: {}",
            error.to_string()
        ))
        .kind(MessageDialogKind::Error)
        .title("Error")
        .blocking_show();
}

/// Shows an error dialog for a Docker failure.
///
/// # Arguments
///
/// * `app` - The app handle.
/// * `error` - The error to show.
pub fn show_docker_error(app: &AppHandle<Wry>, error: &AppError) {
    app.dialog()
        .message(format!("Docker failure: {}", error.to_string()))
        .kind(MessageDialogKind::Error)
        .title("Error")
        .blocking_show();
}

/// Gets the container directory in the local appdata directory.
///
/// If the container directory does not exist, it will create it.
///
/// # Arguments
///
/// * `app` - The app handle.
pub fn get_app_container_dir(app: &AppHandle<Wry>) -> Result<PathBuf, AppError> {
    let app_data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| AppError::TauriError(e))?;

    let app_container_dir = app_data_dir.join("openwebui");

    if !app_container_dir.exists() {
        fs::create_dir_all(&app_container_dir).map_err(|e| AppError::IOError(e))?;
    }

    Ok(app_container_dir)
}

/// Ensures that the `data` directory exists in the container directory.
///
/// If the `data` directory does not exist, it will create it.
///
/// # Arguments
///
/// * `container_dir` - The path to the container directory.
fn ensure_container_data_dir_exists(container_dir: &PathBuf) -> Result<(), AppError> {
    let data_dir = container_dir.join("data");

    if !data_dir.exists() {
        fs::create_dir_all(&data_dir).map_err(|e| AppError::IOError(e))?;
    }

    Ok(())
}

/// Represents the status of Open WebUI retrieved from the `/health` endpoint.
#[derive(Deserialize, Debug, Clone)]
pub struct OpenWebUiHealthStatus {
    /// Whether the server is healthy or not.
    #[serde(rename = "status")]
    pub status: bool,
}

/// Wait until the Open WebUI server is healthy.
///
/// # Arguments
///
/// * `app` - The app handle.
pub async fn wait_until_openwebui_is_healthy(app: &AppHandle<Wry>) -> Result<(), AppError> {
    // I can almost guarantee that this can be done muuuuuuch better.
    // But hey! That's thrown together code for ya. :P
    let mut counter = 0;
    while counter < 120 {
        let api_response_result = reqwest::get("http://localhost:11690/health").await;

        if let Ok(response) = api_response_result {
            let status_result = response.json::<OpenWebUiHealthStatus>().await;

            if let Ok(status_data) = status_result {
                if status_data.status {
                    return Ok(());
                }
            }
        }

        counter += 1;

        std::thread::sleep(Duration::from_secs(1));
    }

    app.dialog()
        .message("Startup took too long")
        .kind(MessageDialogKind::Error)
        .title("Error")
        .blocking_show();

    Err(AppError::GenericError("Startup took too long".to_string()))
}
