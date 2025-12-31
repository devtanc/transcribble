use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use transcribble_core::{
    models::{download_model_with_progress, get_model_path, is_model_downloaded, AVAILABLE_MODELS},
    Config,
};

use crate::state::AppState;

// =====================
// Types for frontend
// =====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfoResponse {
    pub name: String,
    pub filename: String,
    pub size_mb: u32,
    pub description: String,
    pub english_only: bool,
    pub downloaded: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub model_name: String,
    pub model_path: String,
    pub hotkey: String,
    pub auto_type: bool,
    pub show_word_count: bool,
    pub show_duration: bool,
    pub history_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub model_name: String,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionRecord {
    pub id: i64,
    pub timestamp: String,
    pub text: String,
    pub duration_ms: i64,
    pub word_count: i64,
    pub character_count: i64,
    pub keystrokes_saved: i64,
    pub model_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Statistics {
    pub total_transcriptions: i64,
    pub total_words: i64,
    pub total_duration_ms: i64,
    pub total_keystrokes_saved: i64,
    pub total_minutes: f64,
}

// =====================
// Configuration Commands
// =====================

#[tauri::command]
pub fn get_config() -> Result<ConfigResponse, String> {
    let config = Config::load().map_err(|e| e.to_string())?;

    Ok(ConfigResponse {
        model_name: config.model.name,
        model_path: config.model.path.to_string_lossy().to_string(),
        hotkey: config.input.hotkey,
        auto_type: config.output.auto_type,
        show_word_count: config.output.show_word_count,
        show_duration: config.output.show_duration,
        history_enabled: config.history.enabled,
    })
}

#[tauri::command]
pub fn save_config(
    hotkey: String,
    auto_type: bool,
    show_word_count: bool,
    show_duration: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut config = Config::load().map_err(|e| e.to_string())?;

    // Check if hotkey is changing
    let hotkey_changed = config.input.hotkey != hotkey;

    config.input.hotkey = hotkey.clone();
    config.output.auto_type = auto_type;
    config.output.show_word_count = show_word_count;
    config.output.show_duration = show_duration;

    config.save().map_err(|e| e.to_string())?;

    // If hotkey changed, update state and restart listener
    if hotkey_changed {
        *state.current_hotkey.write().unwrap() = hotkey;
        crate::listener::stop_listener();
        crate::listener::start_listener(app);
    }

    Ok(())
}

// =====================
// Listening State Commands
// =====================

#[tauri::command]
pub fn get_listening_state(state: State<'_, AppState>) -> bool {
    state.get_listening()
}

#[tauri::command]
pub fn get_recording_state(state: State<'_, AppState>) -> bool {
    state.get_recording()
}

// =====================
// Model Management Commands
// =====================

#[tauri::command]
pub fn get_available_models() -> Vec<ModelInfoResponse> {
    let active_model = Config::load().ok().map(|c| c.model.name);

    AVAILABLE_MODELS
        .iter()
        .map(|m| ModelInfoResponse {
            name: m.name.to_string(),
            filename: m.filename.to_string(),
            size_mb: m.size_mb,
            description: m.description.to_string(),
            english_only: m.english_only,
            downloaded: is_model_downloaded(m.name),
            active: active_model.as_ref().map(|n| n == m.name).unwrap_or(false),
        })
        .collect()
}

#[tauri::command]
pub fn get_downloaded_models() -> Vec<ModelInfoResponse> {
    get_available_models()
        .into_iter()
        .filter(|m| m.downloaded)
        .collect()
}

#[tauri::command]
pub async fn download_model(model_name: String, app: AppHandle) -> Result<(), String> {
    let _model_info = transcribble_core::get_model_info(&model_name)
        .ok_or_else(|| format!("Unknown model: {}", model_name))?;

    let app_clone = app.clone();
    let model_name_clone = model_name.clone();

    let _path = download_model_with_progress(&model_name, Some(move |downloaded: u64, total: u64| {
        let percent = if total > 0 {
            (downloaded as f32 / total as f32) * 100.0
        } else {
            0.0
        };

        let _ = app_clone.emit(
            "download-progress",
            DownloadProgress {
                model_name: model_name_clone.clone(),
                bytes_downloaded: downloaded,
                total_bytes: total,
                percent,
            },
        );
    }))
    .await
    .map_err(|e| e.to_string())?;

    // Emit completion event
    let _ = app.emit("download-complete", model_name.clone());

    Ok(())
}

#[tauri::command]
pub fn set_active_model(model_name: String, state: State<'_, AppState>) -> Result<(), String> {
    // Verify model exists
    if !is_model_downloaded(&model_name) {
        return Err(format!("Model '{}' is not downloaded", model_name));
    }

    // Update config
    let mut config = Config::load().map_err(|e| e.to_string())?;
    config.model.path = get_model_path(&model_name);
    config.model.name = model_name.clone();
    config.save().map_err(|e| e.to_string())?;

    // Update app state
    *state.current_model.write().unwrap() = model_name.clone();

    // Reload the model
    let model_path = config.model.path.to_string_lossy().to_string();
    let ctx = transcribble_core::load_model(&model_path).map_err(|e| e.to_string())?;
    *state.whisper_ctx.write().unwrap() = Some(ctx);

    Ok(())
}

#[tauri::command]
pub fn get_active_model() -> Result<String, String> {
    let config = Config::load().map_err(|e| e.to_string())?;
    Ok(config.model.name)
}

// =====================
// History & Statistics Commands
// =====================

#[tauri::command]
pub fn get_history(
    limit: Option<usize>,
    _offset: Option<usize>,
) -> Result<Vec<TranscriptionRecord>, String> {
    // For now, return from JSONL history
    let entries = transcribble_core::history::read_recent(limit.unwrap_or(50))
        .map_err(|e| e.to_string())?;

    Ok(entries
        .into_iter()
        .enumerate()
        .map(|(i, e)| {
            let char_count = e.text.len() as i64;
            TranscriptionRecord {
                id: i as i64,
                timestamp: e.timestamp.to_rfc3339(),
                text: e.text,
                duration_ms: e.duration_ms as i64,
                word_count: e.word_count as i64,
                character_count: char_count,
                keystrokes_saved: char_count,
                model_name: e.model,
            }
        })
        .collect())
}

#[tauri::command]
pub fn get_statistics() -> Result<Statistics, String> {
    let entries = transcribble_core::history::read_recent(usize::MAX)
        .map_err(|e| e.to_string())?;

    let total_transcriptions = entries.len() as i64;
    let total_words: i64 = entries.iter().map(|e| e.word_count as i64).sum();
    let total_duration_ms: i64 = entries.iter().map(|e| e.duration_ms as i64).sum();
    let total_keystrokes_saved: i64 = entries.iter().map(|e| e.text.len() as i64).sum();
    let total_minutes = total_duration_ms as f64 / 60000.0;

    Ok(Statistics {
        total_transcriptions,
        total_words,
        total_duration_ms,
        total_keystrokes_saved,
        total_minutes,
    })
}

#[tauri::command]
pub fn search_history(
    query: String,
    limit: Option<usize>,
) -> Result<Vec<TranscriptionRecord>, String> {
    let entries = transcribble_core::history::read_recent(limit.unwrap_or(50))
        .map_err(|e| e.to_string())?;

    let query_lower = query.to_lowercase();
    Ok(entries
        .into_iter()
        .filter(|e| e.text.to_lowercase().contains(&query_lower))
        .enumerate()
        .map(|(i, e)| {
            let char_count = e.text.len() as i64;
            TranscriptionRecord {
                id: i as i64,
                timestamp: e.timestamp.to_rfc3339(),
                text: e.text,
                duration_ms: e.duration_ms as i64,
                word_count: e.word_count as i64,
                character_count: char_count,
                keystrokes_saved: char_count,
                model_name: e.model,
            }
        })
        .collect())
}

#[tauri::command]
pub fn delete_transcription(_id: i64) -> Result<(), String> {
    // Not implemented for JSONL - would need SQLite
    Err("Delete not supported with JSONL history".to_string())
}

#[tauri::command]
pub fn clear_history() -> Result<(), String> {
    transcribble_core::history::clear_history().map_err(|e| e.to_string())
}

// =====================
// UI Settings Commands
// =====================

#[tauri::command]
pub fn get_theme(state: State<'_, AppState>) -> Result<String, String> {
    let db = state.db.lock().unwrap();
    Ok(db.get_setting("theme")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "system".to_string()))
}

