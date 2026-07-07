# VibeHub 审批反馈与诊断系统 设计规格

> 日期：2026-07-07  
> 状态：待批准  
> 范围：VibeHub 前端 + 测试脚本（不涉及 Rust 后端改动）

## 问题陈述

VibeHub 的审批功能（自动批准 + 手动 Allow/Deny 按钮）在后端正常工作，但用户看不到任何视觉反馈。点击按钮后，按钮变为半透明然后消失，整个过程不到 1 秒，用户无法确认操作是否成功。

**根因**：`submit_decision` 成功后，`render()` 立即将状态切回 `running` 并清空按钮区域，没有任何确认过渡动画或提示消息。

## 目标

1. 用户批准/拒绝后看到明确的视觉确认
2. 自动批准触发时有快速提示
3. 提供可选的诊断日志面板，显示完整的审批事件流
4. 提供定量测试脚本，验证每个环节的正确性

## 非目标

- 不修改 Rust 后端逻辑（后端已验证正常工作）
- 不修改 hook 脚本（已验证正常）
- 不添加新 Tauri 命令

---

## 模块 1：审批视觉反馈

### 设计

在 `handleDecision` 提交成功后，不立即清空按钮区域，而是用确认消息替换按钮，持续 1.5 秒后再切回 running 状态。

#### 三种确认状态

| 场景 | 显示文本 | 颜色 | 持续时间 |
|------|----------|------|----------|
| 手动批准 | ✓ 已批准 | `var(--done)` #34c759 | 1.5s |
| 手动拒绝 | ✗ 已拒绝 | `var(--error)` #ff453a | 1.5s |
| 自动批准 | ⚡ 自动批准 | `var(--running)` #3b82f6 | 0.8s |

#### 交互流程

**手动批准/拒绝**：
```
handleDecision("allowed", btn)
  → 禁用所有按钮，标记 acked（现有逻辑）
  → 调用 submit_decision（现有逻辑）
  → 成功后：清空 actions 区，插入确认消息 div
  → 1.5s 后：淡出确认消息
  → 此时 render() 会在收到 agent-update 后自然清空
```

**自动批准**：
```
agent-update 监听器中：
  → 检测到 autoApprove + needs_input + decision_id
  → 调用 submit_decision（现有逻辑）
  → 在 render() 之前显示 "⚡ 自动批准" 消息
  → 0.8s 后自动消失
```

#### 新增 CSS

```css
/* 审批确认消息 */
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
  to { opacity: 1; transform: scale(1); }
}
```

#### 实现位置

- `src/app.js` — `handleDecision()` 函数（第 254-282 行）
- `src/app.js` — `agent-update` 监听器中的 auto-approve 逻辑（第 398-404 行）
- `src/style.css` — 新增 `.decision-feedback` 样式

---

## 模块 2：诊断事件面板

### 设计

扩展现有的 log 面板，增加事件类型标记，让每个状态变化可追溯。

#### 事件分类

| 前缀 | 含义 | 触发时机 |
|------|------|----------|
| 📥 EVENT | 收到 hook 事件 | `agent-update` 事件到达 |
| ✅ DECISION | 提交审批决定 | `handleDecision` 成功 |
| ❌ ERROR | 错误 | Tauri invoke 失败 |
| ⚡ AUTO | 自动批准触发 | auto-approve 逻辑执行 |
| 🔄 STATE | 状态变化 | `render()` 中 status 发生变化 |

#### 实现

在 `appendLog` 的基础上新增 `appendDecisionLog(type, message)` 函数：

```javascript
function appendDecisionLog(type, message) {
  const typeConfig = {
    decision: { icon: "✅", cls: "done" },
    auto:     { icon: "⚡", cls: "running" },
    error:    { icon: "❌", cls: "error" },
    state:    { icon: "🔄", cls: "info" },
  };
  const cfg = typeConfig[type] || typeConfig.state;
  // 追加到 log 面板，样式与 appendLog 一致
}
```

- 在 `handleDecision` 成功后调用 `appendDecisionLog("decision", "Allowed dec_xxx")`
- 在 auto-approve 触发时调用 `appendDecisionLog("auto", "Allowed dec_xxx")`
- 在 `render()` 中检测到状态变化时调用 `appendDecisionLog("state", "idle → running")`

#### 日志面板可见性

