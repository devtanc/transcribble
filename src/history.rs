use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::config::Config;

/// A single transcription log entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionEntry {
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
    pub model: String,
    pub word_count: usize,
    pub text: String,
}

impl TranscriptionEntry {
    /// Create a new transcription entry
    pub fn new(text: String, duration_ms: u64, model: String) -> Self {
        let word_count = text.split_whitespace().count();
        Self {
            timestamp: Utc::now(),
            duration_ms,
            model,
            word_count,
            text,
        }
    }

    /// Create an entry with a specific timestamp (for testing)
    #[cfg(test)]
    fn with_timestamp(text: String, duration_ms: u64, model: String, timestamp: DateTime<Utc>) -> Self {
        let word_count = text.split_whitespace().count();
        Self {
            timestamp,
            duration_ms,
            model,
            word_count,
            text,
        }
    }

    /// Format for display
    pub fn display(&self) -> String {
        let local_time = self.timestamp.format("%Y-%m-%d %H:%M:%S");
        let duration_secs = self.duration_ms as f64 / 1000.0;
        format!(
            "[{}] ({:.1}s, {} words)\n\"{}\"",
            local_time, duration_secs, self.word_count, self.text
        )
    }
}

/// Get the history file path for the current month
fn current_history_file_in(history_dir: &Path) -> PathBuf {
    let now = Utc::now();
    let filename = format!("transcriptions-{}.jsonl", now.format("%Y-%m"));
    history_dir.join(filename)
}

#[allow(dead_code)]
fn current_history_file() -> PathBuf {
    current_history_file_in(&Config::history_dir())
}

/// Get all history files sorted by date (newest first)
fn list_history_files_in(history_dir: &Path) -> Result<Vec<PathBuf>> {
    if !history_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files: Vec<PathBuf> = fs::read_dir(history_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .map(|ext| ext == "jsonl")
                .unwrap_or(false)
        })
        .collect();

    // Sort by filename (which includes date) in reverse order
    files.sort_by(|a, b| b.cmp(a));

    Ok(files)
}

#[allow(dead_code)]
fn list_history_files() -> Result<Vec<PathBuf>> {
    list_history_files_in(&Config::history_dir())
}

/// Count entries in a specific directory
fn count_entries_in(history_dir: &Path) -> Result<usize> {
    let files = list_history_files_in(history_dir)?;
    let mut count = 0;

    for file_path in files {
        let file = File::open(&file_path)?;
        let reader = BufReader::new(file);
        count += reader.lines().count();
    }

    Ok(count)
}

/// Append entry to a specific directory
fn append_entry_in(entry: &TranscriptionEntry, history_dir: &Path) -> Result<()> {
    fs::create_dir_all(history_dir)?;

    let file_path = current_history_file_in(history_dir);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)?;

    let json = serde_json::to_string(entry)?;
    writeln!(file, "{}", json)?;

    Ok(())
}

/// Prune history in a specific directory
fn prune_history_in(keep_count: usize, history_dir: &Path) -> Result<usize> {
    let files = list_history_files_in(history_dir)?;
    if files.is_empty() {
        return Ok(0);
    }

    // Collect all entries with their source file
    let mut all_entries: Vec<(PathBuf, TranscriptionEntry)> = Vec::new();

    for file_path in &files {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);

        for line in reader.lines().map_while(Result::ok) {
            if let Ok(entry) = serde_json::from_str::<TranscriptionEntry>(&line) {
                all_entries.push((file_path.clone(), entry));
            }
        }
    }

    let total = all_entries.len();
    if total <= keep_count {
        return Ok(0);
    }

    // Sort by timestamp (newest first)
    all_entries.sort_by(|a, b| b.1.timestamp.cmp(&a.1.timestamp));

    // Keep only the most recent entries
    let entries_to_keep: Vec<_> = all_entries.into_iter().take(keep_count).collect();
    let pruned = total - entries_to_keep.len();

    // Group entries by file
    let mut entries_by_file: std::collections::HashMap<PathBuf, Vec<TranscriptionEntry>> =
        std::collections::HashMap::new();

    for (path, entry) in entries_to_keep {
        entries_by_file.entry(path).or_default().push(entry);
    }

    // Rewrite each file with only kept entries, delete empty files
    for file_path in &files {
        if let Some(mut entries) = entries_by_file.remove(file_path) {
            // Sort chronologically for file storage
            entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

            let mut file = File::create(file_path)?;
            for entry in entries {
                let json = serde_json::to_string(&entry)?;
                writeln!(file, "{}", json)?;
            }
        } else {
            // No entries to keep in this file, delete it
            let _ = fs::remove_file(file_path);
        }
    }

    Ok(pruned)
}

