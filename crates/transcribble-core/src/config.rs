use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub model: ModelConfig,
    pub input: InputConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub history: HistoryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub path: PathBuf,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConfig {
    pub hotkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    #[serde(default = "default_true")]
    pub show_word_count: bool,
    #[serde(default = "default_true")]
    pub show_duration: bool,
    #[serde(default = "default_true")]
    pub auto_type: bool,
    #[serde(default)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
}

fn default_true() -> bool {
    true
}

fn default_max_entries() -> usize {
    1000
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            show_word_count: true,
            show_duration: true,
            auto_type: true,
            verbose: false,
        }
    }
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 1000,
        }
    }
}

impl Config {
    /// Get the path to the transcribble directory (~/.transcribble)
    pub fn app_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".transcribble")
    }

    /// Get the path to the config file
    pub fn config_path() -> PathBuf {
        Self::app_dir().join("config.toml")
    }

    /// Get the path to the history directory
    pub fn history_dir() -> PathBuf {
        Self::app_dir().join("history")
    }

    /// Check if a config file exists
    pub fn exists() -> bool {
        Self::config_path().exists()
    }

    /// Load config from file
    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        let content = fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Failed to read config file: {}", e))?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config file: {}", e))?;
        Ok(config)
    }

    /// Save config to file
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
        fs::write(&path, content)?;
        Ok(())
    }

    /// Create a new config with the given model and hotkey
    pub fn new(model_path: PathBuf, model_name: String, hotkey: String) -> Self {
        Self {
            model: ModelConfig {
                path: model_path,
                name: model_name,
            },
            input: InputConfig { hotkey },
            output: OutputConfig::default(),
            history: HistoryConfig::default(),
        }
    }
}
