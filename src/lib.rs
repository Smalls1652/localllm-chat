use tauri::RunEvent;

use crate::error::AppError;

/// Config options for the app.
pub mod config;

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
            let webview_window_builder = tauri::WebviewWindowBuilder::new(
                app,
                "local_llm_chat_main",
                tauri::WebviewUrl::App("http://localhost:11690".into()),
            )
            .title("LocalLLM Chat")
            .inner_size(1280.0, 800.0);

            #[cfg(target_os = "macos")]
            let webview_window_builder =
                webview_window_builder.title_bar_style(tauri::TitleBarStyle::Transparent);

            let webview_window = webview_window_builder.build()?;

            // Gotta allow deprecated items in this section.
            // The `cocoa` crate is basically disabled, but, as far as I can tell, the replacement
            // doesn't allow for converting raw pointers to their specific counterparts.
            // Maybe Tauri is aware of that?
            #[allow(deprecated)]
            #[cfg(target_os = "macos")]
            {
                use cocoa::appkit::NSWindow;
                use cocoa::base::id;

                let ns_window = webview_window.ns_window().unwrap() as id;
                unsafe {
                    use cocoa::appkit::NSColor;
                    use cocoa::base::nil;

                    let bg_color = NSColor::colorWithRed_green_blue_alpha_(
                        nil,
                        13.0 / 255.0,
                        13.0 / 255.0,
                        13.0 / 255.0,
                        1.0,
                    );
                    ns_window.setBackgroundColor_(bg_color);
                }
            }

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

    let app_config = match utils::get_app_config(&app.handle()) {
        Ok(config) => config,

        Err(err) => {
            utils::show_setup_local_appdata_error(&app.handle(), &err);

            return Err(err);
        }
    };

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
    if let Err(container_err) = container::pull_required_images(&app_config).await {
        utils::show_docker_error(&app.handle(), &container_err);

        return Err(container_err);
    }

    // Clean up any dangling container resources before running.
    // This *shouldn't* be needed, but, in the event that something catastrophically
    // occurred in a previous session, this can clean up those leftover resources.
    println!("Cleaning up previous containers, if needed");
    if let Err(container_err) = container::cleanup_infrastructure(&app_config).await {
        utils::show_docker_error(&app.handle(), &container_err);

        return Err(container_err);
    }

    // Start the containers.
    println!("Starting container");
    if let Err(container_err) = container::create_infrastructure(&app_config, &data_dir).await {
        utils::show_docker_error(&app.handle(), &container_err);

        container::cleanup_infrastructure(&app_config).await?;

        return Err(container_err);
    }

    // Wait until the Open WebUI container is healthy.
    utils::wait_until_openwebui_is_healthy(&app.handle()).await?;

    // Run the app.
    #[allow(unused_variables)]
    app.run(move |app_handle, event| match event {
        RunEvent::Exit => {
            // On exit, remove the containers and networks created.
            println!("Cleaning up containers, if needed");
            let cleanup_result = tokio::task::block_in_place(|| {
                tauri::async_runtime::block_on(async {
                    container::cleanup_infrastructure(&app_config).await
                })
            });

            if let Err(container_err) = cleanup_result {
                utils::show_docker_error(&app_handle, &container_err);
            }
        }
        _ => {}
    });

    Ok(())
}