#[tauri::command]
pub fn set_theme(theme: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.set_setting("theme", &theme).map_err(|e| e.to_string())
}

// =====================
// System Commands
// =====================

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// =====================
// Permission Commands
// =====================

#[tauri::command]
pub fn get_permission_status() -> crate::permissions::PermissionStatus {
    crate::permissions::get_permission_status()
}

#[tauri::command]
pub fn open_permission_settings(pane: String) -> Result<(), String> {
    crate::permissions::open_system_settings(&pane)
}

#[tauri::command]
pub fn prompt_accessibility_permission() -> bool {
    crate::permissions::prompt_accessibility()
}

#[tauri::command]
pub fn prompt_microphone_permission() -> bool {
    crate::permissions::prompt_microphone()
}

#[tauri::command]
pub fn start_listener(app: AppHandle) -> Result<(), String> {
    crate::listener::start_listener(app);
    Ok(())
}

#[tauri::command]
pub fn restart_listener(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // Stop existing listener
    crate::listener::stop_listener();

    // Reload hotkey from config into state
    let config = Config::load().map_err(|e| e.to_string())?;
    *state.current_hotkey.write().unwrap() = config.input.hotkey;

    // Start fresh
    crate::listener::start_listener(app);
    Ok(())
}

// =====================
// Test Mode Commands
// =====================

#[tauri::command]
pub fn set_test_mode(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    state.test_mode.store(enabled, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn get_test_mode(state: State<'_, AppState>) -> bool {
    use std::sync::atomic::Ordering;
    state.test_mode.load(Ordering::SeqCst)
}
