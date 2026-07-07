# VibeHub 审批反馈与诊断系统 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为 VibeHub 的审批功能添加视觉反馈动画和诊断日志，解决"按钮点击后无反应"的用户体验问题。

**架构：** 纯前端改动，在 `handleDecision()` 成功后用确认消息替换按钮（1.5 秒淡出），在 auto-approve 触发时显示快速提示（0.8 秒），同时在日志面板中记录每个审批事件的状态变化。新增一个 PowerShell 端到端测试脚本验证完整流程。

**技术栈：** 原生 JavaScript（无框架）、CSS 动画、PowerShell

**规格文件：** `docs/superpowers/specs/2026-07-07-approval-feedback-design.md`

---

## 文件结构

| 文件 | 职责 | 变更类型 |
|------|------|----------|
| `src/style.css` | 胶囊、按钮、日志面板样式 | 修改 — 新增 `.decision-feedback` 系列样式 |
| `src/app.js` | 前端全部逻辑（578 行） | 修改 — 新增反馈函数、修改 handleDecision、修改 auto-approve、新增诊断日志 |
| `src/index.html` | HTML 结构 | 修改 — 右键菜单新增"事件日志"选项 |
| `scripts/test-approval.ps1` | 端到端审批测试 | 新增 |

---

## 任务 1：CSS — 添加审批反馈样式

**文件：**
- 修改：`src/style.css:448`（在 `.auto-approve input` 之后、`/* ============== 实时日志面板 ============== */` 之前插入）

- [ ] **步骤 1：在 style.css 中插入审批反馈样式**

在 `src/style.css` 第 448 行（`.auto-approve input` 块的结束 `}` 之后）和第 450 行（`/* ============== 实时日志面板 ============== */` 注释之前）之间，插入以下 CSS：

```css
/* ============== 审批确认反馈 ============== */
.decision-feedback {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 4px 12px;
  font-size: 11px;
  font-weight: 600;
  border-radius: 8px;
  animation: feedbackIn 0.2s ease;
  opacity: 1;
  transition: opacity 0.3s ease;
  white-space: nowrap;
}

.decision-feedback.fade-out {
  opacity: 0;
}

.decision-feedback.approved {
  color: #04210d;
  background: var(--done);
}

.decision-feedback.denied {
  color: #fff;
  background: var(--error);
}

.decision-feedback.auto {
  color: #fff;
  background: var(--running);
}

@keyframes feedbackIn {
  from { opacity: 0; transform: scale(0.9); }
  to   { opacity: 1; transform: scale(1); }
}
```

- [ ] **步骤 2：验证 CSS 语法正确**

运行：在浏览器 DevTools 或 VS Code 中确认无语法错误。

- [ ] **步骤 3：Commit**

```bash
git add src/style.css
git commit -m "style: 添加审批确认反馈 CSS 样式"
```

---

## 任务 2：JS — 添加 `showDecisionFeedback()` 辅助函数

**文件：**
- 修改：`src/app.js:251`（在 `renderButtons` 函数结束 `}` 之后、`handleDecision` 函数之前插入）

- [ ] **步骤 1：在 app.js 中插入 showDecisionFeedback 函数**

在 `src/app.js` 第 251 行（`renderButtons` 函数的结束 `}`）之后、第 253 行（`/// 提交审批决定。` 注释）之前，插入以下函数：

```javascript
/// 在按钮区域显示审批确认消息，持续指定时间后淡出。
/// type: "approved" | "denied" | "auto"
function showDecisionFeedback(type, duration) {
  const labels = {
    approved: "✓ 已批准",
    denied: "✗ 已拒绝",
    auto: "⚡ 自动批准",
  };
  const feedback = document.createElement("div");
  feedback.className = `decision-feedback ${type}`;
  feedback.textContent = labels[type] || type;
  els.actions.innerHTML = "";
  els.actions.appendChild(feedback);

  // 强制 actions 区可见（此时 mainView 可能仍是 needs_input）
  els.actions.style.display = "flex";

  setTimeout(() => {
    feedback.classList.add("fade-out");
    setTimeout(() => {
      // 淡出完成后清空，render() 会自然接管
      if (els.actions.contains(feedback)) {
        els.actions.innerHTML = "";
        els.actions.style.display = "";
      }
    }, 300);
  }, duration);
}
```

- [ ] **步骤 2：Commit**

```bash
git add src/app.js
git commit -m "feat: 添加 showDecisionFeedback 确认消息函数"
```

---

## 任务 3：JS — 修改 `handleDecision()` 集成视觉反馈

