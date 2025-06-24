use tauri::RunEvent;

use crate::error::AppError;

/// Functions for interacting with container APIs (Docker or Docker-compatible).
pub mod container;

/// Error types and utilities.
pub mod error;

/// Generic shared utilities for the app.
pub mod utils;

/// Runs the `tauri` app.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub async fn run() -> Result<(), AppError> {
    // Build the application.
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(move |app| {
            let _webview_window = tauri::WebviewWindowBuilder::new(
                app,
                "local_llm_chat_main",
                tauri::WebviewUrl::App("http://localhost:11690".into()),
            )
            .title("LocalLLM Chat")
            .inner_size(1280.0, 800.0)
            .build()?;

            Ok(())
        })
        .build(tauri::generate_context!())
        .map_err(|e| AppError::TauriError(e))?;

    // Set up the local appdata directory.
    println!("Setting up...");
    let setup_result = utils::setup_local_appdata(&app.handle());

    if let Err(setup_error) = setup_result {
        utils::show_setup_local_appdata_error(&app.handle(), &setup_error);

        return Err(setup_error);
    }

    // Get the data directory path for the Open WebUI container.
    let data_dir = match utils::get_app_container_dir(&app.handle()) {
        Ok(container_dir) => container_dir.join("data"),

        Err(err) => {
            utils::show_setup_local_appdata_error(&app.handle(), &err);

            return Err(err);
        }
    };

    // Pull the container images needed for the application.
    println!("Pulling container images");
    if let Err(container_err) = container::pull_required_images().await {
        utils::show_docker_error(&app.handle(), &container_err);

        return Err(container_err);
    }

    // Clean up any dangling container resources before running.
    // This *shouldn't* be needed, but, in the event that something catastrophically
    // occurred in a previous session, this can clean up those leftover resources.
    println!("Cleaning up previous containers, if needed");
    if let Err(container_err) = container::cleanup_infrastructure().await {
        utils::show_docker_error(&app.handle(), &container_err);

        return Err(container_err);
    }

    // Start the containers.
    println!("Starting container");
    if let Err(container_err) = container::create_infrastructure(&data_dir).await {
        utils::show_docker_error(&app.handle(), &container_err);

        container::cleanup_infrastructure().await?;

        return Err(container_err);
    }

    // Wait until the Open WebUI container is healthy.
    utils::wait_until_openwebui_is_healthy(&app.handle()).await?;

    // Run the app.
    #[allow(unused_variables)]
    app.run(|app_handle, event| match event {
        RunEvent::Exit => {
            // On exit, remove the containers and networks created.
            println!("Cleaning up containers, if needed");
            let cleanup_result = tokio::task::block_in_place(|| {
                tauri::async_runtime::block_on(async { container::cleanup_infrastructure().await })
            });

            if let Err(container_err) = cleanup_result {
                utils::show_docker_error(&app_handle, &container_err);
            }
        }
        _ => {}
    });

    Ok(())
}
