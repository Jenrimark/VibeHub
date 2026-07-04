# VibeHub

面向 Windows 的 **AI Agent 工作中心**——灵感来自 macOS 的 Vibe Island。
它把分散在多个 Terminal / Cursor / Claude Code 里的 AI 工作状态，汇聚到屏幕顶部一个悬浮「灵动岛」胶囊里，让你**一眼看到 AI 在干什么**，并在 AI 完成或需要操作时收到提醒。

技术栈：**Tauri 2（Rust 后端 + 系统 WebView2）**，产物为单个小体积 `VibeHub.exe`。

## MVP 功能

- ✅ **状态监控**：实时显示 Claude Code 当前任务、状态、运行时长
- ✅ **通知提醒**：完成 / 出错 / 需操作时，胶囊高亮变色提醒
- ✅ **一键审批（占位）**：`Allow / Deny` 按钮区（MVP 仅 UI 反馈，不回写）
- ✅ **打包为 exe**

事件来源：Claude Code 官方 **Hooks** 机制——hook 脚本把事件 POST 到 VibeHub 本地服务（`127.0.0.1:51789`），无需截屏或爬终端。

## 目录结构

```
VibeHub/
├─ src/                 前端（原生 HTML/CSS/JS，无框架）
├─ src-tauri/           Rust 后端（state / server / 托盘）
├─ hooks/               Claude Code hook 脚本
├─ scripts/             联调、图标生成、hook 配置辅助
└─ docs/                设计规格
```

## 前置依赖

- **Node.js**（已具备，用于 Tauri CLI）
- **Rust 工具链**——尚未安装，构建前必须装：
  1. 访问 <https://rustup.rs> 或在 PowerShell 运行（需联网，约 10 分钟）：
     ```powershell
     winget install Rustlang.Rustup
     ```
     或下载 `rustup-init.exe` 运行。
  2. 安装后重开终端，确认：`rustc --version`
- **WebView2**：Win11 自带，无需额外安装。

## 开发与运行

```powershell
cd D:\CODE\VibeHub
npm install           # 安装 Tauri CLI
   # 启动开发模式（顶部出现胶囊 + 托盘图标）
```

> 首次 `dev`/`build` 会编译 Rust 依赖，耗时较长属正常。

## 联调（无需真跑 Claude）

VibeHub 运行后，另开一个 PowerShell：

```powershell
cd D:\CODE\VibeHub
powershell -ExecutionPolicy Bypass -File scripts\test-event.ps1
```

会依次模拟 `running → needs_input → completed → idle`，观察顶部胶囊是否正确切换四种状态并计时。

## 接入 Claude Code（真实联调）

1. 生成可粘贴的 hook 配置片段：
   ```powershell
   powershell -ExecutionPolicy Bypass -File scripts\print-hook-config.ps1
   ```
2. 将输出的 `hooks` 字段合并进 `~/.claude/settings.json`。
3. 保持 VibeHub 运行，正常使用 Claude Code，胶囊会实时反映「运行中 → 完成」等状态。

事件映射：

| Claude Hook | VibeHub 状态 |
|---|---|
| SessionStart / UserPromptSubmit / PreToolUse | running |
| Notification | needs_input |
| Stop / SubagentStop | completed |
| SessionEnd | idle |

## 打包 exe

```powershell
npm run tauri build
```

产物：`src-tauri/target/release/VibeHub.exe`，以及 `src-tauri/target/release/bundle/nsis/` 下的安装包。

## 图标

图标由脚本生成（无需设计资源）：

```powershell
node scripts\gen-icons.js
```

## 后续迭代（不在 MVP）

- 审批 / 回答的**真实回写**给 Claude（完整闭环）
- 多 Agent 同时运行的完整 UI（数据结构已支持，接同一 `/event` 端点即可）
- 一键跳回对应 Terminal / Tab
- Progress 细粒度可视化
- 接入 Codex / Gemini / Cursor 等其它 Agent
