use anyhow::Result;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::audio::resample;

/// Execute a closure with stderr suppressed (redirected to /dev/null)
fn with_stderr_suppressed<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let devnull = File::open("/dev/null").expect("Failed to open /dev/null");
    let stderr_fd = 2;
    let saved_fd = unsafe { libc::dup(stderr_fd) };
    unsafe { libc::dup2(devnull.as_raw_fd(), stderr_fd) };

    let result = f();

    unsafe { libc::dup2(saved_fd, stderr_fd) };
    unsafe { libc::close(saved_fd) };
    result
}

/// Load a Whisper model from a file path
pub fn load_model(model_path: &str) -> Result<Arc<WhisperContext>> {
    let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
        .map_err(|e| anyhow::anyhow!("Failed to load Whisper model: {}", e))?;
    Ok(Arc::new(ctx))
}

/// Transcribe audio data using Whisper
pub fn transcribe(ctx: &WhisperContext, audio: &[f32], sample_rate: u32, verbose: bool) -> Result<String> {
    // Resample to 16kHz if needed (Whisper requires 16kHz)
    let audio_16k = if sample_rate != 16000 {
        resample(audio, sample_rate, 16000)
    } else {
        audio.to_vec()
    };

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    // Create whisper state, suppressing stderr output unless verbose mode is enabled
    let state_result = if verbose {
        ctx.create_state()
    } else {
        with_stderr_suppressed(|| ctx.create_state())
    };

    let mut state = state_result
        .map_err(|e| anyhow::anyhow!("Failed to create Whisper state: {}", e))?;

    state
        .full(params, &audio_16k)
        .map_err(|e| anyhow::anyhow!("Transcription failed: {}", e))?;

    let num_segments = state
        .full_n_segments()
        .map_err(|e| anyhow::anyhow!("Failed to get segments: {}", e))?;

    let mut result = String::new();
    for i in 0..num_segments {
        let segment = state
            .full_get_segment_text(i)
            .map_err(|e| anyhow::anyhow!("Failed to get segment {}: {}", i, e))?;
        result.push_str(&segment);
    }

    Ok(result)
}
