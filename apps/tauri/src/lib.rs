//! NaraeClaw Desktop — Tauri application library.

pub mod commands;
pub mod gateway_client;
pub mod health;
pub mod sidecar;
pub mod state;
pub mod tray;

use gateway_client::GatewayClient;
use state::shared_state;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{Manager, RunEvent};
use tauri_plugin_store::StoreExt;

/// Set to `true` by the tray Quit handler before calling `app.exit(0)`.
///
/// `RunEvent::ExitRequested` fires first on every `exit()` call. For tray apps
/// we prevent that exit (keeping the process alive when windows close). This
/// flag lets the Quit handler signal "this exit is intentional — let it through."
pub static INTENTIONAL_QUIT: AtomicBool = AtomicBool::new(false);

/// Attempt to auto-pair with the gateway so the WebView has a valid token
/// before the React frontend mounts. Runs on localhost so the admin endpoints
/// are accessible without auth.
///
/// Token resolution order:
/// 1. Tauri store (persisted from previous session)
/// 2. In-memory state (current session)
/// 3. Fresh auto-pair via admin endpoint
async fn auto_pair(state: &state::SharedState) -> Option<String> {
    let url = {
        let s = state.read().await;
        s.gateway_url.clone()
    };

    let client = GatewayClient::new(&url, None);

    // Check if gateway requires pairing at all.
    if !client.requires_pairing().await.unwrap_or(false) {
        return None;
    }

    // Check existing token in state.
    {
        let s = state.read().await;
        if let Some(ref token) = s.token {
            let authed = GatewayClient::new(&url, Some(token));
            if authed.validate_token().await.unwrap_or(false) {
                return Some(token.clone());
            }
        }
    }

    // Auto-pair with retries (gateway may still be initializing).
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        let client = GatewayClient::new(&url, None);
        match client.auto_pair().await {
            Ok(token) => {
                let mut s = state.write().await;
                s.token = Some(token.clone());
                return Some(token);
            }
            Err(e) => {
                tracing::debug!("auto-pair attempt {}: {e}", attempt + 1);
            }
        }
    }
    None
}

/// Load a previously saved token from the Tauri store.
fn load_token_from_store<R: tauri::Runtime>(app: &tauri::App<R>) -> Option<String> {
    let store = app.store("naraeclaw.json").ok()?;
    store
        .get("gateway_token")
        .and_then(|v| v.as_str().map(String::from))
}

/// Persist the token to the Tauri store for next launch.
fn save_token_to_store<R: tauri::Runtime>(app: &tauri::AppHandle<R>, token: &str) {
    if let Ok(store) = app.store("naraeclaw.json") {
        store.set("gateway_token", token);
        let _ = store.save();
    }
}

/// Inject a bearer token into the WebView's localStorage and reload so the
/// React app picks it up immediately (skipping the pairing dialog).
fn inject_token_into_webview<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>, token: &str) {
    let escaped = token.replace('\\', "\\\\").replace('\'', "\\'");
    let script = format!(
        "if(!localStorage.getItem('naraeclaw_token')){{localStorage.setItem('naraeclaw_token','{escaped}');location.reload();}}"
    );
    let _ = window.eval(&script);
}

/// Set the macOS dock icon programmatically so it shows even in dev builds
/// (which don't have a proper .app bundle).
#[cfg(target_os = "macos")]
fn set_dock_icon() {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::NSApplication;
    use objc2_app_kit::NSImage;
    use objc2_foundation::NSData;

    let icon_bytes = include_bytes!("../icons/128x128.png");
    // Safety: setup() runs on the main thread in Tauri.
    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let data = NSData::with_bytes(icon_bytes);
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        let app = NSApplication::sharedApplication(mtm);
        unsafe { app.setApplicationIconImage(Some(&image)) };
    }
}

