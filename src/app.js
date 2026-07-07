// VibeHub 前端：监听后端 agent-update 事件，渲染胶囊四态并本地计时。

// ============== 全窗口拖拽 ==============
// Tauri 的 data-tauri-drag-region 只检查 event.target，
// 所以需要给所有非交互元素都加上该属性。
const DRAG_EXCLUDE = "button, input, select, textarea, a, label, .btn, .auto-approve, .ctx-item";
document.querySelectorAll("body *").forEach((el) => {
  if (!el.matches(DRAG_EXCLUDE)) {
    el.setAttribute("data-tauri-drag-region", "");
  }
});

const els = {
  pill: document.getElementById("pill"),
  mainView: document.getElementById("mainView"),
  name: document.getElementById("name"),
  task: document.getElementById("task"),
  sep: document.getElementById("sep"),
  line2: document.getElementById("line2"),
  statusText: document.getElementById("statusText"),
  timer: document.getElementById("timer"),
  actions: document.getElementById("actions"),
  contextMenu: document.getElementById("contextMenu"),
  logPanel: document.getElementById("logPanel"),
  logBody: document.getElementById("logBody"),
  logClear: document.getElementById("logClear"),
};

const STATUS_LABEL = {
  idle: "",
  running: "运行中",
  needs_input: "⚠ 需要操作",
  completed: "✔ 完成",
  error: "⚠ 错误",
};

/// 提取文件路径的文件名部分（兼容 Windows/Unix 路径）。
function basename(path) {
  if (!path) return "";
  const parts = path.replace(/\\/g, "/").split("/");
  return parts[parts.length - 1] || path;
}

// ============== 自动批准（localStorage 持久化） ==============
let autoApprove = localStorage.getItem("vibehub_auto_approve") === "true";

function setAutoApprove(val) {
  autoApprove = val;
  localStorage.setItem("vibehub_auto_approve", val ? "true" : "false");
}

let current = { status: "idle", startedAt: null, decisionId: null, connected: null };

// ============== 实时日志（仅开发模式） ==============
// 开发模式：前端通过 localhost 加载（Vite dev server）
// 发布版：前端通过 tauri:// 或 file:// 加载
const isDev = window.location.hostname === "localhost" || window.location.protocol === "http:";

const MAX_LOG_LINES = 80;
let logVisible = isDev && localStorage.getItem("vibehub_log_visible") === "true";
let logEntries = [];

function fmtLogTime() {
  const now = new Date();
  return `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}:${String(now.getSeconds()).padStart(2, "0")}`;
}

function appendLog(state) {
  if (!isDev) return;
  const hook = state.hook_event || state.status;
  const tool = state.current_tool || "";
  const preview = state.tool_preview || "";
  const detail = state.message || state.task || state.error || "";

  // 构造可读详情
  let detailText = "";
  if (tool) {
    detailText = preview ? `${tool}: ${preview}` : tool;
  } else if (detail) {
    detailText = detail.length > 60 ? detail.slice(0, 57) + "..." : detail;
  }

  const entry = { time: fmtLogTime(), hook, status: state.status, detail: detailText };
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

  // 裁剪超出
  while (els.logBody.children.length > MAX_LOG_LINES) {
    els.logBody.removeChild(els.logBody.firstChild);
  }

  // 自动滚动到底部
  els.logBody.scrollTop = els.logBody.scrollHeight;
}

