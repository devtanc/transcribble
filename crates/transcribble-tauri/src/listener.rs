use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::state::AppState;

/// Simple timestamped logging helper
fn log(component: &str, message: &str) {
    let now = chrono::Local::now();
    println!("[{}] [{}] {}", now.format("%H:%M:%S%.3f"), component, message);
}

/// Simple timestamped error logging helper
fn log_err(component: &str, message: &str) {
    let now = chrono::Local::now();
    eprintln!("[{}] [{}] ERROR: {}", now.format("%H:%M:%S%.3f"), component, message);
}

/// Global flag to prevent starting multiple listeners
static LISTENER_STARTED: AtomicBool = AtomicBool::new(false);

/// Global flag to signal the listener to stop
static LISTENER_SHOULD_STOP: AtomicBool = AtomicBool::new(false);

/// Global pointer to the event tap for health monitoring
static EVENT_TAP: AtomicPtr<std::os::raw::c_void> = AtomicPtr::new(std::ptr::null_mut());

// CoreGraphics/CoreFoundation FFI declarations for macOS
#[cfg(target_os = "macos")]
mod cg_ffi {
    use std::os::raw::c_void;

    // CGEventTap constants
    pub const K_CG_SESSION_EVENT_TAP: u32 = 1;
    pub const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
    pub const K_CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;

    // Event types
    pub const K_CG_EVENT_KEY_DOWN: u64 = 10;
    pub const K_CG_EVENT_KEY_UP: u64 = 11;
    pub const K_CG_EVENT_FLAGS_CHANGED: u64 = 12;

    // Event field for keycode
    pub const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;

    // Flag masks
    pub const K_CG_EVENT_FLAG_MASK_ALTERNATE: u64 = 0x00080000;
    pub const K_CG_EVENT_FLAG_MASK_CONTROL: u64 = 0x00040000;
    pub const K_CG_EVENT_FLAG_MASK_SHIFT: u64 = 0x00020000;
    pub const K_CG_EVENT_FLAG_MASK_COMMAND: u64 = 0x00100000;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        pub fn CGEventTapCreate(
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
        pub fn CGEventTapEnable(tap: *const c_void, enable: bool);
        pub fn CGEventTapIsEnabled(tap: *const c_void) -> bool;
        pub fn CGEventGetIntegerValueField(event: *const c_void, field: u32) -> i64;
        pub fn CGEventGetFlags(event: *const c_void) -> u64;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub fn CFMachPortCreateRunLoopSource(
            allocator: *const c_void,
            port: *const c_void,
            order: i64,
        ) -> *const c_void;
        pub fn CFRunLoopAddSource(
            rl: *const c_void,
            source: *const c_void,
            mode: *const c_void,
        );
        pub fn CFRunLoopGetMain() -> *const c_void;
        pub static kCFRunLoopCommonModes: *const c_void;
    }
}