/// Check whether NaraeClaw has been configured (config.toml exists).
fn config_exists() -> bool {
    if let Ok(home) = std::env::var("HOME") {
        std::path::Path::new(&home)
            .join(".naraeclaw")
            .join("config.toml")
            .exists()
    } else {
        false
    }
}

/// Start the gateway sidecar, wait for health, auto-pair, and show the window.
///
/// Called from setup when config exists, and from `complete_onboarding` after
/// the user finishes in-app onboarding.
pub fn start_gateway_and_show(app_handle: tauri::AppHandle, state: state::SharedState) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = sidecar::spawn_agent().await {
            tracing::error!("Failed to start naraeclaw agent: {e}");
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.show();
                let _ = window.eval(r#"document.body.innerHTML='<div style=\"padding:40px;text-align:center;color:#e94560\">에이전트 시작 실패</div>'"#);
            }
            return;
        }

        const MAX_WAIT: u64 = 30;
        const POLL_MS: u64 = 500;
        let steps = (MAX_WAIT * 1000) / POLL_MS;

        let gateway_url = {
            let s = state.read().await;
            s.gateway_url.clone()
        };

        for _ in 0..steps {
            tokio::time::sleep(std::time::Duration::from_millis(POLL_MS)).await;
            if gateway_client::GatewayClient::new(&gateway_url, None)
                .get_health()
                .await
                .unwrap_or(false)
            {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                if let Some(token) = auto_pair(&state).await
                    && let Some(window) = app_handle.get_webview_window("main")
                {
                    save_token_to_store(&app_handle, &token);
                    inject_token_into_webview(&window, &token);
                }
                return;
            }
        }

        tracing::warn!("naraeclaw agent did not become healthy within {MAX_WAIT}s");
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    });
}

/// Minimum and maximum allowed window dimensions.
const WIN_MIN_W: u32 = 600;
const WIN_MIN_H: u32 = 400;
const WIN_MAX_W: u32 = 7680; // 8K width
const WIN_MAX_H: u32 = 4320; // 8K height

/// Restore window size and position from the persisted store.
///
/// Dimensions are clamped to sane bounds so a stale or corrupted store
/// cannot produce a window that is too small, too large, or invisible.
/// Position is only restored when both x and y are non-negative to avoid
/// placing the window off-screen on single-monitor setups.
fn restore_window_state<R: tauri::Runtime>(app: &tauri::App<R>) {
    let Ok(store) = app.store("naraeclaw.json") else {
        return;
    };
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let (Some(w), Some(h)) = (
        store.get("window_width").and_then(|v| v.as_u64()),
        store.get("window_height").and_then(|v| v.as_u64()),
    ) {
        let width = (w as u32).clamp(WIN_MIN_W, WIN_MAX_W);
        let height = (h as u32).clamp(WIN_MIN_H, WIN_MAX_H);
        let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize { width, height }));
    }
    if let (Some(x), Some(y)) = (
        store.get("window_x").and_then(|v| v.as_i64()),
        store.get("window_y").and_then(|v| v.as_i64()),
    ) {
        // Only restore position when both coordinates are non-negative;
        // negative values indicate off-screen positions (e.g. from a
        // disconnected secondary monitor).
        if x >= 0 && y >= 0 {
            let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: x as i32,
                y: y as i32,
            }));
        }
    }
}

/// Save window size and position to the persisted store.
///
/// Called from the tray "Quit" handler (primary path) and from
/// `RunEvent::Exit` as a fallback.
pub fn save_window_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(store) = app.store("naraeclaw.json") else {
        return;
    };
    if let Ok(size) = window.outer_size() {
        store.set("window_width", size.width);
        store.set("window_height", size.height);
    }
    if let Ok(pos) = window.outer_position() {
        store.set("window_x", pos.x);
        store.set("window_y", pos.y);
    }
    let _ = store.save();
}

