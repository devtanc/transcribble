use console::{style, Term};
use std::io::{self, Write};

use transcribble_core::Config;

/// Manages styled console output
pub struct OutputManager {
    term: Term,
    show_duration: bool,
    show_word_count: bool,
}

impl OutputManager {
    pub fn new(config: &Config) -> Self {
        Self {
            term: Term::stdout(),
            show_duration: config.output.show_duration,
            show_word_count: config.output.show_word_count,
        }
    }

    /// Print the startup banner
    pub fn print_startup(&self, version: &str, model_name: &str, hotkey: &str, device: &str) {
        println!("{} v{}", style("transcribble").bold().cyan(), version);
        println!("{}", style("-".repeat(30)).dim());
        println!(
            "Model:  {} {}",
            style(model_name).white(),
            style("(loaded)").dim()
        );
        println!(
            "Hotkey: {} {}",
            style(hotkey).white(),
            style("(hold to record)").dim()
        );
        println!("Device: {}", style(device).dim());
        println!();
        println!(
            "{}",
            style("Ready. Press Ctrl+C to exit.").green()
        );
        println!();
    }

    /// Print recording status with duration
    pub fn print_recording(&self, duration_secs: f32) {
        let _ = self.term.clear_line();
        print!(
            "\r{} {:.1}s",
            style("[Recording]").yellow().bold(),
            duration_secs
        );
        let _ = io::stdout().flush();
    }

    /// Print processing message
    pub fn print_processing(&self) {
        let _ = self.term.clear_line();
        println!("\r{}", style("[Processing...]").blue());
    }

    /// Print the transcription result
    pub fn print_transcription(&self, text: &str, duration_secs: f32) {
        let word_count = text.split_whitespace().count();

        let mut stats = Vec::new();
        if self.show_duration {
            stats.push(format!("{:.1}s", duration_secs));
        }
        if self.show_word_count {
            stats.push(format!("{} words", word_count));
        }

        if !stats.is_empty() {
            println!(
                "{} ({}):",
                style("Transcribed").green(),
                stats.join(", ")
            );
        } else {
            println!("{}:", style("Transcribed").green());
        }
        println!("\"{}\"", text);
        println!();
        println!("{}", style("Ready.").dim());
        println!();
    }

    /// Print ready message
    pub fn print_ready(&self) {
        println!("{}", style("Ready.").dim());
        println!();
    }

    /// Print an error message
    pub fn print_error(&self, message: &str) {
        eprintln!("{}: {}", style("Error").red().bold(), message);
    }

    /// Print a success message
    #[allow(dead_code)]
    pub fn print_success(&self, message: &str) {
        println!("{} {}", style("✓").green(), message);
    }

    /// Print an info message
    #[allow(dead_code)]
    pub fn print_info(&self, message: &str) {
        println!("{}", style(message).dim());
    }
}

/// Simple output functions for cases where we don't have a config yet
#[allow(dead_code)]
pub fn print_error(message: &str) {
    eprintln!("{}: {}", style("Error").red().bold(), message);
}

#[allow(dead_code)]
pub fn print_success(message: &str) {
    println!("{} {}", style("✓").green(), message);
}

#[allow(dead_code)]
pub fn print_info(message: &str) {
    println!("{}", style(message).dim());
}

#[allow(dead_code)]
pub fn print_header(text: &str) {
    println!();
    println!("{}", style(text).bold().cyan());
    println!("{}", style("-".repeat(text.len())).dim());
}
