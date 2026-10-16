//! The window, the tray icon and notifications. Lockstep runs without its window while it follows
//! an app (the shortcut starts it that way); the window opens on demand and closes for real, so a
//! closed window costs nothing.

use crate::lang;
use crate::state::{lock, Shared};
use lockstep_core::session::Phase;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WebviewWindowBuilder};

const TRAY: &str = "lockstep";

/// Shows the window, making it if it is not there.
pub fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let Some(config) = app.config().app.windows.iter().find(|w| w.label == "main").cloned() else {
        return;
    };
    if let Ok(builder) = WebviewWindowBuilder::from_config(app, &config) {
        if builder.build().is_ok() {
            app.state::<Arc<Shared>>().window_open.store(true, Ordering::SeqCst);
        }
    }
}

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let words = lang::pick(&app.state::<Arc<Shared>>().settings().language);
    let open = MenuItem::with_id(app, "open", words.open(), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", words.quit(), true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut builder = TrayIconBuilder::with_id(TRAY)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Lockstep")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let tray = builder.build(app)?;
    tray.set_visible(false)?;
    Ok(())
}

/// Shows the tray icon while Lockstep works without being asked, with what it follows.
pub fn update_tray(app: &AppHandle, shared: &Shared, now: u64) {
    let Some(tray) = app.tray_by_id(TRAY) else { return };
    let settings = shared.settings();
    let sessions = lock(&shared.sessions);
    let visible = !sessions.is_empty() || settings.watch_always;
    let _ = tray.set_visible(visible);
    if !visible {
        return;
    }
    let name = |id: &str| shared.app(id).map(|a| a.name).unwrap_or_default();
    let mut open = Vec::new();
    let mut closing = Vec::new();
    for (id, live) in sessions.iter() {
        match live.session.phase {
            Phase::Closing { at, .. } => closing.push((name(id), at.saturating_sub(now).div_ceil(1000))),
            _ => open.push(name(id)),
        }
    }
    drop(sessions);
    let _ = tray.set_tooltip(Some(lang::pick(&settings.language).tooltip(&open, &closing)));
}

/// A Windows notification, when the settings allow them.
pub fn notify(app: &AppHandle, shared: &Shared, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    if !shared.settings().notify {
        return;
    }
    let _ = app.notification().builder().title(title).body(body).show();
}
