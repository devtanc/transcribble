mod output;
mod wizard;

use anyhow::Result;
use clap::{Parser, Subcommand};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use transcribble_core::{
    AudioCapture, Config, TranscriptionEntry,
    parse_hotkey, load_model, transcribe,
    models::{download_model_with_progress, get_model_path, is_model_downloaded, list_downloaded_models, AVAILABLE_MODELS},
    history,
};
use output::OutputManager;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(author, version, about = "Push-to-talk voice typing with Whisper")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to Whisper model file (overrides config)
    #[arg(short, long, global = true)]
    model: Option<String>,

    /// Hotkey for push-to-talk (overrides config)
    #[arg(long, global = true)]
    hotkey: Option<String>,

    /// Show verbose output including whisper initialization details
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Download a model (legacy flag, use 'models --download' instead)
    #[arg(long, hide = true)]
    download_model: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start transcription (default)
    Run,

    /// Run the setup wizard
    Setup,

    /// View or edit configuration
    Config {
        /// Open config file in your default editor
        #[arg(long)]
        edit: bool,
    },

    /// Manage Whisper models
    Models {
        /// List available models for download
        #[arg(long)]
        available: bool,

        /// Download a model
        #[arg(long)]
        download: Option<String>,

        /// Set the active model
        #[arg(long, value_name = "NAME")]
        r#use: Option<String>,
    },

    /// View transcription history
    History {
        /// Clear all history
        #[arg(long)]
        clear: bool,

        /// Export history to a file
        #[arg(long)]
        export: Option<String>,

        /// Number of recent entries to show
        #[arg(short, long, default_value = "10")]
        count: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle legacy --download-model flag
    if let Some(model_name) = &cli.download_model {
        println!(
            "{}",
            style("Note: --download-model is deprecated. Use 'transcribble models --download' instead.").yellow()
        );
        download_model_cli(&model_name).await?;
        return Ok(());
    }

    match cli.command {
        Some(Commands::Setup) => {
            wizard::run_wizard().await?;
        }
        Some(Commands::Config { edit }) => {
            cmd_config(edit)?;
        }
        Some(Commands::Models {
            available,
            download,
            r#use,
        }) => {
            cmd_models(available, download, r#use).await?;
        }
        Some(Commands::History {
            clear,
            export,
            count,
        }) => {
            cmd_history(clear, export, count)?;
        }
        Some(Commands::Run) | None => {
            // Check for first run
            if !Config::exists() && cli.model.is_none() {
                println!(
                    "{}",
                    style("No configuration found. Starting setup wizard...").dim()
                );
                wizard::run_wizard().await?;
            }

            run_transcription(cli.model, cli.hotkey, cli.verbose).await?;
        }
    }

    Ok(())
}

/// Download a model with CLI progress bar
async fn download_model_cli(model_name: &str) -> Result<std::path::PathBuf> {
    let model_info = transcribble_core::get_model_info(model_name)
        .ok_or_else(|| anyhow::anyhow!("Unknown model: {}", model_name))?;

    println!("Downloading {} ({} MB)...", model_info.name, model_info.size_mb);

    let pb = Arc::new(std::sync::Mutex::new(None::<ProgressBar>));
    let pb_clone = pb.clone();

    let path = download_model_with_progress(model_name, Some(move |downloaded: u64, total: u64| {
        let mut pb_guard = pb_clone.lock().unwrap();

        // Initialize progress bar on first callback
        if pb_guard.is_none() && total > 0 {
            let bar = ProgressBar::new(total);
            bar.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                    .unwrap()
                    .progress_chars("#>-"),
            );
            *pb_guard = Some(bar);
        }

        if let Some(ref bar) = *pb_guard {
            bar.set_position(downloaded);
        }
    })).await?;

    // Finish the progress bar
    if let Some(bar) = pb.lock().unwrap().take() {
        bar.finish_and_clear();
    }

    println!("Downloaded to: {}", path.display());
    Ok(path)
}

