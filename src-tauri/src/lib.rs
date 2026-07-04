//! VibeHub 应用入口：启动 Tauri、本地 HTTP 服务、系统托盘。
mod server;
mod state;

use state::{AppState, Decision};
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, State,
};

type SharedState = Arc<Mutex<AppState>>;

/// 前端按钮调用：提交 Allow / Deny 决定。
/// decision: "allowed" | "denied"
#[tauri::command]
fn submit_decision(
    decision_id: String,
    decision: String,
    app_state: State<'_, SharedState>,
) -> Result<(), String> {
    let d = match decision.as_str() {
        "allowed" => Decision::Allowed,
        "denied" => Decision::Denied,
        _ => return Err(format!("unknown decision: {decision}")),
    };
    let mut guard = app_state.lock().unwrap_or_else(|e| e.into_inner());
    if !guard.submit_decision(&decision_id, d) {
        return Err(format!("decision_id not found: {decision_id}"));
    }
    Ok(())
}

/// 调整窗口大小（供设置面板使用）。
/// 使用 LogicalSize 与 tauri.conf.json 的逻辑像素单位保持一致，
/// 避免高 DPI 下 PhysicalSize 导致窗口尺寸漂移。
#[tauri::command]
fn set_window_size(app: tauri::AppHandle, width: f64, height: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }))
            .map_err(|e| e.to_string())
    } else {
        Err("Window not found".to_string())
    }
}

/// 设置窗口置顶。
#[tauri::command]
fn set_always_on_top(app: tauri::AppHandle, always_on_top: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_always_on_top(always_on_top)
            .map_err(|e| e.to_string())
    } else {
        Err("Window not found".to_string())
    }
}

/// 查询窗口是否置顶。
#[tauri::command]
fn get_always_on_top(app: tauri::AppHandle) -> Result<bool, String> {
    if let Some(window) = app.get_webview_window("main") {
        window.is_always_on_top().map_err(|e| e.to_string())
    } else {
        Err("Window not found".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state: SharedState = Arc::new(Mutex::new(AppState::default()));

    tauri::Builder::default()
        .manage(app_state.clone())
        .invoke_handler(tauri::generate_handler![
            submit_decision,
            set_window_size,
            set_always_on_top,
            get_always_on_top
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // 将胶囊居中对齐到主显示器顶部。
            if let Some(window) = app.get_webview_window("main") {
                if let Some(monitor) = window.primary_monitor().ok().flatten() {
                    let screen_w = monitor.size().width as i32;
                    let win_w = 420_i32;
                    let x = (screen_w - win_w) / 2;
                    let _ = window.set_position(tauri::PhysicalPosition::new(x, 8));
                }
            }

            // 启动本地 HTTP 服务接收 hook 事件。
            if let Err(e) = server::start(handle.clone(), app_state.clone()) {
                let msg = format!(
                    "无法在端口 {} 启动服务: {e}。请检查端口是否被占用。",
                    server::PORT
                );
                eprintln!("[VibeHub] {msg}");
                // 通知前端服务启动失败，让用户在 UI 上看到问题。
                let _ = handle.emit("server-error", &msg);
            }

            // 系统托盘：显示/隐藏、退出。
            let show_i = MenuItem::with_id(app, "show", "显示 / 隐藏", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            TrayIconBuilder::new()
                .icon(
                    app.default_window_icon()
                        .expect("default window icon must be configured in tauri.conf.json")
                        .clone(),
                )
                .tooltip("VibeHub")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) {
                                let _ = w.hide();
                            } else {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        // 关闭窗口不退出进程，仅隐藏到托盘。
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("启动 VibeHub 失败");
}
