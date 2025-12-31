use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::config::Config;

/// Database connection wrapper
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

/// A transcription record stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionRecord {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub text: String,
    pub duration_ms: i64,
    pub word_count: i64,
    pub character_count: i64,
    pub keystrokes_saved: i64,
    pub model_name: String,
    pub sample_rate: Option<i64>,
    pub audio_device: Option<String>,
    pub processing_time_ms: Option<i64>,
    pub created_at: String,
}

/// Statistics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Statistics {
    pub total_transcriptions: i64,
    pub total_words: i64,
    pub total_duration_ms: i64,
    pub total_keystrokes_saved: i64,
    pub total_minutes: f64,
}

/// Downloaded model record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRecord {
    pub name: String,
    pub filename: String,
    pub size_bytes: i64,
    pub downloaded_at: Option<String>,
    pub is_active: bool,
}

impl Database {
    /// Open or create the database
    pub fn open() -> Result<Self> {
        let db_path = Self::db_path();

        // Ensure directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        db.run_migrations()?;
        Ok(db)
    }

    /// Get the database file path
    pub fn db_path() -> PathBuf {
        Config::app_dir().join("transcribble.db")
    }

    /// Run database migrations
    fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            r#"
            -- Transcription history table
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

            -- Indexes for common queries
            CREATE INDEX IF NOT EXISTS idx_transcriptions_timestamp
                ON transcriptions(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_transcriptions_created_at
                ON transcriptions(created_at DESC);

            -- UI settings table (key-value store)
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT DEFAULT (datetime('now'))
            );

            -- Downloaded models tracking
            CREATE TABLE IF NOT EXISTS models (
                name TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                downloaded_at TEXT DEFAULT (datetime('now')),
                is_active INTEGER DEFAULT 0
            );