/// Read recent entries from a specific directory
fn read_recent_in(count: usize, history_dir: &Path) -> Result<Vec<TranscriptionEntry>> {
    let files = list_history_files_in(history_dir)?;
    let mut entries = Vec::new();

    for file_path in files {
        if entries.len() >= count {
            break;
        }

        let file = File::open(&file_path)?;
        let reader = BufReader::new(file);

        let file_entries: Vec<TranscriptionEntry> = reader
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect();

        for entry in file_entries.into_iter().rev() {
            if entries.len() >= count {
                break;
            }
            entries.push(entry);
        }
    }

    Ok(entries)
}

// ============================================================================
// Public API (uses Config::history_dir())
// ============================================================================

/// Append a transcription entry to the history log
/// If max_entries > 0, will periodically prune old entries to stay under the limit
pub fn append_entry_with_limit(entry: &TranscriptionEntry, max_entries: usize) -> Result<()> {
    let history_dir = Config::history_dir();
    append_entry_in(entry, &history_dir)?;

    // Periodically check if pruning is needed
    if max_entries > 0 {
        let current_count = count_entries_in(&history_dir).unwrap_or(0);
        // Prune when we exceed limit by 20% to batch deletions
        let threshold = max_entries + (max_entries / 5).max(20);
        if current_count > threshold {
            let _ = prune_history_in(max_entries, &history_dir);
        }
    }

    Ok(())
}

/// Append a transcription entry (without automatic pruning)
#[allow(dead_code)]
pub fn append_entry(entry: &TranscriptionEntry) -> Result<()> {
    append_entry_with_limit(entry, 0)
}

/// Prune history to keep only the most recent `keep_count` entries
#[allow(dead_code)]
pub fn prune_history(keep_count: usize) -> Result<usize> {
    prune_history_in(keep_count, &Config::history_dir())
}

/// Read recent transcription entries
pub fn read_recent(count: usize) -> Result<Vec<TranscriptionEntry>> {
    read_recent_in(count, &Config::history_dir())
}

/// Clear all history files
pub fn clear_history() -> Result<()> {
    let history_dir = Config::history_dir();
    if history_dir.exists() {
        fs::remove_dir_all(&history_dir)?;
    }
    Ok(())
}

/// Export history to a file
pub fn export_history(output_path: &str, count: Option<usize>) -> Result<usize> {
    let entries = if let Some(n) = count {
        read_recent(n)?
    } else {
        read_recent(usize::MAX)?
    };

    let mut file = File::create(output_path)?;

    for entry in &entries {
        writeln!(file, "{}", entry.display())?;
        writeln!(file)?;
    }

    Ok(entries.len())
}

