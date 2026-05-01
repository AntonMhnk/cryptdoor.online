mod commands;
mod core;

use commands::*;
use tauri::{
    image::Image,
    menu::{Menu, MenuEvent, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .invoke_handler(tauri::generate_handler![
            connect_proxy,
            disconnect_proxy,
            connection_status,
            current_external_ip,
            helper_status,
            install_helper,
            tray_set_status,
            window_show,
        ])
        .setup(|app| {
            core::mihomo::init(app.handle().clone())?;
            setup_tray(app.handle())?;

            let launched_at_login = std::env::args().any(|a| a == "--autostart");
            if launched_at_login {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.hide();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Reopen { .. } = event {
                show_main_window(app);
            }
        });
}

pub(crate) const TRAY_ON_BYTES: &[u8] = include_bytes!("../icons/tray-on.png");
pub(crate) const TRAY_OFF_BYTES: &[u8] = include_bytes!("../icons/tray-off.png");

pub(crate) fn tray_image(connected: bool) -> Image<'static> {
    let bytes = if connected { TRAY_ON_BYTES } else { TRAY_OFF_BYTES };
    Image::from_bytes(bytes).expect("valid tray png")
}

fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = build_menu(app, false)?;

    let _tray = TrayIconBuilder::with_id("main")
        .icon(tray_image(false))
        .icon_as_template(false)
        .tooltip("CryptDoor")
        .menu(&menu)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                toggle_main_window(app);
            }
        })
        .build(app)?;

    Ok(())
}

pub(crate) fn build_menu<R: Runtime>(
    app: &AppHandle<R>,
    connected: bool,
) -> tauri::Result<Menu<R>> {
    let toggle_id = if connected { "disconnect" } else { "connect" };
    let toggle_label = if connected { "Disconnect" } else { "Connect" };
    let toggle_accel = if connected { "Cmd+D" } else { "Cmd+K" };

    let toggle_item = MenuItem::with_id(app, toggle_id, toggle_label, true, Some(toggle_accel))?;
    let show_item = MenuItem::with_id(app, "show", "Open CryptDoor", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit CryptDoor", true, Some("Cmd+Q"))?;

    Menu::with_items(app, &[&toggle_item, &show_item, &quit_item])
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        "show" => show_main_window(app),
        "quit" => {
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = disconnect_proxy().await;
                app_clone.exit(0);
            });
        }
        "connect" => {
            let _ = app.emit("tray-action", "connect");
        }
        "disconnect" => {
            let _ = app.emit("tray-action", "disconnect");
        }
        _ => {}
    }
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

fn toggle_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        match win.is_visible() {
            Ok(true) => {
                let _ = win.hide();
            }
            _ => {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }
    }
}
