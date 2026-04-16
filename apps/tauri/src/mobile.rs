//! Mobile entry point for NaraeClaw Desktop (iOS/Android).

#[tauri::mobile_entry_point]
fn main() {
    naraeclaw_desktop::run();
}
