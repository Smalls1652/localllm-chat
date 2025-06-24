// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use locallm_chat_lib::error::AppError;

/// The application's entrypoint.
#[tokio::main]
async fn main() -> Result<(), AppError> {
    tauri::async_runtime::set(tokio::runtime::Handle::current());

    let _ = locallm_chat_lib::run().await?;

    Ok(())
}
