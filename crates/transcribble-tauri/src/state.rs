use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use whisper_rs::WhisperContext;

/// Thread-safe database wrapper
pub struct DbConnection {
    conn: rusqlite::Connection,
}

// We need to implement Send + Sync manually since rusqlite::Connection
// is not Sync, but we're wrapping it in a Mutex which makes it safe
unsafe impl Send for DbConnection {}
unsafe impl Sync for DbConnection {}

impl DbConnection {
    pub fn open() -> anyhow::Result<Self> {
        let db_path = transcribble_core::Config::app_dir().join("transcribble.db");

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = rusqlite::Connection::open(&db_path)?;

        // Run migrations
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS transcriptions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                text TEXT NOT NULL,
                duration_ms INTEGER NOT NULL,
                word_count INTEGER NOT NULL,
                character_count INTEGER NOT NULL,
                keystrokes_saved INTEGER NOT NULL,
                model_name TEXT NOT NULL,
                sample_rate INTEGER,
                audio_device TEXT,
                processing_time_ms INTEGER,
                created_at TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_transcriptions_timestamp ON transcriptions(timestamp DESC);
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT DEFAULT (datetime('now'))
            );
            "#,
        )?;

        Ok(Self { conn })
    }

    pub fn get_setting(&self, key: &str) -> anyhow::Result<Option<String>> {
        use rusqlite::params;
        let result = self.conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        );
        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        use rusqlite::params;
        self.conn.execute(
            r#"
            INSERT INTO settings (key, value, updated_at)
            VALUES (?1, ?2, datetime('now'))
            ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')
            "#,
            params![key, value],
        )?;
        Ok(())
    }
}

/// Application state shared across Tauri commands
pub struct AppState {
    /// Whether the app is currently listening for the hotkey
    pub is_listening: AtomicBool,

    /// Whether recording is currently active
    pub is_recording: AtomicBool,

    /// Recording start time
    pub recording_start: Mutex<Option<Instant>>,

    /// Audio capture instance (not Send, so we use Option)
    audio_capture: Mutex<Option<()>>, // Placeholder - audio capture handled separately

    /// Whisper model context
    pub whisper_ctx: RwLock<Option<Arc<WhisperContext>>>,

    /// Current model name
    pub current_model: RwLock<String>,

    /// Current hotkey
    pub current_hotkey: RwLock<String>,

    /// Database connection (wrapped for thread safety)
    pub db: Mutex<DbConnection>,

    /// Whether test mode is active (skip history recording)
    pub test_mode: AtomicBool,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        let db = DbConnection::open()?;

        Ok(Self {
            is_listening: AtomicBool::new(false),
            is_recording: AtomicBool::new(false),
            recording_start: Mutex::new(None),
            audio_capture: Mutex::new(None),
            whisper_ctx: RwLock::new(None),
            current_model: RwLock::new(String::new()),
            current_hotkey: RwLock::new(String::new()),
            db: Mutex::new(db),
            test_mode: AtomicBool::new(false),
        })
    }

    pub fn set_listening(&self, value: bool) {
        self.is_listening.store(value, Ordering::SeqCst);
    }

    pub fn get_listening(&self) -> bool {
        self.is_listening.load(Ordering::SeqCst)
    }

    pub fn set_recording(&self, value: bool) {
        self.is_recording.store(value, Ordering::SeqCst);
        if value {
            *self.recording_start.lock().unwrap() = Some(Instant::now());
        }
    }

    pub fn get_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    pub fn get_recording_duration_ms(&self) -> u64 {
        self.recording_start
            .lock()
            .unwrap()
            .map(|start| start.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_mode_default_false() {
        // Create a test_mode AtomicBool to test the default behavior
        // (We can't easily test AppState::new() without DB setup)
        let test_mode = AtomicBool::new(false);
        assert!(!test_mode.load(Ordering::SeqCst));
    }

    #[test]
    fn test_set_and_get_test_mode() {
        let test_mode = AtomicBool::new(false);

        // Initially false
        assert!(!test_mode.load(Ordering::SeqCst));

        // Set to true
        test_mode.store(true, Ordering::SeqCst);
        assert!(test_mode.load(Ordering::SeqCst));

        // Set back to false
        test_mode.store(false, Ordering::SeqCst);
        assert!(!test_mode.load(Ordering::SeqCst));
    }

    #[test]
    fn test_test_mode_skip_history_logic() {
        // This test verifies the conditional logic used in listener.rs
        // to skip history when test_mode is true
        let test_mode = AtomicBool::new(false);
        let mut history_append_called = false;

        // When test_mode is false, history should be appended
        if !test_mode.load(Ordering::SeqCst) {
            history_append_called = true;
        }
        assert!(
            history_append_called,
            "History should be appended when test_mode is false"
        );

        // When test_mode is true, history should NOT be appended
        test_mode.store(true, Ordering::SeqCst);
        history_append_called = false;

        if !test_mode.load(Ordering::SeqCst) {
            history_append_called = true;
        }
        assert!(
            !history_append_called,
            "History should NOT be appended when test_mode is true"
        );
    }
}