- 开发模式（localhost）：同现有逻辑，点击胶囊切换
- 生产模式：右键菜单新增 "📋 事件日志" 选项，可开启日志面板

---

## 模块 3：端到端测试脚本

### 设计

新增 `scripts/test-approval.ps1`，模拟完整的审批生命周期，每步输出 PASS/FAIL。

#### 测试步骤

```
Step 1: 基础连通性
  POST running event → 期望 200 ok

Step 2: 带 decision_id 的 needs_input
  POST needs_input + decision_id → 期望 200 ok
  GET /decision/{id} → 期望 "pending"

Step 3: 手动审批（模拟前端 submit）
  POST /internal/decision（通过 curl 发 Tauri 事件或直接发 decision）
  → 注意：submit_decision 是 Tauri 命令，不是 HTTP 端点
  → 替代方案：在前端 click（需要 UI 自动化）
  → 简化方案：只验证 hook 能收到 decision（通过发事件后查询）

Step 4: 不带 decision_id 的 needs_input（回归测试）
  POST needs_input 无 decision_id → 期望 200 ok
  → 验证不会创建 pending decision

Step 5: 完整生命周期
  running → needs_input(+decision_id) → completed → idle
  → 每步验证 HTTP 状态和 decision 状态

Step 6: 自动批准验证
  （需要用户手动开启 auto-approve）
  POST needs_input + decision_id → 立即查询 decision
  → 期望在 1s 内变为 "allowed"
```

#### 输出格式

```
=== VibeHub 审批流程测试 ===

[Step 1] 基础连通性...
  ✅ PASS: POST /event 返回 ok

[Step 2] Decision 注册...
  ✅ PASS: POST needs_input 返回 ok
  ✅ PASS: GET /decision/test_001 返回 pending

[Step 3] 手动审批...
  ⏭ SKIP: 需要 UI 操作（请手动点击 Allow 按钮）
  → 发送了 needs_input 事件，请在 VibeHub 中点击 Allow

[Step 4] 回归：无 decision_id 的 needs_input...
  ✅ PASS: 不带 decision_id 的事件正确处理

[Step 5] 完整生命周期...
  ✅ PASS: running → needs_input → completed → idle

[Step 6] 自动批准验证...
  ✅ PASS: decision 在 1s 内自动变为 allowed
  ⏭ SKIP: auto-approve 未开启（请开启后重新运行）

=== 结果：6 PASS / 0 FAIL / 2 SKIP ===
```

---

## 模块 4：定量评价指标

### 指标定义

| # | 指标 | 验证方法 | 通过标准 |
|---|------|----------|----------|
| M1 | 事件接收率 | test-approval.ps1 发 5 种事件 | 5/5 返回 ok |
| M2 | Decision 注册 | 发送 needs_input + decision_id 后查询 | 返回 pending |
| M3 | 手批成功 | 手动点击 Allow 后查询 decision | 返回 allowed |
| M4 | 视觉反馈 | 批准后观察 UI | 看到 "✓ 已批准" ≥1s |
| M5 | 自动批准延迟 | 开启 auto-approve 后发事件 | decision 在 1s 内变为 allowed |
| M6 | 拒绝功能 | 手动点击 Deny | 返回 denied |
| M7 | 状态转换 | 完整生命周期 | 四态正确切换 |
| M8 | 日志记录 | 执行上述所有操作 | 日志面板显示每步记录 |
| M9 | 错误恢复 | submit_decision 失败时 | 按钮恢复正常状态 |

### 验证脚本

`scripts/test-approval.ps1` 自动验证 M1、M2、M5、M7。M3、M4、M6、M8、M9 需要用户手动操作 + 观察。

---

## 文件变更清单

| 文件 | 变更类型 | 说明 |
|------|----------|------|
| `src/app.js` | 修改 | handleDecision 增加反馈 UI；auto-approve 增加反馈；render 增加状态日志 |
| `src/style.css` | 修改 | 新增 .decision-feedback 系列样式 |
| `src/index.html` | 不变 | — |
| `src-tauri/src/*.rs` | 不变 | — |
| `hooks/vibehub-hook.ps1` | 不变 | — |
| `scripts/test-approval.ps1` | 新增 | 端到端测试脚本 |

## 不做的事

- 不修改 Rust 后端
- 不修改 hook 脚本
- 不添加新 Tauri 命令
- 不添加新依赖
- 不改变现有的审批轮询机制
