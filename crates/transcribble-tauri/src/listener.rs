use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::state::AppState;

/// Global flag to prevent starting multiple listeners
static LISTENER_STARTED: AtomicBool = AtomicBool::new(false);

/// Global flag to signal the listener to stop
static LISTENER_SHOULD_STOP: AtomicBool = AtomicBool::new(false);

/// Stop the listener and reset flags for restart
pub fn stop_listener() {
    LISTENER_SHOULD_STOP.store(true, Ordering::SeqCst);
    // Give threads time to stop
    std::thread::sleep(std::time::Duration::from_millis(100));
    // Reset flags for next start
    LISTENER_STARTED.store(false, Ordering::SeqCst);
    LISTENER_SHOULD_STOP.store(false, Ordering::SeqCst);
}

/// Event payload for transcription complete
#[derive(Clone, serde::Serialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub duration_ms: u64,
    pub word_count: usize,
}

/// Messages from the hotkey listener to the event emitter
enum HotkeyEvent {
    RecordingStarted,
    RecordingStopped,
}

/// Convert hotkey string to CGKeyCode
fn hotkey_to_keycode(hotkey: &str) -> Option<u16> {
    // macOS virtual key codes
    match hotkey.to_lowercase().as_str() {
        "rightalt" | "altgr" => Some(0x3D), // kVK_RightOption
        "leftalt" | "alt" => Some(0x3A),    // kVK_Option
        "rightcontrol" | "rightctrl" => Some(0x3E), // kVK_RightControl
        "leftcontrol" | "leftctrl" | "ctrl" | "control" => Some(0x3B), // kVK_Control
        "rightshift" => Some(0x3C),         // kVK_RightShift
        "leftshift" | "shift" => Some(0x38), // kVK_Shift
        "rightcommand" | "rightcmd" | "rightmeta" => Some(0x36), // kVK_RightCommand
        "leftcommand" | "leftcmd" | "command" | "cmd" | "meta" => Some(0x37), // kVK_Command
        "capslock" => Some(0x39),           // kVK_CapsLock
        "f1" => Some(0x7A),
        "f2" => Some(0x78),
        "f3" => Some(0x63),
        "f4" => Some(0x76),
        "f5" => Some(0x60),
        "f6" => Some(0x61),
        "f7" => Some(0x62),
        "f8" => Some(0x64),
        "f9" => Some(0x65),
        "f10" => Some(0x6D),
        "f11" => Some(0x67),
        "f12" => Some(0x6F),
        "space" => Some(0x31),
        "escape" | "esc" => Some(0x35),
        _ => None,
    }
}

