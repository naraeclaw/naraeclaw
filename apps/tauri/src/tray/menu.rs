//! Tray menu construction.

use tauri::{
    App, Runtime,
    menu::{Menu, MenuItemBuilder, PredefinedMenuItem},
};

pub fn create_tray_menu<R: Runtime>(app: &App<R>) -> Result<Menu<R>, tauri::Error> {
    let show = MenuItemBuilder::with_id("show", "대시보드 열기").build(app)?;
    let chat = MenuItemBuilder::with_id("chat", "에이전트 채팅").build(app)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let status = MenuItemBuilder::with_id("status", "상태: 확인 중...")
        .enabled(false)
        .build(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "NaraeClaw 종료").build(app)?;

    Menu::with_items(app, &[&show, &chat, &sep1, &status, &sep2, &quit])
}
