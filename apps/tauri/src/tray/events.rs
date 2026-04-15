//! Tray menu event handling.

use tauri::{AppHandle, Manager, Runtime, menu::MenuEvent};

pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        "show" => show_main_window(app, None),
        "chat" => show_main_window(app, Some("/agent")),
        "quit" => {
            // Save window state, shut down the gateway sidecar, then exit.
            // State is saved here (primary quit path) because RunEvent::Exit
            // fires only after ExitRequested is not prevented; the run loop's
            // prevent_exit() guard exists to keep the tray app alive on window
            // close, so the Exit event is not guaranteed when exit() is called.
            crate::save_window_state(app);
            tauri::async_runtime::block_on(crate::sidecar::shutdown_agent());
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
