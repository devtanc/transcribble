use anyhow::Result;

/// All available hotkey options with display names
pub const HOTKEY_OPTIONS: &[(&str, &str)] = &[
    ("RightAlt", "Right Alt key"),
    ("LeftAlt", "Left Alt key"),
    ("RightControl", "Right Control key"),
    ("LeftControl", "Left Control key"),
    ("RightShift", "Right Shift key"),
    ("LeftShift", "Left Shift key"),
    ("Function", "Function (Fn) key"),
    ("F1", "F1 key"),
    ("F2", "F2 key"),
    ("F3", "F3 key"),
    ("F4", "F4 key"),
    ("F5", "F5 key"),
    ("F6", "F6 key"),
    ("F7", "F7 key"),
    ("F8", "F8 key"),
    ("F9", "F9 key"),
    ("F10", "F10 key"),
    ("F11", "F11 key"),
    ("F12", "F12 key"),
];

/// Parse a hotkey string into an rdev::Key
pub fn parse_hotkey(key_str: &str) -> Result<rdev::Key> {
    match key_str {
        "RightAlt" => Ok(rdev::Key::AltGr),
        "LeftAlt" => Ok(rdev::Key::Alt),
        "RightControl" => Ok(rdev::Key::ControlRight),
        "LeftControl" => Ok(rdev::Key::ControlLeft),
        "RightShift" => Ok(rdev::Key::ShiftRight),
        "LeftShift" => Ok(rdev::Key::ShiftLeft),
        "Function" | "Fn" => Ok(rdev::Key::Function),
        "F1" => Ok(rdev::Key::F1),
        "F2" => Ok(rdev::Key::F2),
        "F3" => Ok(rdev::Key::F3),
        "F4" => Ok(rdev::Key::F4),
        "F5" => Ok(rdev::Key::F5),
        "F6" => Ok(rdev::Key::F6),
        "F7" => Ok(rdev::Key::F7),
        "F8" => Ok(rdev::Key::F8),
        "F9" => Ok(rdev::Key::F9),
        "F10" => Ok(rdev::Key::F10),
        "F11" => Ok(rdev::Key::F11),
        "F12" => Ok(rdev::Key::F12),
        _ => Err(anyhow::anyhow!(
            "Unknown hotkey: {}. Valid options: RightAlt, LeftAlt, RightControl, LeftControl, RightShift, LeftShift, Function/Fn, or F1-F12",
            key_str
        )),
    }
}

/// Get the display name for a hotkey
#[allow(dead_code)]
pub fn hotkey_display_name(key_str: &str) -> &str {
    HOTKEY_OPTIONS
        .iter()
        .find(|(k, _)| *k == key_str)
        .map(|(_, name)| *name)
        .unwrap_or(key_str)
}
