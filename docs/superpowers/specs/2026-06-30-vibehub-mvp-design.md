# VibeHub MVP 设计文档

> 日期：2026-06-30
> 状态：已批准，待实现

## 1. 背景与目标

VibeHub 是面向 Windows 的「AI Agent 工作中心」，灵感来自 macOS 上的 Vibe Island。它的核心价值**不是美化灵动岛**，而是把原本分散在多个 Terminal / Cursor / Claude Code 里的 AI 工作状态，汇聚到屏幕顶部一个悬浮「灵动岛」胶囊里，让用户**一眼看到 AI 在干什么**，并在 AI 完成或需要操作时收到提醒。

### MVP 目标（只做 4 件事，严格 YAGNI）

1. **状态监控**：实时显示 Claude Code 当前任务、状态、运行时长。
2. **通知提醒**：完成 / 出错 / 需要操作时，胶囊高亮提醒。
3. **一键审批（占位）**：UI 上展示 `Allow / Deny` 等按钮区域；MVP **仅展示，不回写**（回写为后续迭代）。
4. **打包成单个 `VibeHub.exe`**。

### 明确不做（后续迭代）

- 多 Agent 同时运行的完整 UI（架构留接口，数据结构已支持）。
- 审批/回答的**真实回写**给 Claude（MVP 只展示）。
- 一键跳回对应 Terminal / Tab / tmux。
- Progress 细粒度可视化（Reading/Writing/Building 等分步）。
- Codex / Gemini / Cursor 等其它 Agent 接入（接口通用，后续接同一端点）。

## 2. 技术栈

- **框架**：Tauri 2.x（Rust 后端 + 系统 WebView2 前端）。
- **理由**：产物为单个小体积 exe（~5-10MB），Win11 自带 WebView2，符合「exe 最好 + Rust」诉求。
- **前端**：原生 HTML / CSS / JS，**不引入前端框架**（UI 极简，胶囊 + 少量状态）。
- **后端**：Rust。内置一个轻量 HTTP 服务监听 `127.0.0.1`，接收 Claude Code hook 推送的事件。
- **事件源**：Claude Code 官方 **Hooks** 机制（`~/.claude/settings.json`），hook 脚本将事件 POST 到本地服务。**无需截屏 / 爬终端。**

## 3. 架构与数据流

```
Claude Code 触发事件 (SessionStart / Notification / Stop / ...)
        │  (stdin: JSON 事件)
        ▼
  hook 脚本 (PowerShell / curl)
        │  HTTP POST 127.0.0.1:<port>/event  { agent, event_type, task, ... }
        ▼
  Rust HTTP server  ──►  AgentState (单一数据源, 状态机)
        │  Tauri emit("agent-update", state)
        ▼
  WebView UI (胶囊)  ──►  渲染状态 / 计时 / 高亮提醒
```

### 端口约定

固定监听 `127.0.0.1:51789`（高位端口，避开常见占用）。若被占用，启动时报错并提示。

## 4. 组件拆分（职责隔离）

| 单元 | 职责 | 接口 |
|---|---|---|
| **hook 脚本** (`hooks/vibehub-hook.ps1`) | 读取 Claude 事件 JSON，转换为 VibeHub 事件并 POST | stdin(JSON) → HTTP POST |
| **server** (`src-tauri/src/server.rs`) | 监听 `/event`，解析请求，更新 state，触发广播 | HTTP `POST /event` + Tauri emit |
| **state** (`src-tauri/src/state.rs`) | 单一数据源；`AgentState` 状态机；多 agent 用 map 管理 | 纯结构体 + 方法 |
| **app** (`src-tauri/src/main.rs`) | Tauri 启动、窗口配置、系统托盘、emit 桥接 | Tauri setup |
| **UI** (`src/index.html` + `app.js` + `style.css`) | 渲染胶囊、监听 `agent-update`、本地计时、状态切换 | `listen("agent-update")` |

### 数据结构（state）