**文件：**
- 修改：`src/app.js:253-282`（`handleDecision` 函数）

- [ ] **步骤 1：修改 handleDecision 在成功时显示反馈**

将 `src/app.js` 第 253-282 行的 `handleDecision` 函数替换为：

```javascript
/// 提交审批决定。
async function handleDecision(decision, clickedBtn) {
  // 禁用所有按钮，标记点击的按钮
  const allBtns = els.actions.querySelectorAll(".btn");
  allBtns.forEach((b) => {
    b.disabled = true;
    if (b === clickedBtn) {
      b.classList.add("acked");
    } else {
      b.classList.add("denied");
    }
  });

  const decId = current.decisionId;
  if (decId) {
    try {
      await window.__TAURI__.core.invoke("submit_decision", {
        decisionId: decId,
        decision,
      });
      // 成功：显示确认反馈
      showDecisionFeedback(decision === "allowed" ? "approved" : "denied", 1500);
      appendDecisionLog("decision", `${decision === "allowed" ? "Allowed" : "Denied"} ${decId.slice(0, 8)}`);
    } catch (e) {
      console.error("[VibeHub] submit_decision failed:", e);
      appendDecisionLog("error", `submit_decision failed: ${e}`);
      // 恢复按钮
      allBtns.forEach((b) => {
        b.classList.remove("acked", "denied");
        b.disabled = false;
      });
    }
  }
}
```

**关键变更：**
- 保存 `current.decisionId` 到局部变量 `decId`，避免异步期间被覆盖
- 成功后调用 `showDecisionFeedback()` 替代直接消失
- 成功/失败都调用 `appendDecisionLog()`（下一步实现）

- [ ] **步骤 2：Commit**

```bash
git add src/app.js
git commit -m "feat: handleDecision 成功后显示视觉确认反馈"
```

---

## 任务 4：JS — 修改自动批准逻辑集成视觉反馈

**文件：**
- 修改：`src/app.js:395-404`（`agent-update` 监听器中的 auto-approve 逻辑）

- [ ] **步骤 1：修改 auto-approve 代码块**

将 `src/app.js` 第 398-404 行的自动批准代码：

```javascript
      // 自动批准：needs_input + 有 decision_id + autoApprove 开启
      if (payload.status === "needs_input" && payload.decision_id && autoApprove) {
        window.__TAURI__.core.invoke("submit_decision", {
          decisionId: payload.decision_id,
          decision: "allowed",
        }).catch((e) => console.error("[VibeHub] auto-approve failed:", e));
      }
```

替换为：

```javascript
      // 自动批准：needs_input + 有 decision_id + autoApprove 开启
      if (payload.status === "needs_input" && payload.decision_id && autoApprove) {
        const autoDecId = payload.decision_id;
        window.__TAURI__.core.invoke("submit_decision", {
          decisionId: autoDecId,
          decision: "allowed",
        }).then(() => {
          showDecisionFeedback("auto", 800);
          appendDecisionLog("auto", `Auto-approved ${autoDecId.slice(0, 8)}`);
        }).catch((e) => {
          console.error("[VibeHub] auto-approve failed:", e);
          appendDecisionLog("error", `auto-approve failed: ${e}`);
        });
      }
```

**关键变更：**
- 保存 `decision_id` 到局部变量
- `.catch` 改为 `.then` + `.catch`，成功时显示 "⚡ 自动批准" 反馈（0.8 秒）
- 成功/失败都记录诊断日志

- [ ] **步骤 2：Commit**

```bash
git add src/app.js
git commit -m "feat: 自动批准触发时显示 ⚡ 自动批准 反馈"
```

---

## 任务 5：JS — 添加 `appendDecisionLog()` 诊断日志函数

**文件：**
- 修改：`src/app.js:107`（在 `appendLog` 函数结束后插入）

- [ ] **步骤 1：在 appendLog 函数之后插入 appendDecisionLog**

在 `src/app.js` 第 107 行（`appendLog` 函数的结束 `}` ）之后插入：

