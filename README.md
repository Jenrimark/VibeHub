<div align="center">

# VibeHub
**AI Agent 工作中心 — Windows 灵动胶囊**

把分散在多个 Terminal / Cursor / Claude Code 里的 AI 工作状态，汇聚到屏幕顶部一个悬浮胶囊里。<br/>
一眼看到 AI 在干什么，完成或需要操作时立刻收到提醒。

[![Tauri](https://img.shields.io/badge/Tauri_2-Rust-blue?logo=tauri)](https://tauri.app)
[![Windows](https://img.shields.io/badge/Platform-Windows%2011-0078d4?logo=windows)](https://www.microsoft.com/windows)
[![Version](https://img.shields.io/badge/Version-0.2.0-brightgreen)]()
[![License](https://img.shields.io/badge/License-MIT-yellow)](LICENSE)

</div>

---

## ✨ 特性

- **实时状态胶囊** — 悬浮在屏幕顶部，自动显示当前 AI Agent 的任务、状态和运行时长
- **智能通知** — 完成 / 出错 / 需要操作时，胶囊高亮变色 + 系统通知
- **一键审批** — `Allow / Deny` 按钮区，直接在胶囊内完成操作确认
- **零侵入** — 通过 Claude Code 官方 Hooks 机制获取事件，无需截屏或爬终端
- **极致轻量** — Tauri 2 + WebView2，单个 `VibeHub.exe`，体积极小

## 📸 截图

> 欢迎提交 PR 补充截图！

## 🚀 快速开始

### 前置依赖


| 依赖                            | 说明              |
| ----------------------------- | --------------- |
| [Node.js](https://nodejs.org) | 用于 Tauri CLI    |
| [Rust 工具链](https://rustup.rs) | 构建后端            |
| WebView2                      | Win11 自带，无需额外安装 |


**Rust 安装详情**

```powershell
winget install Rustlang.Rustup
# 或访问 https://rustup.rs 下载 rustup-init.exe
# 安装后重开终端确认：
rustc --version
```

### 安装 & 运行

```bash
git clone https://github.com/Jenrimark/VibeHub.git
cd VibeHub
npm install        # 安装依赖
npm run dev        # 启动开发模式
```

> 首次 `dev` / `build` 会编译 Rust 依赖，耗时较长属正常。

顶部出现胶囊 + 托盘图标即表示启动成功。

## 🔌 接入 Claude Code

```powershell
# 1. 生成 hook 配置
powershell -ExecutionPolicy Bypass -File scripts\print-hook-config.ps1

# 2. 将输出的 hooks 字段合并进 ~/.claude/settings.json

# 3. 保持 VibeHub 运行，正常使用 Claude Code 即可
```

### 事件映射


| Claude Hook                                        | VibeHub 状态     |
| -------------------------------------------------- | -------------- |
| `SessionStart` / `UserPromptSubmit` / `PreToolUse` | 🟢 running     |
| `Notification`                                     | 🟡 needs_input |
| `Stop` / `SubagentStop`                            | 🔵 completed   |
| `SessionEnd`                                       | ⚪ idle         |


## 🧪 联调测试

无需真实 Claude 环境，使用模拟事件测试：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\test-event.ps1
```

会依次模拟 `running → needs_input → completed → idle`，观察胶囊状态切换。

## 📦 打包

```powershell
npm run tauri build
```

产物位置：

- `src-tauri/target/release/VibeHub.exe`
- `src-tauri/target/release/bundle/nsis/` — NSIS 安装包

## 📁 项目结构

```
VibeHub/
├── src/                 # 前端（原生 HTML/CSS/JS）
├── src-tauri/           # Rust 后端（状态管理 / HTTP 服务 / 系统托盘）
├── hooks/               # Claude Code hook 脚本
├── scripts/             # 联调 & 工具脚本
└── docs/                # 设计文档
```

## 🗺️ 路线图

- [x] 实时状态监控
- [x] 通知提醒（高亮变色）
- [x] 一键审批 UI
- [x] 打包为 exe
- [ ] 审批 / 回答的真实回写（完整闭环）
- [ ] 多 Agent 并行 UI
- [ ] 一键跳回对应 Terminal / Tab
- [ ] Progress 细粒度可视化
- [ ] 接入 Codex / Gemini / Cursor 等其它 Agent

## 🤝 贡献

欢迎 Issue 和 PR！

1. Fork 本仓库
2. 创建分支：`git checkout -b feature/your-feature`
3. 提交更改：`git commit -m 'feat: add something'`
4. 推送分支：`git push origin feature/your-feature`
5. 提交 Pull Request

## 📄 许可证

[MIT](LICENSE) © VibeHub

---

## ⭐ Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Jenrimark/VibeHub&type=Date)](https://star-history.com/#Jenrimark/VibeHub&Date)

## 💖 支持

如果 VibeHub 对你有帮助，欢迎：

- ⭐ 给项目点个 **Star**，让更多人看到
- 🐛 提交 [Issue](https://github.com/Jenrimark/VibeHub/issues) 反馈问题或建议
- 🍴 Fork 后打造你自己的版本
- 📣 分享给朋友 / 社交媒体

---

Jenrimark ❤️ 2026