//! VibeHub 应用入口：启动 Tauri、本地 HTTP 服务、系统托盘。
mod hooks;
mod server;
mod state;

use state::{AgentStatus, AppState, Decision, DiscoveredSession};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, State,
};

type SharedState = Arc<Mutex<AppState>>;

/// 前端按钮调用：提交 Allow / Deny 决定。
/// decision: "allowed" | "denied"
#[tauri::command(rename_all = "camelCase")]
fn submit_decision(
    decision_id: String,
    decision: String,
    app_state: State<'_, SharedState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let d = match decision.as_str() {
        "allowed" => Decision::Allowed,
        "denied" => Decision::Denied,
        _ => return Err(format!("unknown decision: {decision}")),
    };
    let (updated, _owner_id) = {
        let mut guard = app_state.lock().unwrap_or_else(|e| e.into_inner());
        let owner_id = guard.submit_decision(&decision_id, d);
        if owner_id.is_none() {
            return Err(format!("decision_id not found: {decision_id}"));
        }
        // 将持有该 decision_id 的 agent 从 needs_input 切回 running。
        if let Some(ref aid) = owner_id {
            if let Some(agent) = guard.agents.get_mut(aid) {
                agent.status = AgentStatus::Running;
            }
        }
        // 克隆用于 emit 的 agent 状态。
        let cloned = owner_id.as_ref()
            .and_then(|aid| guard.agents.get(aid).cloned())
            .or_else(|| guard.agents.values().next().cloned());
        (cloned, owner_id)
    };
    if let Some(state) = updated {
        let _ = app.emit("agent-update", &state);
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
#[tauri::command(rename_all = "camelCase")]
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

/// 审批弹窗：将窗口显示并置顶到前台。
#[tauri::command]
fn focus_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        window.set_focus().map_err(|e| e.to_string())
    } else {
        Err("Window not found".to_string())
    }
}

/// 打开设置窗口。若已存在则聚焦。
#[tauri::command]
fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::WebviewUrl;
    use tauri::WebviewWindowBuilder;

    // 如果设置窗口已存在，直接聚焦。
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }

    WebviewWindowBuilder::new(&app, "settings", WebviewUrl::App("settings.html".into()))
        .title("VibeHub 设置")
        .inner_size(680.0, 480.0)
        .min_inner_size(520.0, 380.0)
        .decorations(true)
        .resizable(true)
        .center()
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// 获取 hook 配置状态（供设置窗口使用）。
#[tauri::command]
fn get_hook_status() -> serde_json::Value {
    let (_, hook_path) = hooks::ensure_hooks_configured();
    serde_json::json!({
        "configured": !hook_path.is_empty(),
        "hook_path": hook_path
    })
}

/// 扫描 ~/.claude/projects/*/sessions/*.jsonl，发现最近活跃的会话。
/// 读取文件末尾 4KB 解析最后的 assistant 消息来推断任务。
fn discover_sessions() -> Option<DiscoveredSession> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    let projects_dir = std::path::PathBuf::from(&home).join(".claude").join("projects");
    if !projects_dir.exists() {
        return None;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();
    let five_min_ago = now.saturating_sub(300);

    let mut best: Option<(std::fs::Metadata, std::path::PathBuf, String)> = None;

    // 遍历 project 目录，查找直接在目录下的 *.jsonl 文件。
    for project_entry in std::fs::read_dir(&projects_dir).ok()?.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }
        let project_name = project_entry
            .file_name()
            .to_string_lossy()
            .to_string();

        // 扫描项目目录下的 *.jsonl 文件（Claude Code 直接放在项目根目录）
        for session_entry in std::fs::read_dir(&project_path).ok()?.flatten() {
            let path = session_entry.path();
            if path.extension().map_or(true, |e| e != "jsonl") {
                continue;
            }
            let meta = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            if modified < five_min_ago {
                continue;
            }

            if best.as_ref().map_or(true, |(m, _, _)| {
                m.modified().ok().map_or(true, |t| {
                    t.duration_since(UNIX_EPOCH)
                        .ok()
                        .map_or(false, |d| d.as_secs() < modified)
                })
            }) {
                best = Some((meta, path, project_name.clone()));
            }
        }
    }

    let (_, session_path, project_name) = best?;

    // 提取 session_id（文件名去掉 .jsonl）。
    let session_id = session_path
        .file_stem()?
        .to_string_lossy()
        .to_string();

    // 读取文件末尾 4KB。
    let tail = read_file_tail(&session_path, 4096)?;

    // 解析最后的 assistant 消息。
    let mut last_task = String::new();
    let mut last_timestamp: i64 = 0;

    for line in tail.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
            // 提取时间戳。
            if let Some(ts) = obj.get("timestamp").and_then(|v| v.as_str()) {
                if let Some(dt) = chrono_parse(ts) {
                    if last_timestamp == 0 {
                        last_timestamp = dt;
                    }
                }
            }

            // 只看 assistant 消息提取任务。
            if obj.get("type").and_then(|v| v.as_str()) == Some("assistant") {
                if let Some(message) = obj.get("message") {
                    if let Some(content) = message.get("content").and_then(|v| v.as_array()) {
                        for block in content {
                            if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                                    let summary: String = text.chars().take(80).collect();
                                    if !summary.trim().is_empty() {
                                        last_task = summary;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                if !last_task.is_empty() {
                    break;
                }
            }
        }
    }

    if last_task.is_empty() {
        last_task = format!("Session {}", &session_id[..8.min(session_id.len())]);
    }

    Some(DiscoveredSession {
        session_id,
        project: project_name,
        last_task,
        last_activity: last_timestamp.max(0),
        status: "running".to_string(),
    })
}

/// 读取文件末尾 `size` 字节。
fn read_file_tail(path: &std::path::Path, size: usize) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let file_len = file.metadata().ok()?.len();
    let skip = if file_len > size as u64 {
        file_len - size as u64
    } else {
        0
    };
    file.seek(SeekFrom::Start(skip)).ok()?;
    let mut buf = String::new();
    file.read_to_string(&mut buf).ok()?;
    Some(buf)
}

