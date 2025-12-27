# Transcribble

Push-to-talk voice typing using local Whisper models. Hold a hotkey, speak, release to transcribe and auto-type.

## Installation

```bash
cargo install --path .
```

## Quick Start

Run `transcribble` for the first time to launch the setup wizard:

```bash
transcribble
```

The wizard will guide you through:
1. Downloading a Whisper model
2. Choosing your push-to-talk hotkey

Once configured, just run `transcribble` to start. Hold your hotkey to record, release to transcribe.

## Commands

```bash
transcribble              # Start transcription
transcribble setup        # Re-run setup wizard
transcribble config       # View current settings
transcribble config --edit  # Edit config file

transcribble models                    # List downloaded models
transcribble models --available        # List all available models
transcribble models --download base.en # Download a model
transcribble models --use small.en     # Switch active model

transcribble history           # Show recent transcriptions
transcribble history -c 20     # Show last 20 entries
transcribble history --export transcript.txt
transcribble history --clear
```

## Available Models

| Model | Size | Description |
|-------|------|-------------|
| tiny.en | 75 MB | Fastest, English only |
| base.en | 142 MB | Good balance (recommended) |
| small.en | 466 MB | More accurate, slower |
| medium.en | 1.5 GB | Highest accuracy |

Multilingual versions (tiny, base, small, medium) are also available.

## Configuration

Config is stored at `~/.transcribble/config.toml`:

```toml
[model]
path = "/Users/you/.transcribble/ggml-base.en.bin"
name = "base.en"

[input]
hotkey = "RightAlt"

[output]
show_word_count = true
show_duration = true
auto_type = true

[history]
enabled = true
max_entries = 1000  # 0 = unlimited, auto-prunes when exceeded
```

## Hotkey Options

RightAlt, LeftAlt, RightControl, LeftControl, RightShift, LeftShift, Function, F1-F12

## Requirements

- macOS (uses local audio input)
- Accessibility permissions (for auto-typing)