```javascript
/// 追加诊断日志条目（审批决定、自动批准、错误、状态变化）。
function appendDecisionLog(type, message) {
  if (!isDev) return; // 仅开发模式记录

  const typeConfig = {
    decision: { icon: "✅", label: "DECISION", cls: "done" },
    auto:     { icon: "⚡", label: "AUTO",     cls: "running" },
    error:    { icon: "❌", label: "ERROR",    cls: "error" },
    state:    { icon: "🔄", label: "STATE",    cls: "info" },
  };
  const cfg = typeConfig[type] || typeConfig.state;
  const entry = {
    time: fmtLogTime(),
    hook: `${cfg.icon} ${cfg.label}`,
    status: cfg.cls,
    detail: message,
  };
  logEntries.push(entry);
  if (logEntries.length > MAX_LOG_LINES) logEntries.shift();

  if (!logVisible) return;

  const line = document.createElement("div");
  line.className = "log-line";
  line.innerHTML =
    `<span class="log-time">${entry.time}</span>` +
    `<span class="log-event ${entry.status}">${entry.hook}</span>` +
    `<span class="log-detail">${escapeHtml(entry.detail)}</span>`;
  els.logBody.appendChild(line);

  while (els.logBody.children.length > MAX_LOG_LINES) {
    els.logBody.removeChild(els.logBody.firstChild);
  }
  els.logBody.scrollTop = els.logBody.scrollHeight;
}
```

- [ ] **步骤 2：在 render() 中添加状态变化日志**

在 `src/app.js` 的 `render()` 函数中（第 294 行 `current.status = state.status;` 之后），添加状态变化检测：

找到这段代码（约第 293-295 行）：
```javascript
  current.status = state.status;
  current.startedAt = state.started_at;
```

在它之后插入：
```javascript
  // 诊断日志：记录状态变化
  if (state.status !== previousStatus) {
    appendDecisionLog("state", `${previousStatus} → ${state.status}`);
  }
```

同时在 `render()` 函数开头（第 284 行 `function render(state) {` 之后）添加：
```javascript
  const previousStatus = current.status;
```

- [ ] **步骤 3：Commit**

```bash
git add src/app.js
git commit -m "feat: 添加 appendDecisionLog 诊断日志函数和状态变化追踪"
```

---

## 任务 6：HTML + JS — 右键菜单新增"事件日志"选项

**文件：**
- 修改：`src/index.html:42-44`（右键菜单区域）
- 修改：`src/app.js:506-512`（`handleMenuAction` 函数）

- [ ] **步骤 1：在 index.html 右键菜单中添加日志选项**

将 `src/index.html` 第 42-44 行：

```html
    <div id="contextMenu" class="context-menu hidden" role="menu">
      <button class="ctx-item" data-action="settings" role="menuitem">⚙️ 设置</button>
    </div>
```

替换为：

```html
    <div id="contextMenu" class="context-menu hidden" role="menu">
      <button class="ctx-item" data-action="log" role="menuitem">📋 事件日志</button>
      <button class="ctx-item" data-action="settings" role="menuitem">⚙️ 设置</button>
    </div>
```

- [ ] **步骤 2：在 handleMenuAction 中添加 log 处理**

将 `src/app.js` 第 506-512 行：

```javascript
function handleMenuAction(action) {
  hideContextMenu();

  if (action === "settings") {
    openSettings();
  }
}
```

替换为：

```javascript
function handleMenuAction(action) {
  hideContextMenu();

  if (action === "settings") {
    openSettings();
  } else if (action === "log") {
    toggleLog();
  }
}
```

- [ ] **步骤 3：Commit**

```bash
git add src/index.html src/app.js
git commit -m "feat: 右键菜单新增事件日志切换选项"
```

---

## 任务 7：PowerShell — 端到端审批测试脚本

**文件：**
- 新增：`scripts/test-approval.ps1`

- [ ] **步骤 1：创建 test-approval.ps1**

创建 `scripts/test-approval.ps1`，内容如下：

```powershell
# VibeHub 审批流程端到端测试
# 验证指标：M1（事件接收率）、M2（Decision 注册）、M5（自动批准延迟）、M7（状态转换）
# 用法：先启动 VibeHub（npm run tauri dev），再运行本脚本。

$url = "http://127.0.0.1:51789"
$pass = 0
$fail = 0
$skip = 0

function Send-Event($payload) {
    $json = $payload | ConvertTo-Json -Compress
    try {
        $resp = Invoke-RestMethod -Uri "$url/event" -Method Post -Body $json -ContentType "application/json; charset=utf-8" -TimeoutSec 2
        return $true
    } catch {
        return $false
    }
}

function Check-Decision($id) {
    try {
        $resp = Invoke-RestMethod -Uri "$url/decision/$id" -Method Get -TimeoutSec 2
        return $resp.decision
    } catch {
        return $null
    }
}

function Assert($condition, $passMsg, $failMsg) {
    if ($condition) {
        Write-Host "  ✅ PASS: $passMsg" -ForegroundColor Green
        $script:pass++
    } else {
        Write-Host "  ❌ FAIL: $failMsg" -ForegroundColor Red
        $script:fail++
    }
}

Write-Host "=== VibeHub 审批流程测试 ===" -ForegroundColor Cyan
Write-Host ""

# --- Step 1: 基础连通性 ---
Write-Host "[Step 1] 基础连通性..." -ForegroundColor Yellow
$ok = Send-Event @{ agent_id="claude"; agent_name="Claude"; event_type="running"; task="Connectivity test" }
Assert $ok "POST /event 返回 ok" "无法连接 VibeHub — 是否在运行？"

if (-not $ok) {
    Write-Host "`n❌ VibeHub 未运行，终止测试。" -ForegroundColor Red
    exit 1
}

