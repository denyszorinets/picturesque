//! Tray icon: shows what the app is working on and keeps long jobs running
//! when the window is closed. The rules for "what is running" live in
//! `picturesque_app::work`; this file only draws them.

use std::sync::Arc;

use picturesque_app::ports::events::AppEvent;
use picturesque_app::work::WorkTracker;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Window, WindowEvent};

const MAIN: &str = "main";
const IDLE: &str = "Picturesque";

/// The tray icon and the work it reports. Absent when the system has no
/// tray, in which case closing the window always quits.
pub struct Tray {
    work: Arc<WorkTracker>,
    status: MenuItem<tauri::Wry>,
    icon: tauri::tray::TrayIcon,
}

impl Tray {
    pub fn create(app: &AppHandle, work: Arc<WorkTracker>) -> tauri::Result<Self> {
        let status = MenuItem::with_id(app, "status", IDLE, false, None::<&str>)?;
        let show = MenuItem::with_id(app, "show", "Show Picturesque", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
        let menu = Menu::with_items(
            app,
            &[&status, &PredefinedMenuItem::separator(app)?, &show, &quit],
        )?;
        let mut builder = TrayIconBuilder::with_id("main")
            .tooltip(IDLE)
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(|app, event| match event.id.as_ref() {
                "show" => show_window(app),
                "quit" => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(|icon, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    show_window(icon.app_handle());
                }
            });
        if let Some(image) = app.default_window_icon() {
            builder = builder.icon(image.clone());
        }
        Ok(Self {
            work,
            status,
            icon: builder.build(app)?,
        })
    }

    /// Updates the status line after an event.
    pub fn observe(&self, event: &AppEvent) {
        self.work.observe(event);
        let line = self.work.summary();
        let text = line.as_deref().unwrap_or(IDLE);
        let _ = self.status.set_text(text);
        let _ = self.icon.set_tooltip(Some(text));
    }
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Closing the window while a job runs hides it instead, so the job
/// continues and the tray icon can bring the window back.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    let app = window.app_handle();
    if let Some(tray) = app.try_state::<Arc<Tray>>()
        && tray.work.busy()
    {
        api.prevent_close();
        let _ = window.hide();
    }
}
