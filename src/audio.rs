use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Information about the audio device
pub struct DeviceInfo {
    pub name: String,
    pub sample_rate: u32,
    #[allow(dead_code)]
    pub channels: u16,
    #[allow(dead_code)]
    pub sample_format: SampleFormat,
}

impl DeviceInfo {
    /// Format for display
    pub fn display(&self) -> String {
        format!("{} ({}kHz)", self.name, self.sample_rate / 1000)
    }
}

/// Audio capture system
pub struct AudioCapture {
    pub buffer: Arc<Mutex<Vec<f32>>>,
    pub sample_rate: u32,
    _stream: Stream,
}

impl AudioCapture {
    /// Set up audio capture from the default input device
    pub fn new(is_recording: Arc<AtomicBool>) -> Result<(Self, DeviceInfo)> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device available"))?;

        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());

        let supported_config = device
            .default_input_config()
            .map_err(|e| anyhow::anyhow!("Failed to get default input config: {}", e))?;

        let sample_rate = supported_config.sample_rate().0;
        let channels = supported_config.channels();
        let sample_format = supported_config.sample_format();

        let device_info = DeviceInfo {
            name: device_name,
            sample_rate,
            channels,
            sample_format,
        };

        let audio_buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let audio_buffer_capture = audio_buffer.clone();
        let is_recording_capture = is_recording;

        let stream = match sample_format {
            SampleFormat::F32 => {
                device.build_input_stream(
                    &supported_config.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if is_recording_capture.load(Ordering::SeqCst) {
                            let mut buffer = audio_buffer_capture.lock().unwrap();
                            if channels == 1 {
                                buffer.extend_from_slice(data);
                            } else {
                                for frame in data.chunks_exact(channels as usize) {
                                    let sum: f32 = frame.iter().sum();
                                    buffer.push(sum / channels as f32);
                                }
                            }
                        }
                    },
                    |err| eprintln!("Stream error: {}", err),
                    None,
                )?
            }
            SampleFormat::I16 => {
                device.build_input_stream(
                    &supported_config.into(),
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if is_recording_capture.load(Ordering::SeqCst) {
                            let mut buffer = audio_buffer_capture.lock().unwrap();
                            if channels == 1 {
                                buffer.extend(data.iter().map(|&s| s as f32 / 32768.0));
                            } else {
                                for frame in data.chunks_exact(channels as usize) {
                                    let sum: f32 =
                                        frame.iter().map(|&s| s as f32 / 32768.0).sum();
                                    buffer.push(sum / channels as f32);
                                }
                            }
                        }
                    },
                    |err| eprintln!("Stream error: {}", err),
                    None,
                )?
            }
            _ => return Err(anyhow::anyhow!("Unsupported sample format: {:?}", sample_format)),
        };

        stream.play()?;

        Ok((
            Self {
                buffer: audio_buffer,
                sample_rate,
                _stream: stream,
            },
            device_info,
        ))
    }

    /// Take the recorded audio from the buffer
    pub fn take_audio(&self) -> Vec<f32> {
        let mut buffer = self.buffer.lock().unwrap();
        let data = buffer.clone();
        buffer.clear();
        data
    }
}

/// Resample audio to a different sample rate using linear interpolation
pub fn resample(audio: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return audio.to_vec();
    }

    let ratio = to_rate as f32 / from_rate as f32;
    let output_len = (audio.len() as f32 * ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_index = i as f32 / ratio;
        let src_index_floor = src_index.floor() as usize;
        let src_index_ceil = (src_index_floor + 1).min(audio.len() - 1);
        let frac = src_index - src_index_floor as f32;

        let sample = audio[src_index_floor] * (1.0 - frac) + audio[src_index_ceil] * frac;
        output.push(sample);
    }

    output
}
