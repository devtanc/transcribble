use anyhow::Result;
use console::style;
use dialoguer::{theme::ColorfulTheme, Select};

use crate::config::Config;
use crate::hotkeys::HOTKEY_OPTIONS;
use crate::models::{download_model, is_model_downloaded, AVAILABLE_MODELS};

/// Run the interactive setup wizard
pub async fn run_wizard() -> Result<Config> {
    println!();
    println!("{}", style("Welcome to Transcribble!").bold().cyan());
    println!("{}", style("========================").dim());
    println!();
    println!("Let's set up voice-to-text transcription on your machine.");
    println!("This wizard will help you download a speech recognition model");
    println!("and configure your preferred push-to-talk hotkey.");
    println!();

    // Step 1: Model selection
    println!("{}", style("Step 1: Choose a Model").bold());
    println!();

    let model_choices: Vec<String> = AVAILABLE_MODELS
        .iter()
        .map(|m| {
            let downloaded = is_model_downloaded(m.name);
            m.display_for_selection(downloaded)
        })
        .collect();

    // Find recommended model index (base.en)
    let default_index = AVAILABLE_MODELS
        .iter()
        .position(|m| m.name == "base.en")
        .unwrap_or(0);

    let model_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a Whisper model")
        .items(&model_choices)
        .default(default_index)
        .interact()?;

    let selected_model = &AVAILABLE_MODELS[model_selection];

    // Download if needed
    let model_path = if !is_model_downloaded(selected_model.name) {
        println!();
        download_model(selected_model.name).await?
    } else {
        println!();
        println!(
            "{} Model '{}' is already downloaded.",
            style("✓").green(),
            selected_model.name
        );
        crate::models::get_model_path(selected_model.name)
    };

    // Step 2: Hotkey selection
    println!();
    println!("{}", style("Step 2: Choose a Hotkey").bold());
    println!();
    println!("Select the key you'll hold down while speaking.");
    println!("Release it to transcribe and type the text.");
    println!();

    let hotkey_choices: Vec<String> = HOTKEY_OPTIONS
        .iter()
        .map(|(key, desc)| format!("{} - {}", key, desc))
        .collect();

    let hotkey_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select your push-to-talk hotkey")
        .items(&hotkey_choices)
        .default(0)
        .interact()?;

    let selected_hotkey = HOTKEY_OPTIONS[hotkey_selection].0.to_string();

    // Create and save config
    let config = Config::new(model_path, selected_model.name.to_string(), selected_hotkey.clone());

    config.save()?;

    // Print summary
    println!();
    println!("{}", style("Setup Complete!").bold().green());
    println!("{}", style("-".repeat(20)).dim());
    println!();
    println!("Configuration saved to: {}", Config::config_path().display());
    println!();
    println!("{}", style("Quick Start:").bold());
    println!("  1. Run 'transcribble' to start");
    println!("  2. Hold {} to record your voice", style(&selected_hotkey).cyan());
    println!("  3. Release to transcribe and auto-type");
    println!();
    println!(
        "{}",
        style("Tip: Run 'transcribble --help' to see all commands.").dim()
    );
    println!();

    Ok(config)
}

/// Run a quick reconfigure (just model and hotkey selection, for existing users)
#[allow(dead_code)]
pub async fn run_reconfigure() -> Result<Config> {
    println!();
    println!("{}", style("Reconfigure Transcribble").bold().cyan());
    println!();

    // Load existing config or use defaults
    let existing_config = Config::load().ok();

    // Model selection
    let model_choices: Vec<String> = AVAILABLE_MODELS
        .iter()
        .map(|m| {
            let downloaded = is_model_downloaded(m.name);
            m.display_for_selection(downloaded)
        })
        .collect();

    let current_model_index = existing_config
        .as_ref()
        .and_then(|c| {
            AVAILABLE_MODELS
                .iter()
                .position(|m| m.name == c.model.name)
        })
        .unwrap_or(2);

    let model_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a Whisper model")
        .items(&model_choices)
        .default(current_model_index)
        .interact()?;

    let selected_model = &AVAILABLE_MODELS[model_selection];

    let model_path = if !is_model_downloaded(selected_model.name) {
        println!();
        download_model(selected_model.name).await?
    } else {
        crate::models::get_model_path(selected_model.name)
    };

    // Hotkey selection
    println!();
    let hotkey_choices: Vec<String> = HOTKEY_OPTIONS
        .iter()
        .map(|(key, desc)| format!("{} - {}", key, desc))
        .collect();

    let current_hotkey_index = existing_config
        .as_ref()
        .and_then(|c| {
            HOTKEY_OPTIONS
                .iter()
                .position(|(k, _)| *k == c.input.hotkey)
        })
        .unwrap_or(0);

    let hotkey_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select your push-to-talk hotkey")
        .items(&hotkey_choices)
        .default(current_hotkey_index)
        .interact()?;

    let selected_hotkey = HOTKEY_OPTIONS[hotkey_selection].0.to_string();

    // Create new config, preserving other settings if they exist
    let config = if let Some(mut existing) = existing_config {
        existing.model.path = model_path;
        existing.model.name = selected_model.name.to_string();
        existing.input.hotkey = selected_hotkey;
        existing
    } else {
        Config::new(model_path, selected_model.name.to_string(), selected_hotkey)
    };

    config.save()?;

    println!();
    println!("{} Configuration updated!", style("✓").green());
    println!();

    Ok(config)
}
