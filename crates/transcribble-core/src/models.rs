use anyhow::Result;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

use crate::config::Config;

/// Information about an available Whisper model
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: &'static str,
    pub filename: &'static str,
    pub size_mb: u32,
    pub description: &'static str,
    pub english_only: bool,
}

/// All available Whisper models
pub const AVAILABLE_MODELS: &[ModelInfo] = &[
    ModelInfo {
        name: "tiny.en",
        filename: "ggml-tiny.en.bin",
        size_mb: 75,
        description: "Fastest, good for quick notes (English only)",
        english_only: true,
    },
    ModelInfo {
        name: "tiny",
        filename: "ggml-tiny.bin",
        size_mb: 75,
        description: "Fastest, multilingual support",
        english_only: false,
    },
    ModelInfo {
        name: "base.en",
        filename: "ggml-base.en.bin",
        size_mb: 142,
        description: "Good balance of speed and accuracy (English only)",
        english_only: true,
    },
    ModelInfo {
        name: "base",
        filename: "ggml-base.bin",
        size_mb: 142,
        description: "Good balance, multilingual",
        english_only: false,
    },
    ModelInfo {
        name: "small.en",
        filename: "ggml-small.en.bin",
        size_mb: 466,
        description: "More accurate, slower (English only)",
        english_only: true,
    },
    ModelInfo {
        name: "small",
        filename: "ggml-small.bin",
        size_mb: 466,
        description: "More accurate, multilingual",
        english_only: false,
    },
    ModelInfo {
        name: "medium.en",
        filename: "ggml-medium.en.bin",
        size_mb: 1500,
        description: "High accuracy, requires more RAM (English only)",
        english_only: true,
    },
    ModelInfo {
        name: "medium",
        filename: "ggml-medium.bin",
        size_mb: 1500,
        description: "High accuracy, multilingual",
        english_only: false,
    },
];

/// Get model info by name
pub fn get_model_info(name: &str) -> Option<&'static ModelInfo> {
    AVAILABLE_MODELS.iter().find(|m| m.name == name)
}

/// Get the path where a model would be stored
pub fn get_model_path(model_name: &str) -> PathBuf {
    let filename = format!("ggml-{}.bin", model_name);
    Config::app_dir().join(filename)
}

/// Check if a model is downloaded
pub fn is_model_downloaded(model_name: &str) -> bool {
    get_model_path(model_name).exists()
}

/// List all downloaded models
pub fn list_downloaded_models() -> Vec<&'static ModelInfo> {
    AVAILABLE_MODELS
        .iter()
        .filter(|m| is_model_downloaded(m.name))
        .collect()
}

/// Progress callback for model downloads
/// Called with (bytes_downloaded, total_bytes)
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send>;

/// Download a model from Hugging Face with optional progress callback
pub async fn download_model_with_progress<F>(model_name: &str, on_progress: Option<F>) -> Result<PathBuf>
where
    F: Fn(u64, u64) + Send + 'static,
{
    let model_info = get_model_info(model_name).ok_or_else(|| {
        let available: Vec<_> = AVAILABLE_MODELS.iter().map(|m| m.name).collect();
        anyhow::anyhow!(
            "Unknown model: {}. Available models: {}",
            model_name,
            available.join(", ")
        )
    })?;

    let base_url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
    let url = format!("{}/{}", base_url, model_info.filename);

    // Ensure download directory exists
    let download_dir = Config::app_dir();
    fs::create_dir_all(&download_dir)?;

    let output_path = download_dir.join(model_info.filename);

    // Check if already exists
    if output_path.exists() {
        return Ok(output_path);
    }

    // Download with progress
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download: HTTP {}",
            response.status()
        ));
    }

    let total_size = response.content_length().unwrap_or(0);

    let mut file = File::create(&output_path)?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;

        if let Some(ref callback) = on_progress {
            callback(downloaded, total_size);
        }
    }

    Ok(output_path)
}

/// Download a model from Hugging Face (without progress callback)
pub async fn download_model(model_name: &str) -> Result<PathBuf> {
    download_model_with_progress::<fn(u64, u64)>(model_name, None).await
}

/// Display format for model selection
impl ModelInfo {
    pub fn display_for_selection(&self, downloaded: bool) -> String {
        let status = if downloaded { " [downloaded]" } else { "" };
        format!(
            "{} ({} MB){} - {}",
            self.name, self.size_mb, status, self.description
        )
    }
}