/// Configure and run the Tauri application.
pub fn run() {
    let shared = shared_state();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // When a second instance launches, focus the existing window.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .manage(shared.clone())
        .invoke_handler(tauri::generate_handler![
            commands::gateway::get_status,
            commands::gateway::get_health,
            commands::channels::list_channels,
            commands::pairing::initiate_pairing,
            commands::pairing::get_devices,
            commands::agent::send_message,
            commands::config::config_exists,
            commands::config::check_ollama,
            commands::config::ollama_pull,
            commands::config::ollama_start,
            commands::config::complete_onboarding,
            commands::config::ollama_health,
            commands::config::ollama_repair_model,
            commands::config::restart_gateway,
            commands::config::get_config,
            commands::config::update_config,
            commands::channel_config::get_channels,
            commands::channel_config::save_channel,
            commands::file_ops::handle_file_drop,
            commands::file_ops::send_clipboard,
            commands::cli_tools::list_cli_tools,
            commands::cli_tools::run_cli_tool,
            commands::scheduler::list_tasks,
            commands::scheduler::create_task_natural,
            commands::scheduler::delete_task,
            commands::computer_use::request_computer_use_approval,
            commands::computer_use::take_screenshot,
            commands::computer_use::mouse_action,
            commands::computer_use::keyboard_type,
            commands::computer_use::keyboard_shortcut,
            commands::computer_use::list_windows,
            commands::resources::get_system_info,
            commands::resources::open_browser,
            commands::resources::open_app,
            commands::resources::list_files,
            commands::resources::list_processes,
            commands::remote::list_servers,
            commands::remote::add_server,
            commands::remote::remove_server,
            commands::remote::switch_server,
            commands::knowledge::memory_to_wiki,
            commands::knowledge::list_memories,
            commands::profiles::list_profiles,
            commands::profiles::create_profile,
            commands::profiles::switch_profile,
            commands::profiles::delete_profile,
            commands::profiles::update_profile_meta,
        ])
        .setup(move |app| {
            // Set macOS dock icon (needed for dev builds without .app bundle).
            #[cfg(target_os = "macos")]
            set_dock_icon();

            // Set up the system tray.
            let _ = tray::setup_tray(app);

            // Restore saved window size and position from previous session.
            restore_window_state(app);

            // Restore token from previous session.
            if let Some(token) = load_token_from_store(app) {
                let state_clone = shared.clone();
                tauri::async_runtime::block_on(async {
                    let mut s = state_clone.write().await;
                    s.token = Some(token);
                });
            }

            // Spawn the naraeclaw agent sidecar. The gateway HTTP server it starts
            // is what the WebView connects to. We show the window only after the
            // gateway is healthy to avoid a blank-page flash.
            let needs_onboarding = !config_exists();
            if needs_onboarding {
                // No config — show onboarding UI immediately.
                // Sidecar is NOT started yet; it will be started after onboarding
                // completes via the `complete_onboarding` Tauri command.
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    // Navigate to onboarding route.
                    let _ = window.eval("window.location.pathname = '/onboarding'");
                }
            } else {
                // Config exists — start gateway sidecar and wait for it.
                start_gateway_and_show(app.handle().clone(), shared.clone());
            }

            // Start background health polling (tray icon / status updates).
            health::spawn_health_poller(app.handle().clone(), shared.clone());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            match event {
                // Keep the app alive when windows are closed (tray app pattern).
                // Exception: the tray Quit handler sets INTENTIONAL_QUIT before
                // calling app.exit(0), so we let that exit through.
                RunEvent::ExitRequested { api, .. } => {
                    if !INTENTIONAL_QUIT.load(Ordering::Acquire) {
                        api.prevent_exit();
                    }
                }
                // Fallback shutdown path — reached when INTENTIONAL_QUIT is true
                // and ExitRequested was not prevented.
                RunEvent::Exit => {
                    save_window_state(app_handle);
                    tauri::async_runtime::block_on(sidecar::shutdown_agent());
                }
                _ => {}
            }
        });
}
