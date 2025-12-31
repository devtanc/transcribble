#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod listener;
mod permissions;
mod state;
mod tray;

use state::AppState;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .setup(|app| {
            // Initialize app state
            let app_state = AppState::new().expect("Failed to initialize app state");
            app.manage(app_state);

            // Create system tray
            let _tray = tray::create_tray(app.handle())?;

            // Load config and initialize model if available
            if let Ok(config) = transcribble_core::Config::load() {
                let state = app.state::<AppState>();
                *state.current_model.write().unwrap() = config.model.name.clone();
                *state.current_hotkey.write().unwrap() = config.input.hotkey.clone();

                // Try to load the model in background
                let model_path = config.model.path.to_string_lossy().to_string();
                if std::path::Path::new(&model_path).exists() {
                    if let Ok(ctx) = transcribble_core::load_model(&model_path) {
                        *state.whisper_ctx.write().unwrap() = Some(ctx);
                        println!("Loaded model: {}", config.model.name);
                    }
                }

                // Note: Listener is started via start_listener command after permissions are granted
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Configuration
            commands::get_config,
            commands::save_config,
            // Listening state
            commands::get_listening_state,
            commands::get_recording_state,
            // Model management
            commands::get_available_models,
            commands::get_downloaded_models,
            commands::download_model,
            commands::set_active_model,
            commands::get_active_model,
            // History & Statistics
            commands::get_history,
            commands::get_statistics,
            commands::search_history,
            commands::delete_transcription,
            commands::clear_history,
            // UI Settings
            commands::get_theme,
            commands::set_theme,
            // System
            commands::get_app_version,
            // Permissions
            commands::get_permission_status,
            commands::open_permission_settings,
            commands::prompt_accessibility_permission,
            commands::prompt_microphone_permission,
            commands::start_listener,
            commands::restart_listener,
            // Test Mode
            commands::set_test_mode,
            commands::get_test_mode,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
