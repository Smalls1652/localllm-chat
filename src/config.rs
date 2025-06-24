use serde::{Deserialize, Serialize};

/// Config for the LocalLLM Chat app.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LlmChatConfig {
    /// The image tag to use for Open WebUI.
    #[serde(rename = "openwebui_image_tag", default = "openwebui_image_tag_default")]
    pub openwebui_image_tag: String,

    /// The image tag to use for Apache Tika.
    #[serde(rename = "tika_image_tag", default = "tika_image_tag_default")]
    pub tika_image_tag: String,

    /// Any extra backend services to run.
    #[serde(rename = "extra_backend_services", skip_serializing_if = "Option::is_none")]
    pub extra_backend_services: Option<Vec<LlmChatConfigExtraBackendService>>
}

impl Default for LlmChatConfig {
    fn default() -> Self {
        Self {
            openwebui_image_tag: "latest".to_string(),
            tika_image_tag: "latest-full".to_string(),
            extra_backend_services: None
        }
    }
}

fn openwebui_image_tag_default() -> String {
    "latest".to_string()
}

fn tika_image_tag_default() -> String {
    "latest-full".to_string()
}

/// Config for an extra background service to run.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LlmChatConfigExtraBackendService {
    /// The name to use for the service.
    #[serde(rename = "name")]
    pub name: String,

    /// The image to use.
    #[serde(rename = "image")]
    pub image: String,

    /// Environment variables for the service.
    #[serde(rename = "env", skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<String>>,

    /// Ports to expose within the backend network.
    #[serde(rename = "ports", skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>
}
