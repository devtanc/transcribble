use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle, Manager, Runtime,
};

/// Tray state enum for icon updates
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayState {
    Idle,
    Listening,
    Recording,
}

/// Create the system tray icon and menu
pub fn create_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<TrayIcon<R>> {
    let quit = MenuItem::with_id(app, "quit", "Quit Transcribble", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Hide Window", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show, &hide, &quit])?;

    let tray = TrayIconBuilder::new()
        .icon(tauri::include_image!("icons/tray-icon.png"))
        .icon_as_template(true)
        .menu(&menu)
        .tooltip("Transcribble - Idle")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                app.exit(0);
            }
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "hide" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click { button, .. } = event {
                if button == tauri::tray::MouseButton::Left {
                    let app = tray.app_handle();
                    if let Some(window) = app.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
            }
        })
        .build(app)?;

    Ok(tray)
}

/// Update the tray icon based on state
pub fn update_tray_state<R: Runtime>(tray: &TrayIcon<R>, state: TrayState) {
    let tooltip = match state {
        TrayState::Idle => "Transcribble - Idle",
        TrayState::Listening => "Transcribble - Listening",
        TrayState::Recording => "Transcribble - Recording...",
    };

    let _ = tray.set_tooltip(Some(tooltip));

    // In a full implementation, we would also update the icon here
    // based on the state. For now, we just update the tooltip.
}