/// Get total number of transcriptions
pub fn count_entries() -> Result<usize> {
    count_entries_in(&Config::history_dir())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    // Counter to ensure unique temp dirs even in parallel tests
    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn create_test_dir() -> TempDir {
        let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        tempfile::Builder::new()
            .prefix(&format!("transcribble_test_{}_", count))
            .tempdir()
            .expect("Failed to create temp dir")
    }

    fn create_entry(text: &str, minutes_ago: i64) -> TranscriptionEntry {
        let timestamp = Utc::now() - Duration::minutes(minutes_ago);
        TranscriptionEntry::with_timestamp(
            text.to_string(),
            1000,
            "test-model".to_string(),
            timestamp,
        )
    }

    #[test]
    fn test_entry_creation() {
        let entry = TranscriptionEntry::new(
            "Hello world test".to_string(),
            2500,
            "base.en".to_string(),
        );

        assert_eq!(entry.word_count, 3);
        assert_eq!(entry.duration_ms, 2500);
        assert_eq!(entry.model, "base.en");
        assert_eq!(entry.text, "Hello world test");
    }

    #[test]
    fn test_entry_display() {
        let entry = TranscriptionEntry::new(
            "Test message".to_string(),
            3500,
            "base.en".to_string(),
        );

        let display = entry.display();
        assert!(display.contains("3.5s"));
        assert!(display.contains("2 words"));
        assert!(display.contains("\"Test message\""));
    }

    #[test]
    fn test_append_and_count() {
        let temp_dir = create_test_dir();
        let history_dir = temp_dir.path().to_path_buf();

        // Initially empty
        assert_eq!(count_entries_in(&history_dir).unwrap(), 0);

        // Add entries
        let entry1 = create_entry("First entry", 0);
        append_entry_in(&entry1, &history_dir).unwrap();
        assert_eq!(count_entries_in(&history_dir).unwrap(), 1);

        let entry2 = create_entry("Second entry", 0);
        append_entry_in(&entry2, &history_dir).unwrap();
        assert_eq!(count_entries_in(&history_dir).unwrap(), 2);
    }

    #[test]
    fn test_read_recent_ordering() {
        let temp_dir = create_test_dir();
        let history_dir = temp_dir.path().to_path_buf();

        // Add entries with different timestamps (older first)
        let entry_old = create_entry("Old entry", 60);
        let entry_mid = create_entry("Middle entry", 30);
        let entry_new = create_entry("New entry", 0);

        append_entry_in(&entry_old, &history_dir).unwrap();
        append_entry_in(&entry_mid, &history_dir).unwrap();
        append_entry_in(&entry_new, &history_dir).unwrap();

        // Read recent should return newest first
        let recent = read_recent_in(10, &history_dir).unwrap();
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].text, "New entry");
        assert_eq!(recent[1].text, "Middle entry");
        assert_eq!(recent[2].text, "Old entry");
    }

    #[test]
    fn test_read_recent_limit() {
        let temp_dir = create_test_dir();
        let history_dir = temp_dir.path().to_path_buf();

        // Add 5 entries
        for i in 0..5 {
            let entry = create_entry(&format!("Entry {}", i), (4 - i) as i64);
            append_entry_in(&entry, &history_dir).unwrap();
        }

        // Read only 2
        let recent = read_recent_in(2, &history_dir).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].text, "Entry 4"); // newest
        assert_eq!(recent[1].text, "Entry 3");
    }

    #[test]
    fn test_prune_removes_oldest() {
        let temp_dir = create_test_dir();
        let history_dir = temp_dir.path().to_path_buf();

        // Add 10 entries with increasing timestamps
        for i in 0..10 {
            let entry = create_entry(&format!("Entry {}", i), (9 - i) as i64);
            append_entry_in(&entry, &history_dir).unwrap();
        }

        assert_eq!(count_entries_in(&history_dir).unwrap(), 10);

        // Prune to keep only 5
        let pruned = prune_history_in(5, &history_dir).unwrap();
        assert_eq!(pruned, 5);
        assert_eq!(count_entries_in(&history_dir).unwrap(), 5);

        // Check that the newest 5 remain
        let remaining = read_recent_in(10, &history_dir).unwrap();
        assert_eq!(remaining.len(), 5);
        assert_eq!(remaining[0].text, "Entry 9"); // newest
        assert_eq!(remaining[4].text, "Entry 5"); // oldest remaining
    }

    #[test]
    fn test_prune_no_op_when_under_limit() {
        let temp_dir = create_test_dir();
        let history_dir = temp_dir.path().to_path_buf();

        // Add 5 entries
        for i in 0..5 {
            let entry = create_entry(&format!("Entry {}", i), 0);
            append_entry_in(&entry, &history_dir).unwrap();
        }

        // Try to prune to 10 (more than we have)
        let pruned = prune_history_in(10, &history_dir).unwrap();
        assert_eq!(pruned, 0);
        assert_eq!(count_entries_in(&history_dir).unwrap(), 5);
    }

    #[test]
    fn test_prune_empty_history() {
        let temp_dir = create_test_dir();
        let history_dir = temp_dir.path().to_path_buf();

        let pruned = prune_history_in(5, &history_dir).unwrap();
        assert_eq!(pruned, 0);
    }

    #[test]
    fn test_prune_to_zero() {
        let temp_dir = create_test_dir();
        let history_dir = temp_dir.path().to_path_buf();

        // Add entries
        for i in 0..5 {
            let entry = create_entry(&format!("Entry {}", i), 0);
            append_entry_in(&entry, &history_dir).unwrap();
        }

        // Prune to 0
        let pruned = prune_history_in(0, &history_dir).unwrap();
        assert_eq!(pruned, 5);
        assert_eq!(count_entries_in(&history_dir).unwrap(), 0);
    }

    #[test]
    fn test_prune_deletes_empty_files() {
        let temp_dir = create_test_dir();
        let history_dir = temp_dir.path().to_path_buf();
        fs::create_dir_all(&history_dir).unwrap();

        // Create two history files manually
        let file1 = history_dir.join("transcriptions-2024-01.jsonl");
        let file2 = history_dir.join("transcriptions-2024-02.jsonl");

        // Old entries in file1
        let old_entry = TranscriptionEntry::with_timestamp(
            "Old".to_string(),
            1000,
            "test".to_string(),
            Utc::now() - Duration::days(60),
        );
        let json = serde_json::to_string(&old_entry).unwrap();
        fs::write(&file1, format!("{}\n", json)).unwrap();

        // New entries in file2
        let new_entry = TranscriptionEntry::with_timestamp(
            "New".to_string(),
            1000,
            "test".to_string(),
            Utc::now(),
        );
        let json = serde_json::to_string(&new_entry).unwrap();
        fs::write(&file2, format!("{}\n", json)).unwrap();

        assert!(file1.exists());
        assert!(file2.exists());

        // Prune to keep only 1 (the newest)
        prune_history_in(1, &history_dir).unwrap();

        // Old file should be deleted
        assert!(!file1.exists());
        assert!(file2.exists());
        assert_eq!(count_entries_in(&history_dir).unwrap(), 1);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let entry = TranscriptionEntry::new(
            "Test with \"quotes\" and special chars: é ñ 中文".to_string(),
            1234,
            "medium.en".to_string(),
        );

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: TranscriptionEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(entry.text, parsed.text);
        assert_eq!(entry.duration_ms, parsed.duration_ms);
        assert_eq!(entry.model, parsed.model);
        assert_eq!(entry.word_count, parsed.word_count);
    }

    #[test]
    fn test_large_history_pruning() {
        let temp_dir = create_test_dir();
        let history_dir = temp_dir.path().to_path_buf();

        // Add 100 entries
        for i in 0..100 {
            let entry = create_entry(&format!("Entry {}", i), (99 - i) as i64);
            append_entry_in(&entry, &history_dir).unwrap();
        }

        assert_eq!(count_entries_in(&history_dir).unwrap(), 100);

        // Prune to 25
        let pruned = prune_history_in(25, &history_dir).unwrap();
        assert_eq!(pruned, 75);
        assert_eq!(count_entries_in(&history_dir).unwrap(), 25);

        // Verify we kept the newest
        let remaining = read_recent_in(30, &history_dir).unwrap();
        assert_eq!(remaining.len(), 25);
        assert_eq!(remaining[0].text, "Entry 99");
        assert_eq!(remaining[24].text, "Entry 75");
    }

    #[test]
    fn test_threshold_calculation() {
        // Test that the threshold is calculated correctly
        // threshold = max_entries + max(max_entries / 5, 20)

        // For max_entries = 100: threshold = 100 + 20 = 120
        let max = 100;
        let threshold = max + (max / 5).max(20);
        assert_eq!(threshold, 120);

        // For max_entries = 50: threshold = 50 + 20 = 70 (20 > 10)
        let max = 50;
        let threshold = max + (max / 5).max(20);
        assert_eq!(threshold, 70);

        // For max_entries = 1000: threshold = 1000 + 200 = 1200
        let max = 1000;
        let threshold = max + (max / 5).max(20);
        assert_eq!(threshold, 1200);
    }
}