/// 追加诊断日志条目（审批决定、自动批准、错误、状态变化）。
function appendDecisionLog(type, message) {
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

  // 控制台也记录一份
  console.log(`[VibeHub] ${cfg.icon} ${cfg.label}: ${message}`);

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

function renderLogEntries() {
  els.logBody.innerHTML = "";
  for (const entry of logEntries) {
    const line = document.createElement("div");
    line.className = "log-line";
    line.innerHTML =
      `<span class="log-time">${entry.time}</span>` +
      `<span class="log-event ${entry.status}">${entry.hook}</span>` +
      `<span class="log-detail">${escapeHtml(entry.detail)}</span>`;
    els.logBody.appendChild(line);
  }
  els.logBody.scrollTop = els.logBody.scrollHeight;
}

function escapeHtml(s) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

function toggleLog() {
  logVisible = !logVisible;
  localStorage.setItem("vibehub_log_visible", logVisible ? "true" : "false");
  if (logVisible) {
    els.logPanel.classList.remove("hidden");
    renderLogEntries();
  } else {
    els.logPanel.classList.add("hidden");
  }
  adjustWindowHeight(current.status);
}

// 点击胶囊展开/折叠日志（避免与拖拽和右键菜单冲突，仅开发模式）
let pillDragged = false;
if (isDev) {
  els.pill.addEventListener("mousedown", () => { pillDragged = false; });
  els.pill.addEventListener("mousemove", () => { pillDragged = true; });
  els.pill.addEventListener("click", (e) => {
    if (e.button !== 0 || pillDragged) return;
    if (!els.contextMenu.classList.contains("hidden")) return;
    toggleLog();
  });
}

// 清空日志
els.logClear.addEventListener("click", (e) => {
  e.stopPropagation();
  logEntries = [];
  els.logBody.innerHTML = "";
});

// 恢复上次日志面板状态（仅开发模式）
if (isDev && logVisible) {
  els.logPanel.classList.remove("hidden");
  setTimeout(() => adjustWindowHeight("idle"), 300);
}

function fmtElapsed(startedAt) {
  if (!startedAt) return "";
  const now = Math.floor(Date.now() / 1000);
  let s = Math.max(0, now - startedAt);
  const m = Math.floor(s / 60);
  s = s % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

// ============== 动态按钮渲染 ==============

/// 根据 permission_suggestions 生成按钮。
function renderButtons(suggestions) {
  els.actions.innerHTML = "";

  // 解析 suggestions -> 按钮列表
  const buttons = [];
  if (suggestions && suggestions.length > 0) {
    for (const s of suggestions) {
      const behavior = s.behavior;
      const dest = s.destination;
      const type = s.type;

      if (behavior === "deny") {
        buttons.push({ label: "拒绝", decision: "denied", cls: "deny" });
      } else if (behavior === "allow") {
        if (dest === "session") {
          buttons.push({ label: "允许一次", decision: "allowed", cls: "allow" });
        } else {
          // projectSettings / userSettings / localSettings
          buttons.push({ label: "始终允许", decision: "allowed", cls: "allow-always" });
        }
      } else if (type === "setMode") {
        buttons.push({ label: "自动批准", decision: "allowed", cls: "allow" });
      }
    }
    // 去重（同 decision+label 只保留一个）
    const seen = new Set();
    const deduped = [];
    for (const b of buttons) {
      const key = b.label;
      if (!seen.has(key)) {
        seen.add(key);
        deduped.push(b);
      }
    }
    buttons.length = 0;
    buttons.push(...deduped);
  }

  // 无 suggestions 时回退到默认：拒绝 + 允许一次
  if (buttons.length === 0) {
    buttons.push({ label: "拒绝", decision: "denied", cls: "deny" });
    buttons.push({ label: "允许一次", decision: "allowed", cls: "allow" });
  }

  // 确保"拒绝"在最左边
  buttons.sort((a, b) => {
    if (a.decision === "denied") return -1;
    if (b.decision === "denied") return 1;
    return 0;
  });

  for (const b of buttons) {
    const btn = document.createElement("button");
    btn.className = `btn ${b.cls}`;
    btn.textContent = b.label;
    btn.addEventListener("click", () => handleDecision(b.decision, btn));
    els.actions.appendChild(btn);
  }

  // 自动批准 checkbox
  const label = document.createElement("label");
  label.className = "auto-approve";
  const cb = document.createElement("input");
  cb.type = "checkbox";
  cb.checked = autoApprove;
  cb.addEventListener("change", () => {
    setAutoApprove(cb.checked);
    // 开启自动批准时，如果有待审批请求，立即批准
    if (cb.checked && current.decisionId) {
      handleDecision("allowed", null);
    }
  });
  label.appendChild(cb);
  label.appendChild(document.createTextNode("自动"));
  els.actions.appendChild(label);
}

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

function render(state) {
  const previousStatus = current.status;

  // 保留 decisionId：后端 preservesActionableState 在审批期间会持续 emit
  // 同一个 decision_id，但普通事件（PostToolUse 等）的 payload 不含 decision_id。
  // 只在 payload 明确携带新 decision_id 时才更新，避免审批期间被覆盖为 null。
  // 状态变为非 needs_input（已审批完成 / 会话结束）时自然清空。
  if (state.decision_id) {
    current.decisionId = state.decision_id;
  } else if (state.status !== "needs_input") {
    current.decisionId = null;
  }
  current.status = state.status;
  current.startedAt = state.started_at;

  // 诊断日志：记录状态变化
  if (state.status !== previousStatus) {
    appendDecisionLog("state", `${previousStatus} → ${state.status}`);
  }

  els.pill.dataset.status = state.status;
  els.mainView.dataset.status = state.status;

  // 更新连接状态指示（仅 idle 时显示）。
  if (state.status === "idle" && current.connected === false) {
    els.pill.dataset.connected = "false";
  } else if (current.connected !== null) {
    els.pill.dataset.connected = current.connected ? "true" : "false";
  }

  if (state.status === "idle") {
    els.name.textContent = "VibeHub";
    els.task.textContent = current.connected === false ? "未连接" : "idle";
    els.sep.style.display = "";
    els.statusText.textContent = "";
    els.timer.textContent = "";
    els.line2.style.display = "none";
    els.actions.innerHTML = "";
  } else {
    els.name.textContent = state.agent_name || "Agent";

    // 任务行：completed 时优先显示 last_message。
    let taskText = state.task || state.message || "...";
    if (state.status === "completed" && state.last_message) {
      taskText = state.last_message.length > 60
        ? state.last_message.slice(0, 57) + "..."
        : state.last_message;
    }
    els.task.textContent = taskText;
    els.sep.style.display = "";

    // completed/error 时清除残留计时器文本。
    if (state.status === "completed" || state.status === "error") {
      els.timer.textContent = "";
    }

    // 状态行：优先显示工具上下文 > 错误信息 > 状态标签。
    let statusLabel = "";
    if (state.status === "error" && state.error) {
      statusLabel = state.error.length > 40
        ? "⚠ " + state.error.slice(0, 37) + "..."
        : "⚠ " + state.error;
    } else if (state.current_tool) {
      const tool = state.current_tool;
      const preview = state.tool_preview;
      if (preview && (tool === "Write" || tool === "Edit" || tool === "MultiEdit")) {
        statusLabel = tool + " → " + basename(preview);
      } else if (preview && tool === "Bash") {
        statusLabel = "Bash: " + (preview.length > 30 ? preview.slice(0, 27) + "..." : preview);
      } else if (preview) {
        statusLabel = tool + ": " + (preview.length > 30 ? preview.slice(0, 27) + "..." : preview);
      } else {
        statusLabel = tool;
      }
    } else if (state.hook_event === "SubagentStart") {
      statusLabel = "子代理启动";
    } else if (state.hook_event === "PostToolUse" && state.message) {
      statusLabel = state.message.length > 40
        ? state.message.slice(0, 37) + "..."
        : state.message;
    } else {
      statusLabel = state.message || STATUS_LABEL[state.status] || "";
    }

    const timerLabel = fmtElapsed(state.started_at);
    els.statusText.textContent = statusLabel;
    els.timer.textContent = timerLabel;
    els.line2.style.display = (statusLabel || timerLabel) ? "" : "none";

    // 需要审批时渲染按钮。
    if (state.status === "needs_input" && state.decision_id) {
      renderButtons(state.permission_suggestions);
    }
  }

  // 调整窗口高度
  adjustWindowHeight(state.status);
}

function adjustWindowHeight(status) {
  if (window.__TAURI__?.core?.invoke) {
    let h = status === "needs_input" ? 120 : 80;
    if (logVisible) h += 240; // 日志面板空间
    window.__TAURI__.core.invoke("set_window_size", { width: 340, height: h });
  }
}

setInterval(() => {
  if (current.startedAt && current.status !== "idle"
      && current.status !== "completed" && current.status !== "error") {
    els.timer.textContent = fmtElapsed(current.startedAt);
  }
}, 1000);

// 监听后端事件。
function bindTauri() {
  const tauri = window.__TAURI__;
  if (tauri && tauri.event && tauri.event.listen) {
    tauri.event.listen("agent-update", (e) => {
      const payload = e.payload;

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

      render(payload);
      appendLog(payload);

      // 需要审批时自动弹出窗口到前台。
      if (payload.status === "needs_input" && payload.decision_id && !autoApprove) {
        window.__TAURI__.core.invoke("focus_window").catch(() => {});
      }
    });

    tauri.event.listen("server-error", (e) => {
      render({
        status: "error",
        agent_name: "⚠ VibeHub",
        task: "服务启动失败",
        started_at: null,
        message: "",
        decision_id: null,
      });
      appendLog({ hook_event: "ServerError", status: "error", message: e.payload });
      console.error("[VibeHub] server-error:", e.payload);
    });

    tauri.event.listen("hooks-status", (e) => {
      const { configured, hook_path } = e.payload;
      current.connected = configured;
      const hookStatusEl = document.getElementById("hookStatus");
      if (hookStatusEl) {
        hookStatusEl.textContent = configured ? "✅ 已连接" : "⚠ 未配置";
        hookStatusEl.style.color = configured ? "#34c759" : "#f5a623";
      }
      if (current.status === "idle" && !configured) {
        els.pill.dataset.connected = "false";
      } else {
        els.pill.dataset.connected = "true";
      }
      console.log("[VibeHub] hooks-status:", configured ? "已配置" : "未配置", hook_path);
    });

    tauri.event.listen("session-discovered", (e) => {
      const session = e.payload;
      console.log("[VibeHub] 发现活跃会话:", session.project, session.session_id);
      appendLog({ hook_event: "SessionDiscovered", status: "running", message: `${session.project} (${session.session_id.slice(0, 8)})` });
      render({
        status: "running",
        agent_name: "Claude",
        task: session.last_task,
        started_at: session.last_activity || null,
        message: session.project,
        decision_id: null,
      });
    });
    return true;
  }
  return false;
}

if (!bindTauri()) {
  let tries = 0;
  const t = setInterval(() => {
    if (bindTauri() || ++tries > 100) clearInterval(t);
  }, 200);
}

render({ status: "idle", agent_name: "VibeHub", task: "idle", started_at: null, message: "", decision_id: null });

// ============== 右键上下文菜单 ==============
let settingsVisible = false;

let contextMenuHideTimer = null;

function showContextMenu(e) {
  e.preventDefault();
  e.stopPropagation();

  if (contextMenuHideTimer !== null) {
    clearTimeout(contextMenuHideTimer);
    contextMenuHideTimer = null;
  }

  const menu = els.contextMenu;
  menu.classList.remove("hidden");
  menu.classList.remove("visible");

  const x = Math.min(e.clientX, window.innerWidth - 140);
  const y = Math.min(e.clientY, window.innerHeight - 80);
  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;

  requestAnimationFrame(() => menu.classList.add("visible"));
}

function hideContextMenu() {
  const menu = els.contextMenu;
  menu.classList.remove("visible");
  contextMenuHideTimer = setTimeout(() => {
    menu.classList.add("hidden");
    contextMenuHideTimer = null;
  }, 150);
}

function handleMenuAction(action) {
  hideContextMenu();

  if (action === "settings") {
    openSettings();
  } else if (action === "log") {
    toggleLog();
  }
}

els.pill.addEventListener("contextmenu", showContextMenu);

els.contextMenu.querySelectorAll(".ctx-item").forEach((item) => {
  item.addEventListener("click", () => {
    handleMenuAction(item.dataset.action);
  });
});

document.addEventListener("click", (e) => {
  if (!els.contextMenu.contains(e.target)) {
    hideContextMenu();
  }
});

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    hideContextMenu();
  }
});

// ============== 设置页面（独立窗口） ==============
function openSettings() {
  if (window.__TAURI__?.core?.invoke) {
    window.__TAURI__.core.invoke("open_settings").catch((e) => {
      console.error("[VibeHub] open_settings failed:", e);
    });
  }
}
