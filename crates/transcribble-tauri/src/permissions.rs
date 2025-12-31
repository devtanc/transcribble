//! macOS permission handling for Input Monitoring and Accessibility

use serde::{Deserialize, Serialize};

/// Permission status for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionStatus {
    pub accessibility: bool,
    pub microphone: bool,
    /// Microphone status: "not_determined", "denied", "authorized", "restricted"
    pub microphone_status: String,
    pub all_granted: bool,
}

/// Check if the app has accessibility permissions (required for auto-typing and hotkeys)
#[cfg(target_os = "macos")]
pub fn check_accessibility_permission(prompt: bool) -> bool {
    use std::ffi::c_void;
    use std::ptr;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFDictionaryCreate(
            allocator: *const c_void,
            keys: *const *const c_void,
            values: *const *const c_void,
            num_values: isize,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> *const c_void;
        fn CFRelease(cf: *const c_void);

        static kCFTypeDictionaryKeyCallBacks: c_void;
        static kCFTypeDictionaryValueCallBacks: c_void;
        static kCFBooleanTrue: *const c_void;
        static kAXTrustedCheckOptionPrompt: *const c_void;
    }

    let result = unsafe {
        if prompt {
            // Create options dictionary with prompt = true
            let keys = [kAXTrustedCheckOptionPrompt];
            let values = [kCFBooleanTrue];

            let options = CFDictionaryCreate(
                ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                1,
                &kCFTypeDictionaryKeyCallBacks as *const _ as *const c_void,
                &kCFTypeDictionaryValueCallBacks as *const _ as *const c_void,
            );

            let trusted = AXIsProcessTrustedWithOptions(options);
            CFRelease(options);
            trusted
        } else {
            AXIsProcessTrustedWithOptions(ptr::null())
        }
    };

    result
}

#[cfg(not(target_os = "macos"))]
pub fn check_accessibility_permission(_prompt: bool) -> bool {
    true
}

/// Get microphone authorization status as a string
/// Returns: "not_determined", "denied", "authorized", or "restricted"
#[cfg(target_os = "macos")]
pub fn get_microphone_status() -> String {
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};

    let media_type = match unsafe { AVMediaTypeAudio } {
        Some(mt) => mt,
        None => {
            eprintln!("Failed to get AVMediaTypeAudio constant");
            return "not_determined".to_string();
        }
    };

    let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(media_type) };

    match status {
        AVAuthorizationStatus::NotDetermined => "not_determined".to_string(),
        AVAuthorizationStatus::Restricted => "restricted".to_string(),
        AVAuthorizationStatus::Denied => "denied".to_string(),
        AVAuthorizationStatus::Authorized => "authorized".to_string(),
        _ => "not_determined".to_string(),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_microphone_status() -> String {
    "authorized".to_string()
}

/// Check microphone permission using AVFoundation
/// Returns true only if permission is explicitly authorized
#[cfg(target_os = "macos")]
pub fn check_microphone_permission() -> bool {
    get_microphone_status() == "authorized"
}

/// Request microphone permission (triggers system prompt)
/// Uses cpal to trigger the macOS microphone permission dialog
#[cfg(target_os = "macos")]
pub fn request_microphone_permission() {
    use cpal::traits::{DeviceTrait, HostTrait};

    // Attempting to get the default input device config will trigger the permission dialog
    // if permission hasn't been granted yet
    let host = cpal::default_host();
    if let Some(device) = host.default_input_device() {
        // Just querying the config is enough to trigger the permission dialog
        let _ = device.default_input_config();
        println!("Microphone permission request initiated via cpal");
    } else {
        eprintln!("No default input device found");
    }
}

#[cfg(not(target_os = "macos"))]
pub fn check_microphone_permission() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
pub fn request_microphone_permission() {
    // No-op on non-macOS
}

/// Prompt for microphone permission and return the updated status
/// Returns true if permission was granted
pub fn prompt_microphone() -> bool {
    request_microphone_permission();
    // Give the system a moment to process the request
    std::thread::sleep(std::time::Duration::from_millis(100));
    check_microphone_permission()
}

/// Get current permission status (never prompts - just checks)
pub fn get_permission_status() -> PermissionStatus {
    // Only check, never prompt - let the UI handle prompting
    let accessibility = check_accessibility_permission(false);
    let microphone_status = get_microphone_status();
    let microphone = microphone_status == "authorized";

    println!(
        "Permission check: accessibility={}, microphone={} (status: {})",
        accessibility, microphone, microphone_status
    );

    PermissionStatus {
        accessibility,
        microphone,
        microphone_status,
        all_granted: accessibility && microphone,
    }
}

/// Request all required permissions on macOS
/// Returns true if all permissions are granted
pub fn request_permissions() -> bool {
    #[cfg(target_os = "macos")]
    {
        // Just check without prompting - we'll let the UI handle prompts
        let status = get_permission_status();

        if status.accessibility {
            println!("✓ Accessibility permission granted");
        } else {
            println!("⚠ Accessibility permission not granted");
        }

        if status.microphone {
            println!("✓ Microphone permission granted");
        } else {
            println!("⚠ Microphone permission not granted");
        }

        status.all_granted
    }

    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Open System Settings to a specific pane
#[cfg(target_os = "macos")]
pub fn open_system_settings(pane: &str) -> Result<(), String> {
    let url = match pane {
        "accessibility" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        "microphone" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
        "input_monitoring" => "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
        _ => return Err(format!("Unknown settings pane: {}", pane)),
    };

    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn open_system_settings(_pane: &str) -> Result<(), String> {
    Ok(())
}

/// Prompt for accessibility permission (shows system dialog)
pub fn prompt_accessibility() -> bool {
    check_accessibility_permission(true)
}