/// Start the global hotkey listener using CGEventTap (macOS native API)
#[cfg(target_os = "macos")]
pub fn start_listener<R: Runtime>(app: AppHandle<R>) {
    // Check permissions before starting
    let permissions = crate::permissions::get_permission_status();

    if !permissions.accessibility {
        eprintln!("Accessibility permission not granted - hotkey detection will not work");
        let _ = app.emit("permission-error", serde_json::json!({
            "permission": "accessibility",
            "message": "Accessibility permission is required for hotkey detection"
        }));
    }

    if !permissions.microphone {
        eprintln!("Microphone permission not granted - audio recording will not work");
        let _ = app.emit("permission-error", serde_json::json!({
            "permission": "microphone",
            "message": "Microphone permission is required for audio recording"
        }));
        // Don't return - still try to set up listener, audio will fail gracefully
    }

    // Prevent starting multiple listeners
    if LISTENER_STARTED.swap(true, Ordering::SeqCst) {
        println!("Listener already started, skipping");
        return;
    }

    let state = app.state::<AppState>();
    let hotkey_str = state.current_hotkey.read().unwrap().clone();

    if hotkey_str.is_empty() {
        println!("No hotkey configured, skipping listener");
        let _ = app.emit("listener-error", serde_json::json!({
            "error": "No hotkey configured"
        }));
        return;
    }

    let target_keycode = match hotkey_to_keycode(&hotkey_str) {
        Some(k) => k,
        None => {
            eprintln!("Unknown hotkey: {}", hotkey_str);
            let _ = app.emit("listener-error", serde_json::json!({
                "error": format!("Unknown hotkey: {}", hotkey_str)
            }));
            return;
        }
    };

    println!("Starting hotkey listener for: {} (keycode: {})", hotkey_str, target_keycode);

    // Set up recording state
    let is_recording = Arc::new(AtomicBool::new(false));
    let is_recording_audio = is_recording.clone();

    // Track recording start time
    let recording_start: Arc<std::sync::Mutex<Option<Instant>>> =
        Arc::new(std::sync::Mutex::new(None));
    let recording_start_main = recording_start.clone();

    // Set up audio capture
    let audio_result = transcribble_core::AudioCapture::new(is_recording_audio);
    let (audio_capture, device_info) = match audio_result {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to initialize audio capture: {}", e);
            let _ = app.emit("listener-error", serde_json::json!({
                "error": format!("Failed to initialize audio: {}", e)
            }));
            return;
        }
    };

    println!("Audio device: {}", device_info.display());

    let audio_buffer = audio_capture.buffer.clone();
    let sample_rate = audio_capture.sample_rate;

    // Keep audio capture alive for the lifetime of the listener
    // We intentionally leak this to prevent the audio stream from being dropped
    // when start_listener() returns. The stream needs to stay alive to capture audio.
    let _audio_capture: &'static _ = Box::leak(Box::new(audio_capture));

    // Clone app handle for the processing thread
    let app_for_processor = app.clone();

    // Create channel for hotkey events
    let (tx, rx) = mpsc::channel::<HotkeyEvent>();

    // Start event emitter thread (handles Tauri API calls safely)
    let app_for_emitter = app.clone();
    let is_recording_emitter = is_recording.clone();
    let recording_start_emitter = recording_start.clone();
    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            match event {
                HotkeyEvent::RecordingStarted => {
                    is_recording_emitter.store(true, Ordering::SeqCst);
                    *recording_start_emitter.lock().unwrap() = Some(Instant::now());
                    if let Some(window) = app_for_emitter.get_webview_window("main") {
                        let state = window.state::<AppState>();
                        state.is_recording.store(true, Ordering::SeqCst);
                    }
                    let _ = app_for_emitter.emit("recording-started", ());
                }
                HotkeyEvent::RecordingStopped => {
                    is_recording_emitter.store(false, Ordering::SeqCst);
                    if let Some(window) = app_for_emitter.get_webview_window("main") {
                        let state = window.state::<AppState>();
                        state.is_recording.store(false, Ordering::SeqCst);
                    }
                    let _ = app_for_emitter.emit("recording-stopped", ());
                }
            }
        }
    });

    // Start CGEventTap listener on a dedicated thread using raw CoreGraphics API
    let tx_clone = tx.clone();
    let app_for_tap = app.clone();
    let hotkey_str_clone = hotkey_str.clone();
    std::thread::spawn(move || {
        use std::os::raw::c_void;

        // CGEventTap constants
        const K_CG_SESSION_EVENT_TAP: u32 = 1;
        const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
        const K_CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;

        // Event types we care about
        const K_CG_EVENT_KEY_DOWN: u64 = 10;
        const K_CG_EVENT_KEY_UP: u64 = 11;
        const K_CG_EVENT_FLAGS_CHANGED: u64 = 12;

        // Event field for keycode
        const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;

        // Flag masks
        const K_CG_EVENT_FLAG_MASK_ALTERNATE: u64 = 0x00080000;
        const K_CG_EVENT_FLAG_MASK_CONTROL: u64 = 0x00040000;
        const K_CG_EVENT_FLAG_MASK_SHIFT: u64 = 0x00020000;
        const K_CG_EVENT_FLAG_MASK_COMMAND: u64 = 0x00100000;

        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventTapCreate(
                tap: u32,
                place: u32,
                options: u32,
                events_of_interest: u64,
                callback: extern "C" fn(
                    proxy: *const c_void,
                    event_type: u64,
                    event: *const c_void,
                    user_info: *mut c_void,
                ) -> *const c_void,
                user_info: *mut c_void,
            ) -> *const c_void;
            fn CGEventTapEnable(tap: *const c_void, enable: bool);
            fn CFMachPortCreateRunLoopSource(
                allocator: *const c_void,
                port: *const c_void,
                order: i64,
            ) -> *const c_void;
            fn CFRunLoopAddSource(
                rl: *const c_void,
                source: *const c_void,
                mode: *const c_void,
            );
            fn CFRunLoopGetCurrent() -> *const c_void;
            fn CFRunLoopRun();
            fn CGEventGetIntegerValueField(event: *const c_void, field: u32) -> i64;
            fn CGEventGetFlags(event: *const c_void) -> u64;
        }

        // Shared state for the callback
        struct CallbackState {
            target_keycode: u16,
            is_key_down: AtomicBool,
            tx: mpsc::Sender<HotkeyEvent>,
        }

        // Store state in a Box to pass to callback
        let state = Box::new(CallbackState {
            target_keycode,
            is_key_down: AtomicBool::new(false),
            tx: tx_clone,
        });
        let state_ptr = Box::into_raw(state);

        extern "C" fn event_callback(
            _proxy: *const c_void,
            event_type: u64,
            event: *const c_void,
            user_info: *mut c_void,
        ) -> *const c_void {
            unsafe {
                let state = &*(user_info as *const CallbackState);
                let keycode = CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) as u16;

                if keycode == state.target_keycode {
                    if event_type == K_CG_EVENT_FLAGS_CHANGED {
                        // Modifier key - check flags
                        let flags = CGEventGetFlags(event);
                        let is_pressed = match state.target_keycode {
                            0x3D | 0x3A => (flags & K_CG_EVENT_FLAG_MASK_ALTERNATE) != 0, // Alt
                            0x3E | 0x3B => (flags & K_CG_EVENT_FLAG_MASK_CONTROL) != 0,   // Control
                            0x3C | 0x38 => (flags & K_CG_EVENT_FLAG_MASK_SHIFT) != 0,     // Shift
                            0x36 | 0x37 => (flags & K_CG_EVENT_FLAG_MASK_COMMAND) != 0,   // Command
                            _ => false,
                        };

                        if is_pressed && !state.is_key_down.load(Ordering::SeqCst) {
                            state.is_key_down.store(true, Ordering::SeqCst);
                            let _ = state.tx.send(HotkeyEvent::RecordingStarted);
                        } else if !is_pressed && state.is_key_down.load(Ordering::SeqCst) {
                            state.is_key_down.store(false, Ordering::SeqCst);
                            let _ = state.tx.send(HotkeyEvent::RecordingStopped);
                        }
                    } else if event_type == K_CG_EVENT_KEY_DOWN {
                        if !state.is_key_down.load(Ordering::SeqCst) {
                            state.is_key_down.store(true, Ordering::SeqCst);
                            let _ = state.tx.send(HotkeyEvent::RecordingStarted);
                        }
                    } else if event_type == K_CG_EVENT_KEY_UP {
                        if state.is_key_down.load(Ordering::SeqCst) {
                            state.is_key_down.store(false, Ordering::SeqCst);
                            let _ = state.tx.send(HotkeyEvent::RecordingStopped);
                        }
                    }
                }

                event // Pass through
            }
        }

        // Event mask: KeyDown, KeyUp, FlagsChanged
        let event_mask = (1u64 << K_CG_EVENT_KEY_DOWN)
            | (1u64 << K_CG_EVENT_KEY_UP)
            | (1u64 << K_CG_EVENT_FLAGS_CHANGED);

        unsafe {
            let tap = CGEventTapCreate(
                K_CG_SESSION_EVENT_TAP,
                K_CG_HEAD_INSERT_EVENT_TAP,
                K_CG_EVENT_TAP_OPTION_LISTEN_ONLY,
                event_mask,
                event_callback,
                state_ptr as *mut c_void,
            );

            if tap.is_null() {
                eprintln!("Failed to create event tap. Make sure the app has Accessibility permissions.");
                let _ = app_for_tap.emit("listener-error", serde_json::json!({
                    "error": "Failed to create event tap (check Accessibility permissions)"
                }));
                let _ = Box::from_raw(state_ptr); // Clean up
                return;
            }

            let run_loop_source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
            if run_loop_source.is_null() {
                eprintln!("Failed to create run loop source");
                let _ = app_for_tap.emit("listener-error", serde_json::json!({
                    "error": "Failed to create run loop source"
                }));
                let _ = Box::from_raw(state_ptr);
                return;
            }

            let run_loop = CFRunLoopGetCurrent();

            // kCFRunLoopCommonModes as raw pointer
            #[link(name = "CoreFoundation", kind = "framework")]
            extern "C" {
                static kCFRunLoopCommonModes: *const c_void;
            }

            CFRunLoopAddSource(run_loop, run_loop_source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);

            println!("Event tap started successfully");
            let _ = app_for_tap.emit("listener-started", serde_json::json!({
                "hotkey": hotkey_str_clone,
                "keycode": target_keycode
            }));
            CFRunLoopRun();

            // Clean up (won't reach here normally)
            let _ = Box::from_raw(state_ptr);
        }
    });

    // Start processing thread
    std::thread::spawn(move || {
        let mut last_recording_state = false;
        let mut enigo = match enigo::Enigo::new(&enigo::Settings::default()) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Failed to initialize enigo: {:?}", e);
                let _ = app_for_processor.emit("listener-error", serde_json::json!({
                    "error": "Failed to initialize keyboard input"
                }));
                return;
            }
        };

        loop {
            // Check if we should stop
            if LISTENER_SHOULD_STOP.load(Ordering::SeqCst) {
                println!("Processing thread stopping");
                break;
            }

            let current_recording_state = is_recording.load(Ordering::SeqCst);

            // Detect transition from recording to not recording
            if last_recording_state && !current_recording_state {
                // Calculate recording duration
                let duration_ms = recording_start_main
                    .lock()
                    .unwrap()
                    .map(|s| s.elapsed().as_millis() as u64)
                    .unwrap_or(0);

                // Get recorded audio
                let audio_data = {
                    let mut buffer = audio_buffer.lock().unwrap();
                    let data = buffer.clone();
                    buffer.clear();
                    data
                };

                if audio_data.is_empty() {
                    // No audio captured - emit error
                    let _ = app_for_processor.emit("transcription-error", "No audio captured");
                    // Must update last_recording_state before continue to avoid infinite loop
                    last_recording_state = current_recording_state;
                    continue;
                }

                // Emit processing event
                let _ = app_for_processor.emit("transcription-processing", ());

                // Get whisper context from state (clone Arc immediately to release lock)
                let state = app_for_processor.state::<AppState>();
                let ctx = {
                    let ctx_guard = state.whisper_ctx.read().unwrap();
                    ctx_guard.as_ref().map(Arc::clone)
                }; // Lock released here - transcription won't block other threads

                if let Some(ref ctx) = ctx {
                    match transcribble_core::transcribe(ctx, &audio_data, sample_rate, false) {
                        Ok(text) => {
                            let text = text.trim().to_string();
                            if text.is_empty() {
                                // Empty transcription (silence) - emit error
                                let _ = app_for_processor.emit(
                                    "transcription-error",
                                    "No speech detected",
                                );
                            } else {
                                let word_count = text.split_whitespace().count();

                                // Emit transcription complete event
                                let _ = app_for_processor.emit(
                                    "transcription-complete",
                                    TranscriptionResult {
                                        text: text.clone(),
                                        duration_ms,
                                        word_count,
                                    },
                                );

                                // Log to history (skip in test mode)
                                if !state.test_mode.load(Ordering::SeqCst) {
                                    let model_name = state.current_model.read().unwrap().clone();
                                    let entry = transcribble_core::TranscriptionEntry::new(
                                        text.clone(),
                                        duration_ms,
                                        model_name,
                                    );
                                    let _ = transcribble_core::history::append_entry(&entry);
                                }

                                // Auto-type the text
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                let _ = enigo::Keyboard::text(&mut enigo, &text);
                            }
                        }
                        Err(e) => {
                            eprintln!("Transcription failed: {}", e);
                            let _ = app_for_processor.emit("transcription-error", e.to_string());
                        }
                    }
                } else {
                    eprintln!("No whisper model loaded");
                    let _ = app_for_processor.emit("transcription-error", "No model loaded");
                }
            }

            last_recording_state = current_recording_state;
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });
}

/// Stub for non-macOS platforms
#[cfg(not(target_os = "macos"))]
pub fn start_listener<R: Runtime>(_app: AppHandle<R>) {
    eprintln!("Hotkey listener is only supported on macOS");
}