async fn run_transcription(model_override: Option<String>, hotkey_override: Option<String>, verbose_override: bool) -> Result<()> {
    // Load config
    let config = if Config::exists() {
        Config::load()?
    } else if let Some(model_path) = &model_override {
        // Create temporary config for headless mode
        Config::new(
            model_path.into(),
            "custom".to_string(),
            hotkey_override.clone().unwrap_or_else(|| "RightAlt".to_string()),
        )
    } else {
        return Err(anyhow::anyhow!(
            "No configuration found. Run 'transcribble setup' or provide --model flag."
        ));
    };

    // Apply overrides
    let model_path = model_override.unwrap_or_else(|| config.model.path.to_string_lossy().to_string());
    let hotkey_str = hotkey_override.unwrap_or_else(|| config.input.hotkey.clone());
    let model_name = config.model.name.clone();
    let verbose = verbose_override || config.output.verbose;

    // Load model
    let ctx = load_model(&model_path)?;

    // Parse hotkey
    let hotkey = parse_hotkey(&hotkey_str)?;

    // Set up recording state
    let is_recording = Arc::new(AtomicBool::new(false));
    let is_recording_listener = is_recording.clone();

    // Track recording start time
    let recording_start: Arc<std::sync::Mutex<Option<Instant>>> =
        Arc::new(std::sync::Mutex::new(None));
    let recording_start_listener = recording_start.clone();

    // Listen for hotkey in separate thread
    std::thread::spawn(move || {
        if let Err(e) = rdev::listen(move |event| {
            match event.event_type {
                rdev::EventType::KeyPress(key) if key == hotkey => {
                    if !is_recording_listener.load(Ordering::SeqCst) {
                        is_recording_listener.store(true, Ordering::SeqCst);
                        *recording_start_listener.lock().unwrap() = Some(Instant::now());
                    }
                }
                rdev::EventType::KeyRelease(key) if key == hotkey => {
                    if is_recording_listener.load(Ordering::SeqCst) {
                        is_recording_listener.store(false, Ordering::SeqCst);
                    }
                }
                _ => {}
            }
        }) {
            eprintln!("Error listening for hotkey: {:?}", e);
        }
    });

    // Set up audio capture
    let (audio_capture, device_info) = AudioCapture::new(is_recording.clone())?;

    // Set up output manager
    let output = OutputManager::new(&config);

    // Print startup info
    output.print_startup(VERSION, &model_name, &hotkey_str, &device_info.display());

    // Main loop
    let mut last_recording_state = false;
    let mut enigo = enigo::Enigo::new(&enigo::Settings::default()).unwrap();

    loop {
        let current_recording_state = is_recording.load(Ordering::SeqCst);

        // Show recording duration
        if current_recording_state {
            if let Some(start) = *recording_start.lock().unwrap() {
                let duration = start.elapsed().as_secs_f32();
                output.print_recording(duration);
            }
        }

        // Detect transition from recording to not recording
        if last_recording_state && !current_recording_state {
            output.print_processing();

            // Calculate recording duration
            let duration_ms = recording_start
                .lock()
                .unwrap()
                .map(|s| s.elapsed().as_millis() as u64)
                .unwrap_or(0);
            let duration_secs = duration_ms as f32 / 1000.0;

            // Get recorded audio
            let audio_data = audio_capture.take_audio();

            if !audio_data.is_empty() {
                match transcribe(&ctx, &audio_data, audio_capture.sample_rate, verbose) {
                    Ok(text) => {
                        let text = text.trim().to_string();
                        if !text.is_empty() {
                            output.print_transcription(&text, duration_secs);

                            // Log to history
                            if config.history.enabled {
                                let entry =
                                    TranscriptionEntry::new(text.clone(), duration_ms, model_name.clone());
                                if let Err(e) = history::append_entry_with_limit(&entry, config.history.max_entries) {
                                    eprintln!("Warning: Failed to log transcription: {}", e);
                                }
                            }

                            // Type the text
                            if config.output.auto_type {
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                let _ = enigo::Keyboard::text(&mut enigo, &text);
                            }
                        } else {
                            output.print_ready();
                        }
                    }
                    Err(e) => {
                        output.print_error(&format!("Transcription failed: {}", e));
                        output.print_ready();
                    }
                }
            } else {
                output.print_ready();
            }
        }

        last_recording_state = current_recording_state;
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn cmd_config(edit: bool) -> Result<()> {
    let config_path = Config::config_path();

    if edit {
        if !config_path.exists() {
            return Err(anyhow::anyhow!(
                "No configuration file found. Run 'transcribble setup' first."
            ));
        }

        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
        std::process::Command::new(&editor)
            .arg(&config_path)
            .status()?;
        return Ok(());
    }

    if !Config::exists() {
        println!("No configuration found.");
        println!();
        println!("Run 'transcribble setup' to create one.");
        return Ok(());
    }

    let config = Config::load()?;

    println!("{}", style("Current Configuration").bold());
    println!("{}", style("-".repeat(25)).dim());
    println!();
    println!("Config file: {}", config_path.display());
    println!();
    println!("{}", style("[model]").cyan());
    println!("  name   = {}", config.model.name);
    println!("  path   = {}", config.model.path.display());
    println!();
    println!("{}", style("[input]").cyan());
    println!("  hotkey = {}", config.input.hotkey);
    println!();
    println!("{}", style("[output]").cyan());
    println!("  show_word_count = {}", config.output.show_word_count);
    println!("  show_duration   = {}", config.output.show_duration);
    println!("  auto_type       = {}", config.output.auto_type);
    println!("  verbose         = {}", config.output.verbose);
    println!();
    println!("{}", style("[history]").cyan());
    println!("  enabled     = {}", config.history.enabled);
    println!("  max_entries = {}", config.history.max_entries);
    println!();
    println!(
        "{}",
        style("Use 'transcribble config --edit' to modify.").dim()
    );

    Ok(())
}

async fn cmd_models(available: bool, download: Option<String>, use_model: Option<String>) -> Result<()> {
    if let Some(model_name) = download {
        download_model_cli(&model_name).await?;
        return Ok(());
    }

    if let Some(model_name) = use_model {
        // Verify model exists
        if !is_model_downloaded(&model_name) {
            return Err(anyhow::anyhow!(
                "Model '{}' is not downloaded. Run 'transcribble models --download {}' first.",
                model_name,
                model_name
            ));
        }

        // Update config
        let mut config = if Config::exists() {
            Config::load()?
        } else {
            return Err(anyhow::anyhow!(
                "No configuration found. Run 'transcribble setup' first."
            ));
        };

        config.model.path = get_model_path(&model_name);
        config.model.name = model_name.clone();
        config.save()?;

        println!("{} Now using model: {}", style("✓").green(), model_name);
        return Ok(());
    }

    if available {
        println!("{}", style("Available Whisper Models").bold());
        println!("{}", style("-".repeat(25)).dim());
        println!();

        for model in AVAILABLE_MODELS {
            let downloaded = is_model_downloaded(model.name);
            let status = if downloaded {
                style("[downloaded]").green()
            } else {
                style("").dim()
            };

            println!(
                "  {} ({} MB) {} - {}",
                style(model.name).cyan(),
                model.size_mb,
                status,
                model.description
            );
        }

        println!();
        println!(
            "{}",
            style("Use 'transcribble models --download <name>' to download.").dim()
        );
        return Ok(());
    }

    // Default: list downloaded models
    let downloaded = list_downloaded_models();

    if downloaded.is_empty() {
        println!("No models downloaded yet.");
        println!();
        println!("Run 'transcribble models --available' to see available models.");
        println!("Run 'transcribble models --download base.en' to download the recommended model.");
        return Ok(());
    }

    println!("{}", style("Downloaded Models").bold());
    println!("{}", style("-".repeat(20)).dim());
    println!();

    let active_model = Config::load().ok().map(|c| c.model.name);

    for model in downloaded {
        let is_active = active_model.as_ref().map(|n| n == model.name).unwrap_or(false);
        let marker = if is_active {
            style("*").green()
        } else {
            style(" ").dim()
        };

        println!(
            " {} {} ({} MB) - {}",
            marker,
            style(model.name).cyan(),
            model.size_mb,
            model.description
        );
    }

    println!();
    if active_model.is_some() {
        println!("{}", style("* = active model").dim());
    }
    println!(
        "{}",
        style("Use 'transcribble models --use <name>' to switch models.").dim()
    );

    Ok(())
}

fn cmd_history(clear: bool, export: Option<String>, count: usize) -> Result<()> {
    if clear {
        println!("This will delete all transcription history.");
        print!("Are you sure? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() == "y" {
            history::clear_history()?;
            println!("{} History cleared.", style("✓").green());
        } else {
            println!("Cancelled.");
        }
        return Ok(());
    }

    if let Some(path) = export {
        let exported = history::export_history(&path, Some(count))?;
        println!(
            "{} Exported {} entries to: {}",
            style("✓").green(),
            exported,
            path
        );
        return Ok(());
    }

    // Show recent history
    let entries = history::read_recent(count)?;

    if entries.is_empty() {
        println!("No transcription history yet.");
        return Ok(());
    }

    let num_entries = entries.len();

    println!("{}", style("Recent Transcriptions").bold());
    println!("{}", style("-".repeat(25)).dim());
    println!();

    for entry in &entries {
        println!("{}", entry.display());
        println!();
    }

    let total = history::count_entries()?;
    println!(
        "{}",
        style(format!(
            "Showing {} of {} total entries. Use -c to show more.",
            num_entries.min(count),
            total
        ))
        .dim()
    );

    Ok(())
}