```rust
enum AgentStatus { Idle, Running, NeedsInput, Completed, Error }

struct AgentState {
    agent_id: String,     // "claude" — 多 agent 扩展位
    agent_name: String,   // "Claude"
    task: String,         // 当前任务摘要
    status: AgentStatus,
    started_at: Option<i64>, // unix 秒，用于计时
    message: String,         // 通知文案，如 "Tests Passed"
}
```

`AppState` 持有 `HashMap<String, AgentState>`，MVP 实际只有 `"claude"` 一条。

### 事件类型映射（Claude Hook → VibeHub）

| Claude Hook | VibeHub event_type | 效果 |
|---|---|---|
| `SessionStart` / `UserPromptSubmit` | `running` | 胶囊进入 Running，开始计时 |
| `Notification`（需权限/提问） | `needs_input` | 胶囊高亮，显示 `⚠ Needs you` + 按钮区 |
| `Stop`（回合结束） | `completed` | 胶囊显示 `✔ Done`，停止计时 |
| `SessionEnd` | `idle` | 胶囊缩回空闲药丸 |
| 自定义错误 | `error` | 胶囊红色 `⚠ Error` |

## 5. UI 形态与状态

- **位置**：屏幕顶部居中。无边框、`always-on-top`、透明背景、圆角胶囊、可拖动。
- **系统托盘**：托盘图标，右键菜单「显示/隐藏」「退出」；关闭窗口不退出进程。

三种视觉状态：

1. **空闲**：小药丸 `● VibeHub · idle`。
2. **运行**：`Claude · <task> · Running m:ss`（计时由前端基于 `started_at` 本地累加，每秒刷新）。
3. **提醒**：胶囊换色（绿色=完成 / 黄色=需操作 / 红色=错误）+ 文案；点击可展开详情面板。`needs_input` 时展示 `Allow / Deny` 按钮区（MVP 点击仅 UI 反馈，不回写）。

## 6. 错误处理

- **端口占用**：启动失败时托盘气泡/日志提示，进程退出码非零。
- **hook POST 失败**：hook 脚本静默失败（`-ErrorAction SilentlyContinue`），绝不阻塞 Claude 本身运行。
- **非法请求体**：server 返回 400，不更新 state，不崩溃。
- **未知 agent_id**：自动创建一条新 AgentState（向前兼容多 agent）。

## 7. 测试策略

- **Rust 单元测试**：`state.rs` 的状态机转换（idle→running→needs_input→completed→idle），事件解析。
- **手动联调脚本**：`scripts/test-event.ps1`，向 `/event` POST 模拟事件，**不依赖真跑 Claude** 即可验证 UI 全部状态切换。
- **真实联调**：在 `~/.claude/settings.json` 注册 hook 后实跑一次 Claude Code，确认状态实时反映。

## 8. 构建与产物

- 开发：`npm run tauri dev`（或 `cargo tauri dev`）。
- 打包：`npm run tauri build` → 产出 `src-tauri/target/release/VibeHub.exe` 及安装包。
- 前置依赖：Rust 工具链（`rustup`，一次性安装）；Node 已具备（v24）。

## 9. 目录结构

```
VibeHub/
├─ docs/superpowers/specs/          # 本设计文档
├─ src/                             # 前端
│  ├─ index.html
│  ├─ app.js
│  └─ style.css
├─ src-tauri/                       # Rust 后端
│  ├─ src/
│  │  ├─ main.rs
│  │  ├─ server.rs
│  │  └─ state.rs
│  ├─ Cargo.toml
│  ├─ tauri.conf.json
│  └─ icons/
├─ hooks/
│  └─ vibehub-hook.ps1              # Claude Code hook 脚本
├─ scripts/
│  └─ test-event.ps1                # 手动联调
└─ package.json
```

## 10. 验收标准

1. 运行 `test-event.ps1` 依次 POST `running / needs_input / completed / idle`，顶部胶囊实时正确切换四种视觉状态并计时。
2. 在 `settings.json` 注册 hook 后实跑 Claude Code，胶囊能反映「运行中 → 完成」。
3. `tauri build` 成功产出可双击运行的 `VibeHub.exe`，启动后顶部出现胶囊、托盘有图标、可退出。