/// Stop the listener and reset flags for restart
pub fn stop_listener() {
    log("STOP", "Stopping listener...");
    LISTENER_SHOULD_STOP.store(true, Ordering::SeqCst);
    // Give threads time to stop
    std::thread::sleep(std::time::Duration::from_millis(100));
    // Reset flags for next start
    LISTENER_STARTED.store(false, Ordering::SeqCst);
    LISTENER_SHOULD_STOP.store(false, Ordering::SeqCst);
    // Clear the event tap pointer
    EVENT_TAP.store(std::ptr::null_mut(), Ordering::SeqCst);
    log("STOP", "Listener stopped and flags reset");
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
    log("START", "=== Starting hotkey listener ===");

    // Check permissions before starting
    log("START", "Checking permissions...");
    let permissions = crate::permissions::get_permission_status();

    if !permissions.accessibility {
        log_err("START", "Accessibility permission NOT granted - hotkey detection will not work");
        let _ = app.emit("permission-error", serde_json::json!({
            "permission": "accessibility",
            "message": "Accessibility permission is required for hotkey detection"
        }));
    } else {
        log("START", "Accessibility permission: OK");
    }

    if !permissions.microphone {
        log_err("START", "Microphone permission NOT granted - audio recording will not work");
        let _ = app.emit("permission-error", serde_json::json!({
            "permission": "microphone",
            "message": "Microphone permission is required for audio recording"
        }));
        // Don't return - still try to set up listener, audio will fail gracefully
    } else {
        log("START", "Microphone permission: OK");
    }

    // Prevent starting multiple listeners
    if LISTENER_STARTED.swap(true, Ordering::SeqCst) {
        log("START", "Listener already started, skipping");
        return;
    }

    let state = app.state::<AppState>();
    let hotkey_str = state.current_hotkey.read().unwrap().clone();
    log("START", &format!("Configured hotkey: '{}'", hotkey_str));

    if hotkey_str.is_empty() {
        log_err("START", "No hotkey configured, skipping listener");
        let _ = app.emit("listener-error", serde_json::json!({
            "error": "No hotkey configured"
        }));
        return;
    }

    let target_keycode = match hotkey_to_keycode(&hotkey_str) {
        Some(k) => k,
        None => {
            log_err("START", &format!("Unknown hotkey: {}", hotkey_str));
            let _ = app.emit("listener-error", serde_json::json!({
                "error": format!("Unknown hotkey: {}", hotkey_str)
            }));
            return;
        }
    };

    log("START", &format!("Hotkey '{}' mapped to keycode: 0x{:02X} ({})", hotkey_str, target_keycode, target_keycode));

    // Set up recording state
    let is_recording = Arc::new(AtomicBool::new(false));
    let is_recording_audio = is_recording.clone();

    // Track recording start time
    let recording_start: Arc<std::sync::Mutex<Option<Instant>>> =
        Arc::new(std::sync::Mutex::new(None));
    let recording_start_main = recording_start.clone();

    // Set up audio capture
    log("START", "Initializing audio capture...");
    let audio_result = transcribble_core::AudioCapture::new(is_recording_audio);
    let (audio_capture, device_info) = match audio_result {
        Ok(r) => r,
        Err(e) => {
            log_err("START", &format!("Failed to initialize audio capture: {}", e));
            let _ = app.emit("listener-error", serde_json::json!({
                "error": format!("Failed to initialize audio: {}", e)
            }));
            return;
        }
    };

    log("START", &format!("Audio device: {}", device_info.display()));

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
    log("START", "Starting emitter thread...");
    let app_for_emitter = app.clone();
    let is_recording_emitter = is_recording.clone();
    let recording_start_emitter = recording_start.clone();
    std::thread::spawn(move || {
        log("EMITTER", "Emitter thread started, waiting for hotkey events...");
        while let Ok(event) = rx.recv() {
            match event {
                HotkeyEvent::RecordingStarted => {
                    log("EMITTER", "Received RecordingStarted event");
                    is_recording_emitter.store(true, Ordering::SeqCst);
                    *recording_start_emitter.lock().unwrap() = Some(Instant::now());
                    if let Some(window) = app_for_emitter.get_webview_window("main") {
                        let state = window.state::<AppState>();
                        state.is_recording.store(true, Ordering::SeqCst);
                    }
                    log("EMITTER", "Emitting 'recording-started' to frontend");
                    let _ = app_for_emitter.emit("recording-started", ());
                }
                HotkeyEvent::RecordingStopped => {
                    log("EMITTER", "Received RecordingStopped event");
                    is_recording_emitter.store(false, Ordering::SeqCst);
                    if let Some(window) = app_for_emitter.get_webview_window("main") {
                        let state = window.state::<AppState>();
                        state.is_recording.store(false, Ordering::SeqCst);
                    }
                    log("EMITTER", "Emitting 'recording-stopped' to frontend");
                    let _ = app_for_emitter.emit("recording-stopped", ());
                }
            }
        }
        log("EMITTER", "Emitter thread exiting (channel closed)");
    });

    // Set up CGEventTap and add to main run loop for global hotkey detection
    // Note: We use the MAIN run loop, not a thread's run loop, because:
    // 1. Main run loop is always active and properly integrated with macOS event system
    // 2. Thread run loops require CFRunLoopRun() which blocks, but aren't always reliable for global events
    // 3. Using main run loop allows hotkeys to work even when app is in background
    log("START", "Setting up CGEventTap...");
    let tx_clone = tx.clone();
    let app_for_tap = app.clone();
    let hotkey_str_clone = hotkey_str.clone();

    use std::os::raw::c_void;
    use cg_ffi::*;

    // Shared state for the callback - must be 'static since callback is C
    struct CallbackState {
        target_keycode: u16,
        is_key_down: AtomicBool,
        tx: mpsc::Sender<HotkeyEvent>,
    }

    // Store state in a Box and leak it to get a 'static reference for the C callback
    let callback_state = Box::new(CallbackState {
        target_keycode,
        is_key_down: AtomicBool::new(false),
        tx: tx_clone,
    });
    let state_ptr = Box::into_raw(callback_state);

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
                let now = chrono::Local::now();
                let ts = now.format("%H:%M:%S%.3f");

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
                        println!("[{}] [CALLBACK] Hotkey PRESSED (modifier flags changed)", ts);
                        state.is_key_down.store(true, Ordering::SeqCst);
                        let _ = state.tx.send(HotkeyEvent::RecordingStarted);
                    } else if !is_pressed && state.is_key_down.load(Ordering::SeqCst) {
                        println!("[{}] [CALLBACK] Hotkey RELEASED (modifier flags changed)", ts);
                        state.is_key_down.store(false, Ordering::SeqCst);
                        let _ = state.tx.send(HotkeyEvent::RecordingStopped);
                    }
                } else if event_type == K_CG_EVENT_KEY_DOWN {
                    if !state.is_key_down.load(Ordering::SeqCst) {
                        println!("[{}] [CALLBACK] Hotkey PRESSED (key down)", ts);
                        state.is_key_down.store(true, Ordering::SeqCst);
                        let _ = state.tx.send(HotkeyEvent::RecordingStarted);
                    }
                } else if event_type == K_CG_EVENT_KEY_UP {
                    if state.is_key_down.load(Ordering::SeqCst) {
                        println!("[{}] [CALLBACK] Hotkey RELEASED (key up)", ts);
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

    log("START", "Creating CGEventTap with session-level tap...");
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
            log_err("START", "Failed to create event tap - check Accessibility permissions");
            let _ = app_for_tap.emit("listener-error", serde_json::json!({
                "error": "Failed to create event tap (check Accessibility permissions)"
            }));
            let _ = Box::from_raw(state_ptr); // Clean up
            LISTENER_STARTED.store(false, Ordering::SeqCst);
            return;
        }

        log("START", &format!("Event tap created at {:?}", tap));

        // Store tap pointer for health monitoring
        EVENT_TAP.store(tap as *mut c_void, Ordering::SeqCst);

        log("START", "Creating run loop source...");
        let run_loop_source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        if run_loop_source.is_null() {
            log_err("START", "Failed to create run loop source");
            let _ = app_for_tap.emit("listener-error", serde_json::json!({
                "error": "Failed to create run loop source"
            }));
            let _ = Box::from_raw(state_ptr);
            EVENT_TAP.store(std::ptr::null_mut(), Ordering::SeqCst);
            LISTENER_STARTED.store(false, Ordering::SeqCst);
            return;
        }

        log("START", &format!("Run loop source created at {:?}", run_loop_source));

        // Use MAIN run loop instead of current thread's run loop
        // This is critical for global hotkey detection to work when app is in background
        let main_run_loop = CFRunLoopGetMain();
        log("START", &format!("Got main run loop at {:?}", main_run_loop));

        log("START", "Adding source to main run loop...");
        CFRunLoopAddSource(main_run_loop, run_loop_source, kCFRunLoopCommonModes);

        log("START", "Enabling event tap...");
        CGEventTapEnable(tap, true);

        log("START", "=== Event tap setup complete ===");
        log("START", &format!("Listening for hotkey: {} (keycode: 0x{:02X})", hotkey_str_clone, target_keycode));
        let _ = app_for_tap.emit("listener-started", serde_json::json!({
            "hotkey": hotkey_str_clone,
            "keycode": target_keycode
        }));

        // Note: We do NOT call CFRunLoopRun() because:
        // 1. The main run loop is already running (Tauri's event loop)
        // 2. The event tap source is now part of the main run loop
        // 3. Events will be delivered to our callback automatically
    }

    // Start watchdog thread to monitor event tap health
    // macOS can disable event taps if they become unresponsive or there are permission issues
    log("START", "Starting watchdog thread...");
    let app_for_watchdog = app.clone();
    std::thread::spawn(move || {
        log("WATCHDOG", "Watchdog thread started, monitoring event tap health...");
        let mut check_count = 0u64;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));

            if LISTENER_SHOULD_STOP.load(Ordering::SeqCst) {
                log("WATCHDOG", "Watchdog stopping (LISTENER_SHOULD_STOP=true)");
                break;
            }

            let tap = EVENT_TAP.load(Ordering::SeqCst);
            if tap.is_null() {
                log("WATCHDOG", "Event tap is null, skipping health check");
                continue;
            }

            check_count += 1;
            unsafe {
                let is_enabled = CGEventTapIsEnabled(tap);
                if check_count % 15 == 0 {
                    // Log status every 30 seconds (15 checks * 2 seconds)
                    log("WATCHDOG", &format!("Health check #{}: tap enabled = {}", check_count, is_enabled));
                }

                if !is_enabled {
                    log("WATCHDOG", "Event tap was DISABLED by system, attempting re-enable...");
                    CGEventTapEnable(tap, true);

                    // Check if re-enable succeeded
                    if !CGEventTapIsEnabled(tap) {
                        log_err("WATCHDOG", "Failed to re-enable event tap - check Accessibility permissions");
                        let _ = app_for_watchdog.emit("listener-error", serde_json::json!({
                            "error": "Event tap disabled by system (check Accessibility permissions)"
                        }));
                    } else {
                        log("WATCHDOG", "Event tap re-enabled successfully!");
                        let _ = app_for_watchdog.emit("listener-recovered", serde_json::json!({
                            "message": "Hotkey listener recovered"
                        }));
                    }
                }
            }
        }
        log("WATCHDOG", "Watchdog thread exited");
    });

    // Start processing thread
    log("START", "Starting processing thread...");
    std::thread::spawn(move || {
        log("PROCESS", "Processing thread started");
        let mut last_recording_state = false;
        log("PROCESS", "Initializing enigo for auto-typing...");
        let mut enigo = match enigo::Enigo::new(&enigo::Settings::default()) {
            Ok(e) => {
                log("PROCESS", "Enigo initialized successfully");
                e
            }
            Err(e) => {
                log_err("PROCESS", &format!("Failed to initialize enigo: {:?}", e));
                let _ = app_for_processor.emit("listener-error", serde_json::json!({
                    "error": "Failed to initialize keyboard input"
                }));
                return;
            }
        };

        log("PROCESS", "Entering main processing loop...");
        loop {
            // Check if we should stop
            if LISTENER_SHOULD_STOP.load(Ordering::SeqCst) {
                log("PROCESS", "Processing thread stopping (LISTENER_SHOULD_STOP=true)");
                break;
            }

            let current_recording_state = is_recording.load(Ordering::SeqCst);

            // Detect transition from recording to not recording
            if last_recording_state && !current_recording_state {
                log("PROCESS", "Recording stopped - processing audio...");

                // Calculate recording duration
                let duration_ms = recording_start_main
                    .lock()
                    .unwrap()
                    .map(|s| s.elapsed().as_millis() as u64)
                    .unwrap_or(0);

                log("PROCESS", &format!("Recording duration: {}ms", duration_ms));

                // Get recorded audio
                let audio_data = {
                    let mut buffer = audio_buffer.lock().unwrap();
                    let data = buffer.clone();
                    buffer.clear();
                    data
                };

                log("PROCESS", &format!("Audio buffer size: {} samples", audio_data.len()));

                if audio_data.is_empty() {
                    log_err("PROCESS", "No audio captured - buffer was empty");
                    let _ = app_for_processor.emit("transcription-error", "No audio captured");
                    last_recording_state = current_recording_state;
                    continue;
                }

                // Emit processing event
                log("PROCESS", "Emitting 'transcription-processing' event");
                let _ = app_for_processor.emit("transcription-processing", ());

                // Get whisper context from state (clone Arc immediately to release lock)
                let state = app_for_processor.state::<AppState>();
                let ctx = {
                    let ctx_guard = state.whisper_ctx.read().unwrap();
                    ctx_guard.as_ref().map(Arc::clone)
                }; // Lock released here - transcription won't block other threads

                if let Some(ref ctx) = ctx {
                    log("PROCESS", "Starting transcription...");
                    let transcribe_start = Instant::now();
                    match transcribble_core::transcribe(ctx, &audio_data, sample_rate, false) {
                        Ok(text) => {
                            let transcribe_time = transcribe_start.elapsed().as_millis();
                            let text = text.trim().to_string();
                            log("PROCESS", &format!("Transcription completed in {}ms", transcribe_time));

                            if text.is_empty() {
                                log("PROCESS", "Transcription result was empty (no speech detected)");
                                let _ = app_for_processor.emit(
                                    "transcription-error",
                                    "No speech detected",
                                );
                            } else {
                                let word_count = text.split_whitespace().count();
                                log("PROCESS", &format!("Transcription: \"{}\" ({} words)", text, word_count));

                                // Emit transcription complete event
                                log("PROCESS", "Emitting 'transcription-complete' event");
                                let _ = app_for_processor.emit(
                                    "transcription-complete",
                                    TranscriptionResult {
                                        text: text.clone(),
                                        duration_ms,
                                        word_count,
                                    },
                                );

                                // Log to history (skip in test mode)
                                let test_mode = state.test_mode.load(Ordering::SeqCst);
                                if !test_mode {
                                    log("PROCESS", "Saving to history...");
                                    let model_name = state.current_model.read().unwrap().clone();
                                    let entry = transcribble_core::TranscriptionEntry::new(
                                        text.clone(),
                                        duration_ms,
                                        model_name,
                                    );
                                    let _ = transcribble_core::history::append_entry(&entry);
                                } else {
                                    log("PROCESS", "Test mode enabled - skipping history save");
                                }

                                // Auto-type the text
                                log("PROCESS", "Auto-typing text...");
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                match enigo::Keyboard::text(&mut enigo, &text) {
                                    Ok(_) => log("PROCESS", "Auto-type completed"),
                                    Err(e) => log_err("PROCESS", &format!("Auto-type failed: {:?}", e)),
                                }
                            }
                        }
                        Err(e) => {
                            log_err("PROCESS", &format!("Transcription failed: {}", e));
                            let _ = app_for_processor.emit("transcription-error", e.to_string());
                        }
                    }
                } else {
                    log_err("PROCESS", "No whisper model loaded");
                    let _ = app_for_processor.emit("transcription-error", "No model loaded");
                }
            }

            last_recording_state = current_recording_state;
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        log("PROCESS", "Processing thread exited");
    });

    log("START", "=== Listener startup complete ===");
}

/// Stub for non-macOS platforms
#[cfg(not(target_os = "macos"))]
pub fn start_listener<R: Runtime>(_app: AppHandle<R>) {
    eprintln!("Hotkey listener is only supported on macOS");
}
