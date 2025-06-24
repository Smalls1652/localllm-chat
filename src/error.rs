use std::env::VarError;

use thiserror::Error;

/// Errors for the LocalLLM Chat app.
#[derive(Error, Debug)]
pub enum AppError {
    /// A generic error.
    #[error("An error occurred: {0}")]
    GenericError(String),

    /// Failed to get an environment variable.
    #[error("Failed to get environment variable: {0}\n\n{1}")]
    EnvironmentVariableError(String, VarError),

    /// An I/O operation failed.
    #[error("An I/O operation failed: {0}")]
    IOError(std::io::Error),

    /// An error occurred with Tauri.
    #[error("An error occurred with tauri: {0}")]
    TauriError(tauri::Error),

    /// An error occurred while interacting with the Docker API.
    #[error("Docker error: {0}")]
    DockerError(bollard::errors::Error),

    /// An error occurred while serializing/deserializing YAML.
    #[error("YAML error: {0}")]
    YamlError(serde_yaml::Error)
}
