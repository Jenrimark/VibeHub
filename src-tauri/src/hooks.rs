//! 启动时自动配置 Claude Code hooks，让 VibeHub 接收事件。
use serde_json::Value as JsonValue;
use std::path::PathBuf;

/// hooks-status 事件载荷，emit 给前端。
#[derive(serde::Serialize, Clone)]
pub struct HooksStatus {
    pub configured: bool,
    pub hook_path: String,
}

/// 推导 hook 脚本的绝对路径。
/// 1. 先看 exe 同级目录下的 hooks/vibehub-hook.ps1（打包模式）
/// 2. 再从 exe 位置向上逐级查找（开发模式：exe 在 src-tauri/target/debug/ 下）
fn find_hook_script() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();

    // 打包模式：exe 旁直接有 hooks/
    let bundled = exe_dir.join("hooks").join("vibehub-hook.ps1");
    if bundled.exists() {
        return Some(bundled);
    }

    // 开发模式：从 exe 目录向上找包含 hooks/ 的项目根
    let mut dir = exe_dir.clone();
    for _ in 0..10 {
        let candidate = dir.join("hooks").join("vibehub-hook.ps1");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }

    None
}

/// 获取 Claude Code settings.json 路径：~/.claude/settings.json
fn settings_path() -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    Some(PathBuf::from(home).join(".claude").join("settings.json"))
}

/// 检查 settings.json 中是否已配置 VibeHub hook。
fn has_vibehub_hook(settings: &JsonValue) -> bool {
    let hooks = match settings.get("hooks") {
        Some(h) => h,
        None => return false,
    };
    let json_str = hooks.to_string();
    json_str.contains("vibehub-hook")
}

/// 生成 hook command 字符串。
fn hook_command(hook_path: &str) -> String {
    format!(
        "powershell -NoProfile -ExecutionPolicy Bypass -File \"{}\"",
        hook_path
    )
}

/// 为所有 Claude Code 生命周期事件创建 hook 条目。
fn make_hook_entry(command: &str) -> JsonValue {
    serde_json::json!([{
        "hooks": [{
            "type": "command",
            "command": command
        }]
    }])
}

/// 核心逻辑：读取 → 检测 → 合并 → 写回。
/// 返回 (是否新配置, hook 路径)。
pub fn ensure_hooks_configured() -> (bool, String) {
    let hook_path = match find_hook_script() {
        Some(p) => p,
        None => {
            eprintln!("[VibeHub] 未找到 hooks/vibehub-hook.ps1，跳过 hook 自动配置");
            return (false, String::new());
        }
    };

    let hook_path_str = hook_path.to_string_lossy().to_string();
    let settings_path = match settings_path() {
        Some(p) => p,
        None => {
            eprintln!("[VibeHub] 无法确定 ~/.claude/ 路径");
            return (false, hook_path_str);
        }
    };

    // 确保 ~/.claude/ 目录存在。
    if let Some(parent) = settings_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // 读取现有 settings.json（不存在则为空对象）。
    let mut settings: JsonValue = match std::fs::read_to_string(&settings_path) {
        Ok(content) => {
            // 处理 UTF-8 BOM（Windows 记事本可能添加）。
            let clean = content.trim_start_matches('\u{FEFF}');
            serde_json::from_str(clean).unwrap_or(JsonValue::Object(serde_json::Map::new()))
        }
        Err(_) => JsonValue::Object(serde_json::Map::new()),
    };

    // 已配置则跳过。
    if has_vibehub_hook(&settings) {
        println!("[VibeHub] hooks 已配置，跳过");
        return (false, hook_path_str);
    }

    // 备份现有文件。
    if settings_path.exists() {
        let backup = settings_path.with_extension("json.vibehub-backup");
        let _ = std::fs::copy(&settings_path, &backup);
        println!("[VibeHub] 已备份 settings.json -> {}", backup.display());
    }

    // 合并 hooks。
    let cmd = hook_command(&hook_path_str);
    let hook_entry = make_hook_entry(&cmd);

    let events = [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "Notification",
        "Stop",
        "SessionEnd",
    ];

    if settings.get("hooks").is_none() {
        settings["hooks"] = serde_json::json!({});
    }
    for event in &events {
        settings["hooks"][event] = hook_entry.clone();
    }

    // 写回。
    match serde_json::to_string_pretty(&settings) {
        Ok(json) => match std::fs::write(&settings_path, json) {
            Ok(_) => {
                println!(
                    "[VibeHub] 已自动配置 hooks -> {}",
                    settings_path.display()
                );
                (true, hook_path_str)
            }
            Err(e) => {
                eprintln!("[VibeHub] 写入 settings.json 失败: {e}");
                (false, hook_path_str)
            }
        },
        Err(e) => {
            eprintln!("[VibeHub] JSON 序列化失败: {e}");
            (false, hook_path_str)
        }
    }
}
