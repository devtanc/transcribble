# Transcribble - AI Agent Instructions

## Project Overview
Real-time microphone audio streaming to AWS Transcribe using Rust. Single-binary CLI tool that captures audio, streams PCM data to AWS, and displays live transcription results with optional file output.

## Architecture
- **Audio Pipeline**: `cpal` (capture) → `mpsc::channel` (buffer) → AWS SDK stream → transcription results
- **Threading Model**: 
  - Main thread: Tokio async runtime for AWS operations
  - Audio thread: Synchronous CPAL stream (uses `std::thread::park()` to block)
  - Bridge: `tokio::mpsc` channels with `blocking_send` from audio thread
- **Audio Format**: 16kHz mono PCM, i16 samples converted to little-endian bytes for AWS

## Key Constraints
- **Sample rate**: Hardcoded to 16000 Hz (AWS Transcribe requirement for telephony quality)
- **Channels**: Mono only - AWS streaming API requires single channel
- **Buffer size**: `mpsc` channels use 100-slot capacity for back-pressure management
- **Audio conversion**: Must convert `i16` samples to LE bytes via `to_le_bytes()` before AWS transmission

## Development Workflows

### Building & Running
```bash
cargo build --release           # Optimized build
cargo run -- --help            # See CLI options
cargo run -- -l es-US -o output.txt  # Spanish with file output
```

### AWS Authentication
- Uses AWS SDK credential chain (env vars, credentials file, IAM roles)
- `--profile` flag to specify named AWS profile
- `--region` flag for AWS region (default: us-west-2)
- Requires `transcribe:StartStreamTranscription` IAM permission

### Testing Audio
No automated tests (audio hardware dependency). Manual testing:
1. Verify microphone access (macOS requires permission prompt)
2. Check AWS credentials: `aws sts get-caller-identity`
3. Run with output file to verify transcription quality

## Common Modifications

### Adding Language Support
Update `transcribe_stream()` language_code match:
```rust
let language_code = match language {
    "de-DE" => LanguageCode::DeDe,
    // AWS SDK LanguageCode enum has all supported codes
}
```

### Changing Audio Format
Modify constants + CPAL config + AWS params together:
```rust
const SAMPLE_RATE: u32 = 8000;  // Also update media_sample_rate_hertz()
const CHANNELS: u16 = 2;         // AWS requires mono, don't change
```

### Error Handling Patterns
- Audio capture errors: Logged to stderr, don't crash main thread
- AWS errors: Bubble via `anyhow::Result`, terminate gracefully
- Channel send failures: Silent ignore (receiver dropped = intentional shutdown)

## Dependencies Rationale
- `tokio` "full" features: Needed for async AWS SDK + mpsc channels
- `aws-sdk-transcribestreaming`: Specialized SDK (not in main aws-sdk-rust)
- `cpal`: Cross-platform audio I/O (CoreAudio on macOS, WASAPI on Windows)
- `clap` derive: Type-safe CLI parsing with minimal boilerplate
- `futures`: StreamExt trait for polling AWS result stream

## Output Behavior
- **Partial results**: Overwrite current line with `\r` (real-time feedback)
- **Final results**: New line + optional file append
- **File format**: Line-separated transcript segments (no timestamps)
- Console uses emoji indicators (🎤📝✅) - remove if logging to files

## Debugging Tips
- CPAL device selection: Check `host.input_devices()` for available mics
- AWS stream errors: Often credential/permission issues, not code bugs
- Silent output: Verify mic input levels in system settings first
- Buffer overruns: Increase mpsc channel capacity if dropping audio
