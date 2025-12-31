pub mod audio;
pub mod config;
pub mod db;
pub mod history;
pub mod hotkeys;
pub mod models;
pub mod transcription;

pub use audio::{AudioCapture, DeviceInfo};
pub use config::{Config, HistoryConfig, InputConfig, ModelConfig, OutputConfig};
pub use db::{Database, TranscriptionRecord, Statistics, ModelRecord};
pub use history::TranscriptionEntry;
pub use hotkeys::{parse_hotkey, HOTKEY_OPTIONS};
pub use models::{get_model_info, get_model_path, is_model_downloaded, list_downloaded_models, ModelInfo, AVAILABLE_MODELS};
pub use transcription::{load_model, transcribe};