# --- Step 2: Decision 注册 ---
Write-Host "`n[Step 2] Decision 注册..." -ForegroundColor Yellow
$decId = "test_$(Get-Random)"
$ok = Send-Event @{
    agent_id="claude"; agent_name="Claude"; event_type="needs_input"
    task="Approval test"; message="Allow Write -> test.txt?"
    decision_id=$decId; hook_event_name="PreToolUse"; tool_name="Write"; tool_preview="test.txt"
}
Assert $ok "POST needs_input + decision_id 返回 ok" "POST needs_input 失败"

# 等待一小段时间让 auto-approve（如果开启）生效
Start-Sleep -Milliseconds 200
$result = Check-Decision $decId
# 可能是 pending（未开 auto-approve）或 allowed（已开 auto-approve）
Assert (($result -eq "pending") -or ($result -eq "allowed")) "GET /decision/$decId 返回 '$result' (pending 或 allowed)" "GET /decision/$decId 返回异常: $result"

# --- Step 3: 无 decision_id 的 needs_input（回归测试） ---
Write-Host "`n[Step 3] 回归：无 decision_id 的 needs_input..." -ForegroundColor Yellow
$ok = Send-Event @{
    agent_id="claude"; agent_name="Claude"; event_type="needs_input"
    task="Regression test"; message="Notification without approval"
}
Assert $ok "不带 decision_id 的 needs_input 正确处理" "POST 失败"

# --- Step 4: 完整生命周期 ---
Write-Host "`n[Step 4] 完整生命周期..." -ForegroundColor Yellow

$ok = Send-Event @{ agent_id="claude"; agent_name="Claude"; event_type="running"; task="Lifecycle test" }
Assert $ok "running 事件发送成功" "running 事件失败"
Start-Sleep -Milliseconds 300

$decId2 = "lifecycle_$(Get-Random)"
$ok = Send-Event @{
    agent_id="claude"; agent_name="Claude"; event_type="needs_input"
    task="Lifecycle test"; message="Allow Edit -> main.rs?"
    decision_id=$decId2; hook_event_name="PreToolUse"; tool_name="Edit"; tool_preview="main.rs"
}
Assert $ok "needs_input 事件发送成功" "needs_input 事件失败"
Start-Sleep -Milliseconds 300

$ok = Send-Event @{ agent_id="claude"; agent_name="Claude"; event_type="completed"; task="Lifecycle test"; message="Done" }
Assert $ok "completed 事件发送成功" "completed 事件失败"
Start-Sleep -Milliseconds 300

$ok = Send-Event @{ agent_id="claude"; agent_name="Claude"; event_type="idle" }
Assert $ok "idle 事件发送成功" "idle 事件失败"

# --- Step 5: 自动批准验证 ---
Write-Host "`n[Step 5] 自动批准验证..." -ForegroundColor Yellow
$decId3 = "auto_$(Get-Random)"
$null = Send-Event @{
    agent_id="claude"; agent_name="Claude"; event_type="needs_input"
    task="Auto-approve test"; message="Allow Bash -> npm test?"
    decision_id=$decId3; hook_event_name="PreToolUse"; tool_name="Bash"; tool_preview="npm test"
}

# 轮询最多 2 秒
$deadline = (Get-Date).AddSeconds(2)
$finalDecision = "pending"
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 200
    $d = Check-Decision $decId3
    if ($d -eq "allowed" -or $d -eq "denied") {
        $finalDecision = $d
        break
    }
}

if ($finalDecision -eq "allowed") {
    Write-Host "  ✅ PASS: auto-approve 生效，decision 在 2s 内变为 allowed" -ForegroundColor Green
    $script:pass++
} elseif ($finalDecision -eq "pending") {
    Write-Host "  ⏭ SKIP: auto-approve 未开启（请在 VibeHub 中开启后重新运行）" -ForegroundColor DarkYellow
    $script:skip++
} else {
    Write-Host "  ❌ FAIL: unexpected decision: $finalDecision" -ForegroundColor Red
    $script:fail++
}