            -- Statistics cache for dashboard performance
            CREATE TABLE IF NOT EXISTS statistics_cache (
                period TEXT PRIMARY KEY,
                total_transcriptions INTEGER NOT NULL,
                total_words INTEGER NOT NULL,
                total_duration_ms INTEGER NOT NULL,
                total_keystrokes_saved INTEGER NOT NULL,
                updated_at TEXT DEFAULT (datetime('now'))
            );
            "#,
        )?;

        Ok(())
    }

    // =====================
    // Transcription methods
    // =====================

    /// Insert a new transcription record
    pub fn insert_transcription(
        &self,
        text: &str,
        duration_ms: i64,
        model_name: &str,
        sample_rate: Option<i64>,
        audio_device: Option<&str>,
        processing_time_ms: Option<i64>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let timestamp = Utc::now().to_rfc3339();
        let word_count = text.split_whitespace().count() as i64;
        let character_count = text.chars().count() as i64;
        let keystrokes_saved = character_count; // Approximate

        conn.execute(
            r#"
            INSERT INTO transcriptions
                (timestamp, text, duration_ms, word_count, character_count,
                 keystrokes_saved, model_name, sample_rate, audio_device, processing_time_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                timestamp,
                text,
                duration_ms,
                word_count,
                character_count,
                keystrokes_saved,
                model_name,
                sample_rate,
                audio_device,
                processing_time_ms
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get recent transcriptions with pagination
    pub fn get_transcriptions(&self, limit: usize, offset: usize) -> Result<Vec<TranscriptionRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, timestamp, text, duration_ms, word_count, character_count,
                   keystrokes_saved, model_name, sample_rate, audio_device,
                   processing_time_ms, created_at
            FROM transcriptions
            ORDER BY timestamp DESC
            LIMIT ?1 OFFSET ?2
            "#,
        )?;

        let records = stmt
            .query_map(params![limit as i64, offset as i64], |row| {
                Ok(TranscriptionRecord {
                    id: row.get(0)?,
                    timestamp: row.get::<_, String>(1)?.parse().unwrap_or_else(|_| Utc::now()),
                    text: row.get(2)?,
                    duration_ms: row.get(3)?,
                    word_count: row.get(4)?,
                    character_count: row.get(5)?,
                    keystrokes_saved: row.get(6)?,
                    model_name: row.get(7)?,
                    sample_rate: row.get(8)?,
                    audio_device: row.get(9)?,
                    processing_time_ms: row.get(10)?,
                    created_at: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    /// Search transcriptions by text
    pub fn search_transcriptions(&self, query: &str, limit: usize) -> Result<Vec<TranscriptionRecord>> {
        let conn = self.conn.lock().unwrap();
        let search_pattern = format!("%{}%", query);

        let mut stmt = conn.prepare(
            r#"
            SELECT id, timestamp, text, duration_ms, word_count, character_count,
                   keystrokes_saved, model_name, sample_rate, audio_device,
                   processing_time_ms, created_at
            FROM transcriptions
            WHERE text LIKE ?1
            ORDER BY timestamp DESC
            LIMIT ?2
            "#,
        )?;

        let records = stmt
            .query_map(params![search_pattern, limit as i64], |row| {
                Ok(TranscriptionRecord {
                    id: row.get(0)?,
                    timestamp: row.get::<_, String>(1)?.parse().unwrap_or_else(|_| Utc::now()),
                    text: row.get(2)?,
                    duration_ms: row.get(3)?,
                    word_count: row.get(4)?,
                    character_count: row.get(5)?,
                    keystrokes_saved: row.get(6)?,
                    model_name: row.get(7)?,
                    sample_rate: row.get(8)?,
                    audio_device: row.get(9)?,
                    processing_time_ms: row.get(10)?,
                    created_at: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    /// Delete a transcription by ID
    pub fn delete_transcription(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM transcriptions WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Clear all transcription history
    pub fn clear_transcriptions(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM transcriptions", [])?;
        Ok(())
    }

    /// Get total count of transcriptions
    pub fn count_transcriptions(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transcriptions",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    // ==================
    // Statistics methods
    // ==================

    /// Get aggregated statistics
    pub fn get_statistics(&self) -> Result<Statistics> {
        let conn = self.conn.lock().unwrap();

        let (total_transcriptions, total_words, total_duration_ms, total_keystrokes_saved):
            (i64, i64, i64, i64) = conn.query_row(
            r#"
            SELECT
                COUNT(*),
                COALESCE(SUM(word_count), 0),
                COALESCE(SUM(duration_ms), 0),
                COALESCE(SUM(keystrokes_saved), 0)
            FROM transcriptions
            "#,
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;

        let total_minutes = total_duration_ms as f64 / 60000.0;

        Ok(Statistics {
            total_transcriptions,
            total_words,
            total_duration_ms,
            total_keystrokes_saved,
            total_minutes,
        })
    }

    // ================
    // Settings methods
    // ================

    /// Get a setting value
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
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

    /// Set a setting value
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO settings (key, value, updated_at)
            VALUES (?1, ?2, datetime('now'))
            ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')
            "#,
            params![key, value],
        )?;
        Ok(())
    }

    // =============
    // Model methods
    // =============

    /// Record a downloaded model
    pub fn record_model_download(&self, name: &str, filename: &str, size_bytes: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO models (name, filename, size_bytes, downloaded_at, is_active)
            VALUES (?1, ?2, ?3, datetime('now'), 0)
            ON CONFLICT(name) DO UPDATE SET
                filename = ?2,
                size_bytes = ?3,
                downloaded_at = datetime('now')
            "#,
            params![name, filename, size_bytes],
        )?;
        Ok(())
    }

    /// Set the active model
    pub fn set_active_model(&self, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // First, deactivate all models
        conn.execute("UPDATE models SET is_active = 0", [])?;
        // Then activate the specified model
        conn.execute(
            "UPDATE models SET is_active = 1 WHERE name = ?1",
            params![name],
        )?;
        Ok(())
    }

    /// Get all downloaded models
    pub fn get_downloaded_models(&self) -> Result<Vec<ModelRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT name, filename, size_bytes, downloaded_at, is_active
            FROM models
            ORDER BY name
            "#,
        )?;

        let records = stmt
            .query_map([], |row| {
                Ok(ModelRecord {
                    name: row.get(0)?,
                    filename: row.get(1)?,
                    size_bytes: row.get(2)?,
                    downloaded_at: row.get(3)?,
                    is_active: row.get::<_, i64>(4)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    /// Get the active model name
    pub fn get_active_model(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT name FROM models WHERE is_active = 1",
            [],
            |row| row.get(0),
        );

        match result {
            Ok(name) => Ok(Some(name)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete a model record
    pub fn delete_model_record(&self, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM models WHERE name = ?1", params![name])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_db() -> (Database, TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let conn = Connection::open(&db_path).unwrap();
        let db = Database {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.run_migrations().unwrap();

        (db, temp_dir)
    }

    #[test]
    fn test_insert_and_get_transcription() {
        let (db, _temp) = create_test_db();

        let id = db
            .insert_transcription(
                "Hello world test",
                2500,
                "base.en",
                Some(16000),
                Some("Built-in Microphone"),
                Some(150),
            )
            .unwrap();

        assert!(id > 0);

        let records = db.get_transcriptions(10, 0).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].text, "Hello world test");
        assert_eq!(records[0].word_count, 3);
        assert_eq!(records[0].duration_ms, 2500);
    }

    #[test]
    fn test_statistics() {
        let (db, _temp) = create_test_db();

        db.insert_transcription("Hello world", 1000, "tiny.en", None, None, None)
            .unwrap();
        db.insert_transcription("Testing one two three", 2000, "tiny.en", None, None, None)
            .unwrap();

        let stats = db.get_statistics().unwrap();
        assert_eq!(stats.total_transcriptions, 2);
        assert_eq!(stats.total_words, 6); // 2 + 4
        assert_eq!(stats.total_duration_ms, 3000);
    }

    #[test]
    fn test_settings() {
        let (db, _temp) = create_test_db();

        assert!(db.get_setting("theme").unwrap().is_none());

        db.set_setting("theme", "dark").unwrap();
        assert_eq!(db.get_setting("theme").unwrap(), Some("dark".to_string()));

        db.set_setting("theme", "light").unwrap();
        assert_eq!(db.get_setting("theme").unwrap(), Some("light".to_string()));
    }

    #[test]
    fn test_search_transcriptions() {
        let (db, _temp) = create_test_db();

        db.insert_transcription("Hello world", 1000, "tiny.en", None, None, None)
            .unwrap();
        db.insert_transcription("Goodbye world", 1000, "tiny.en", None, None, None)
            .unwrap();
        db.insert_transcription("Hello there", 1000, "tiny.en", None, None, None)
            .unwrap();

        let results = db.search_transcriptions("Hello", 10).unwrap();
        assert_eq!(results.len(), 2);
    }
}
