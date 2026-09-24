//! WeChatBridge Windows 核心库。
//!
//! 模块对应 macOS 版 WeChatBridgeCore 的行为规格移植，详见仓库根 `SPEC.md`。

pub mod archive;
pub mod batch;
pub mod commands;
pub mod obsidian;
pub mod paste_plan;
pub mod scene;
pub mod store;
#[cfg(windows)]
pub mod win32;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

/// 显示并聚焦主窗口。
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// 创建拖拽悬浮球窗口（置顶、无边框、透明、跳过任务栏）。
fn create_float_ball(app: &tauri::AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, "float-ball", WebviewUrl::default())
        .title("")
        .inner_size(72.0, 72.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .resizable(false)
        .skip_taskbar(true)
        .visible(true)
        .build()?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = commands::AppState {
        store: store::Store::new(store::Store::default_root()),
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 单实例：二次启动时聚焦已有窗口。
            show_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None::<Vec<&str>>,
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(|app| {
            // 系统托盘：显示主窗口 / 显示悬浮球 / 退出。
            let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let ball = MenuItem::with_id(app, "ball", "显示悬浮球", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &ball, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("微信流 WeChatBridge")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_main_window(app),
                    "ball" => {
                        if let Some(w) = app.get_webview_window("float-ball") {
                            let _ = w.show();
                        }
                    }
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
                        show_main_window(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            // 拖拽悬浮球：始终置顶，接收从微信/资源管理器拖入的 ZIP。
            create_float_ball(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // 仅主窗口关闭时最小化到托盘；悬浮球直接隐藏。
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    let close_to_tray = window
                        .state::<commands::AppState>()
                        .store
                        .load_settings()
                        .map(|s| s.close_to_tray)
                        .unwrap_or(true);
                    if close_to_tray {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                } else {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::import_zip,
            commands::list_batches,
            commands::delete_batch,
            commands::preview_batch,
            commands::open_batch_folder,
            commands::list_targets,
            commands::forward_batch,
            commands::copy_manual_payload,
            commands::export_obsidian,
            commands::list_scenes,
            commands::save_scenes,
            commands::get_settings,
            commands::save_settings,
            commands::show_float_ball,
            commands::hide_float_ball,
            commands::save_targets,
            commands::pick_file,
            commands::pick_folder,
            commands::list_skills,
            commands::install_skill,
            commands::remove_skill,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
