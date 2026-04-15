//! Tray menu event handling.

use tauri::{AppHandle, Manager, Runtime, menu::MenuEvent};

pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        "show" => show_main_window(app, None),
        "chat" => show_main_window(app, Some("/agent")),
        "quit" => {
            // Save window state and shut down the sidecar before exiting.
            // INTENTIONAL_QUIT must be set before app.exit(0) so that the
            // run loop lets RunEvent::ExitRequested through (instead of
            // preventing it), allowing the process to actually terminate.
            crate::save_window_state(app);
            tauri::async_runtime::block_on(crate::sidecar::shutdown_agent());
            crate::INTENTIONAL_QUIT.store(true, std::sync::atomic::Ordering::Release);
            app.exit(0);
        }
        _ => {}
    }
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>, navigate_to: Option<&str>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        if let Some(path) = navigate_to {
            let script = format!("window.location.hash = '{path}'");
            let _ = window.eval(&script);
        }
    }
}
