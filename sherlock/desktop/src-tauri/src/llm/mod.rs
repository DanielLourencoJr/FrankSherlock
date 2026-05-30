pub mod client;
pub mod groq;
pub mod management;
pub mod model_selection;

pub const OLLAMA_BASE: &str = "http://localhost:11434";

pub use client::{generate, ollama_generate, parse_json_response};
pub use groq::{groq_generate, is_api_key_configured, GROQ_DEFAULT_MODEL};
pub use management::{
    cleanup_loaded_models, list_installed_models, list_loaded_models, model_satisfies,
    resolve_installed_model, DownloadState,
};
pub use model_selection::recommended_model;

/// Supported LLM providers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[serde(rename = "ollama")]
    Ollama { model: String },
    #[serde(rename = "groq")]
    Groq { model: String, api_key: String },
}

impl Provider {
    /// Human-readable provider name.
    pub fn name(&self) -> &'static str {
        match self {
            Provider::Ollama { .. } => "ollama",
            Provider::Groq { .. } => "groq",
        }
    }

    /// The model string (Ollama tag or Groq model name).
    pub fn model(&self) -> &str {
        match self {
            Provider::Ollama { model } => model,
            Provider::Groq { model, .. } => model,
        }
    }
}