/// 简易 ISO 8601 时间戳解析（只提取 unix 秒）。
fn chrono_parse(ts: &str) -> Option<i64> {
    // 格式: "2026-07-04T19:22:33.123Z" 或带时区偏移。
    // 简化处理：用 NaiveDateTime 解析前 19 个字符。
    let date_str = &ts[..19.min(ts.len())];
    // "2026-07-04T19:22:33"
    let parts: Vec<&str> = date_str.split('T').collect();
    if parts.len() != 2 {
        return None;
    }
    let date_parts: Vec<i32> = parts[0].split('-').filter_map(|s| s.parse().ok()).collect();
    let time_parts: Vec<i32> = parts[1].split(':').filter_map(|s| s.parse().ok()).collect();
    if date_parts.len() != 3 || time_parts.len() < 2 {
        return None;
    }
    // 简化 unix timestamp 计算（不处理闰秒）。
    let y = date_parts[0];
    let m = date_parts[1];
    let d = date_parts[2];
    let h = time_parts[0];
    let mi = time_parts[1];
    let s = if time_parts.len() > 2 { time_parts[2] } else { 0 };

    // 转为 days since epoch。
    let mut days = 0_i64;
    for yr in 1970..y {
        days += if is_leap(yr) { 366 } else { 365 };
    }
    let month_days = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    days += month_days[(m - 1) as usize] as i64;
    if m > 2 && is_leap(y) {
        days += 1;
    }
    days += (d - 1) as i64;

    Some(days * 86400 + h as i64 * 3600 + mi as i64 * 60 + s as i64)
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// 检测 claude.exe 是否在运行。
/// 用 `tasklist` 过滤进程名，避免引入 winapi 依赖。
fn is_claude_running() -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq claude.exe", "/NH", "/FO", "CSV"])
        .output()
        .map(|out| {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.contains("claude.exe")
        })
        .unwrap_or(true) // tasklist 失败时保守地认为还在运行
}

/// 后台进程存活监控：每 30 秒检测一次 claude.exe。
/// 如果检测到进程消失，将所有非 Idle 会话清零并通知前端。
fn start_process_monitor(handle: tauri::AppHandle, app_state: SharedState) {
    std::thread::spawn(move || {
        let mut was_running = true;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(30));
            let running = is_claude_running();
            if was_running && !running {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                let updated = {
                    let mut guard = app_state.lock().unwrap_or_else(|e| e.into_inner());
                    guard.clear_stale_agents(now)
                };
                for state in updated {
                    let _ = handle.emit("agent-update", &state);
                }
                println!("[VibeHub] claude.exe 已退出，会话状态已清零");
            }
            was_running = running;
        }
    });
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
            get_always_on_top,
            focus_window,
            open_settings,
            get_hook_status
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // 将胶囊居中对齐到主显示器顶部。
            if let Some(window) = app.get_webview_window("main") {
                if let Some(monitor) = window.primary_monitor().ok().flatten() {
                    let screen_w = monitor.size().width as i32;
                    let win_w = 340_i32;
                    let x = (screen_w - win_w) / 2;
                    let _ = window.set_position(tauri::PhysicalPosition::new(x, 8));
                }
            }

            // 启动进程存活监控。
            start_process_monitor(handle.clone(), app_state.clone());

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

            // 自动配置 Claude Code hooks。
            let hooks_handle = handle.clone();
            std::thread::spawn(move || {
                let (newly_configured, hook_path) = hooks::ensure_hooks_configured();
                let status = hooks::HooksStatus {
                    configured: !hook_path.is_empty(),
                    hook_path,
                };
                let _ = hooks_handle.emit("hooks-status", &status);
                if newly_configured {
                    println!("[VibeHub] hooks 已自动配置完成，Claude Code 事件将实时推送");
                }

                // 发现已有的活跃会话。
                if let Some(session) = discover_sessions() {
                    println!(
                        "[VibeHub] 发现活跃会话: {} ({})",
                        session.project, session.session_id
                    );
                    let _ = hooks_handle.emit("session-discovered", &session);
                }
            });

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