# --- 清理 ---
Write-Host "`n[清理] 发送 idle 事件重置状态..." -ForegroundColor DarkGray
$null = Send-Event @{ agent_id="claude"; agent_name="Claude"; event_type="idle" }

# --- 结果 ---
Write-Host ""
$total = $pass + $fail + $skip
Write-Host "=== 结果：$pass PASS / $fail FAIL / $skip SKIP (共 $total 项) ===" -ForegroundColor $(if ($fail -eq 0) { "Green" } else { "Red" })

if ($fail -gt 0) { exit 1 }
exit 0
```

- [ ] **步骤 2：运行测试脚本验证 VibeHub 连通性**

运行：`powershell -ExecutionPolicy Bypass -File scripts/test-approval.ps1`
预期：Step 1-4 全部 PASS，Step 5 取决于 auto-approve 是否开启（PASS 或 SKIP）。

- [ ] **步骤 3：Commit**

```bash
git add scripts/test-approval.ps1
git commit -m "test: 添加审批流程端到端测试脚本"
```

---

## 任务 8：集成验证 — 手动测试所有指标

- [ ] **步骤 1：启动 VibeHub 开发模式**

运行：`cd D:\CODE\VibeHub && npm run tauri dev`
预期：顶部出现 VibeHub 胶囊。

- [ ] **步骤 2：运行自动化测试**

运行：`powershell -ExecutionPolicy Bypass -File scripts\test-approval.ps1`
预期：M1、M2、M7 验证通过（PASS）。

- [ ] **步骤 3：手动验证 M3（手动批准）和 M4（视觉反馈）**

操作：
1. 在另一个终端用真实 Claude Code 触发写操作，或用 curl 发送 `needs_input + decision_id`
2. 观察 VibeHub 胶囊下方出现按钮
3. 点击 "允许一次" 按钮
4. 验证：看到 "✓ 已批准" 绿色确认消息，持续约 1.5 秒

预期：M3 和 M4 通过。

- [ ] **步骤 4：手动验证 M6（拒绝功能）**

操作：
1. 再次触发 needs_input
2. 点击 "拒绝" 按钮
3. 验证：看到 "✗ 已拒绝" 红色确认消息

预期：M6 通过。

- [ ] **步骤 5：验证 M5（自动批准）**

操作：
1. 在 VibeHub 中勾选"自动"复选框
2. 再次运行 `scripts/test-approval.ps1`
3. 验证：Step 5 显示 PASS（decision 在 2s 内变为 allowed）
4. 验证：VibeHub 胶囊短暂显示 "⚡ 自动批准" 蓝色消息

预期：M5 通过。

- [ ] **步骤 6：验证 M8（日志记录）**

操作：
1. 点击胶囊展开日志面板（开发模式），或右键 → "📋 事件日志"（生产模式）
2. 执行上述所有操作
3. 验证：日志面板显示 ✅ DECISION、⚡ AUTO、🔄 STATE 条目

预期：M8 通过。

- [ ] **步骤 7：验证 M9（错误恢复）**

操作：
1. 如果 submit_decision 失败（如网络错误），验证按钮恢复正常状态
2. 可通过停止 VibeHub 后端来模拟

预期：M9 通过（按钮从透明恢复为可点击）。

- [ ] **步骤 8：最终 Commit**

```bash
git add -A
git commit -m "feat: 完成审批反馈与诊断系统实现"
```

---

## 指标验证清单

| # | 指标 | 任务 | 验证方式 | 状态 |
|---|------|------|----------|------|
| M1 | 事件接收率 | 任务 7 | test-approval.ps1 Step 1 | - [ ] |
| M2 | Decision 注册 | 任务 7 | test-approval.ps1 Step 2 | - [ ] |
| M3 | 手动批准成功 | 任务 8 | 手动点击 Allow | - [ ] |
| M4 | 视觉反馈显示 | 任务 3, 8 | 手动观察 "✓ 已批准" | - [ ] |
| M5 | 自动批准延迟 | 任务 4, 7 | test-approval.ps1 Step 5 | - [ ] |
| M6 | 拒绝功能 | 任务 8 | 手动点击 Deny | - [ ] |
| M7 | 状态转换 | 任务 7 | test-approval.ps1 Step 4 | - [ ] |
| M8 | 日志记录 | 任务 5, 6, 8 | 手动观察日志面板 | - [ ] |
| M9 | 错误恢复 | 任务 8 | 手动模拟失败 | - [ ] |
