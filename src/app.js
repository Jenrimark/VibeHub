// VibeHub 前端：监听后端 agent-update 事件，渲染胶囊四态并本地计时。

const els = {
  pill: document.getElementById("pill"),
  mainView: document.getElementById("mainView"),
  name: document.getElementById("name"),
  task: document.getElementById("task"),
  sep: document.getElementById("sep"),
  line2: document.getElementById("line2"),
  statusText: document.getElementById("statusText"),
  timer: document.getElementById("timer"),
  allowBtn: document.getElementById("allowBtn"),
  denyBtn: document.getElementById("denyBtn"),
  contextMenu: document.getElementById("contextMenu"),
  settingsPage: document.getElementById("settingsPage"),
  settingsClose: document.getElementById("settingsClose"),
  settAlwaysOnTop: document.getElementById("settAlwaysOnTop"),
};

const STATUS_LABEL = {
  idle: "",
  running: "Running",
  needs_input: "⚠ Needs you",
  completed: "✔ Done",
  error: "⚠ Error",
};

/// 提取文件路径的文件名部分（兼容 Windows/Unix 路径）。
function basename(path) {
  if (!path) return "";
  const parts = path.replace(/\\/g, "/").split("/");
  return parts[parts.length - 1] || path;
}

let current = { status: "idle", startedAt: null, decisionId: null, connected: null };

function fmtElapsed(startedAt) {
  if (!startedAt) return "";
  const now = Math.floor(Date.now() / 1000);
  let s = Math.max(0, now - startedAt);
  const m = Math.floor(s / 60);
  s = s % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

function render(state) {
  current = {
    status: state.status,
    startedAt: state.started_at,
    decisionId: state.decision_id || null,
    connected: current.connected, // 保留连接状态
  };

  els.pill.dataset.status = state.status;

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

    // 状态行：优先显示工具上下文 > 错误信息 > 状态标签。
    let statusLabel = "";
    if (state.status === "error" && state.error) {
      statusLabel = state.error.length > 40
        ? "⚠ " + state.error.slice(0, 37) + "..."
        : "⚠ " + state.error;
    } else if (state.current_tool) {
      // 显示当前工具名 + 路径预览
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
      statusLabel = "Subagent started";
    } else if (state.hook_event === "PostToolUse" && state.message) {
      // 工具完成后的摘要
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
  }

  // 重置按钮状态
  els.allowBtn.classList.remove("acked", "denied");
  els.denyBtn.classList.remove("acked", "denied");
  els.allowBtn.disabled = false;
  els.denyBtn.disabled = false;
}

setInterval(() => {
  if (current.startedAt && current.status !== "idle"
      && current.status !== "completed" && current.status !== "error") {
    els.timer.textContent = fmtElapsed(current.startedAt);
  }
}, 1000);

// Allow / Deny：有 decision_id 时调用 Tauri command 回写，否则仅 UI 反馈。
async function handleDecision(decision) {
  const btn = decision === "allowed" ? els.allowBtn : els.denyBtn;
  const other = decision === "allowed" ? els.denyBtn : els.allowBtn;

  btn.classList.add("acked");
  other.classList.add("denied");
  btn.disabled = true;
  other.disabled = true;

  if (current.decisionId) {
    try {
      await window.__TAURI__.core.invoke("submit_decision", {
        decisionId: current.decisionId,
        decision,
      });
    } catch (e) {
      console.error("[VibeHub] submit_decision failed:", e);
      // 后端失败时恢复按钮，让用户可以重试。
      btn.classList.remove("acked");
      other.classList.remove("denied");
      btn.disabled = false;
      other.disabled = false;
    }
  }
}

els.allowBtn.addEventListener("click", () => handleDecision("allowed"));
els.denyBtn.addEventListener("click", () => handleDecision("denied"));

// 监听后端事件。优先用全局 __TAURI__（withGlobalTauri）。
function bindTauri() {
  const tauri = window.__TAURI__;
  if (tauri && tauri.event && tauri.event.listen) {
    tauri.event.listen("agent-update", (e) => {
      // 每次收到新事件恢复按钮可用状态（新的审批请求）
      els.allowBtn.disabled = false;
      els.denyBtn.disabled = false;
      render(e.payload);

      // 需要审批时自动弹出窗口到前台。
      if (e.payload.status === "needs_input" && e.payload.decision_id) {
        window.__TAURI__.core.invoke("focus_window").catch(() => {});
      }
    });
    // 监听后端服务启动失败。
    tauri.event.listen("server-error", (e) => {
      render({
        status: "error",
        agent_name: "⚠ VibeHub",
        task: "服务启动失败",
        started_at: null,
        message: "",
        decision_id: null,
      });
      console.error("[VibeHub] server-error:", e.payload);
    });

    // 监听 hooks 配置状态。
    tauri.event.listen("hooks-status", (e) => {
      const { configured, hook_path } = e.payload;
      current.connected = configured;
      const hookStatusEl = document.getElementById("hookStatus");
      if (hookStatusEl) {
        hookStatusEl.textContent = configured ? "✅ Connected" : "⚠ Not configured";
        hookStatusEl.style.color = configured ? "#34c759" : "#f5a623";
      }
      // 未连接时更新 idle 胶囊外观。
      if (current.status === "idle" && !configured) {
        els.pill.dataset.connected = "false";
      } else {
        els.pill.dataset.connected = "true";
      }
      console.log("[VibeHub] hooks-status:", configured ? "已配置" : "未配置", hook_path);
    });

    // 监听启动时发现的活跃会话。
    tauri.event.listen("session-discovered", (e) => {
      const session = e.payload;
      console.log("[VibeHub] 发现活跃会话:", session.project, session.session_id);
      // 将发现的会话渲染为 running 状态。
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
  }, 200);  // 总共等待 20 秒，兼容慢速初始化
}

render({ status: "idle", agent_name: "VibeHub", task: "idle", started_at: null, message: "", decision_id: null });

// ============== 右键上下文菜单 ==============
let settingsVisible = false;

let contextMenuHideTimer = null;

function showContextMenu(e) {
  e.preventDefault();
  e.stopPropagation();

  // 取消待执行的隐藏定时器，防止竞态。
  if (contextMenuHideTimer !== null) {
    clearTimeout(contextMenuHideTimer);
    contextMenuHideTimer = null;
  }

  const menu = els.contextMenu;
  menu.classList.remove("hidden");
  menu.classList.remove("visible");

  // 定位到鼠标位置
  const x = Math.min(e.clientX, window.innerWidth - 140);
  const y = Math.min(e.clientY, window.innerHeight - 80);
  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;

  // 触发动画
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
  }
}

// 绑定右键菜单
els.pill.addEventListener("contextmenu", showContextMenu);

// 菜单项点击
els.contextMenu.querySelectorAll(".ctx-item").forEach((item) => {
  item.addEventListener("click", () => {
    handleMenuAction(item.dataset.action);
  });
});

// 点击空白处关闭菜单
document.addEventListener("click", (e) => {
  if (!els.contextMenu.contains(e.target)) {
    hideContextMenu();
  }
});

// Escape 键关闭菜单或设置面板
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    if (settingsVisible) closeSettings();
    else hideContextMenu();
  }
});

// ============== 设置页面（全页面视图） ==============
function openSettings() {
  settingsVisible = true;
  // 隐藏胶囊，显示设置页面
  els.mainView.style.display = "none";
  els.settingsPage.classList.add("visible");

  // 同步置顶状态
  if (window.__TAURI__?.core?.invoke) {
    window.__TAURI__.core.invoke("get_always_on_top").then((v) => {
      els.settAlwaysOnTop.checked = v;
    }).catch(() => {});
  }

  // 扩大窗口以容纳设置页面
  if (window.__TAURI__?.core?.invoke) {
    window.__TAURI__.core.invoke("set_window_size", { width: 340, height: 260 });
  }
}

function closeSettings() {
  settingsVisible = false;
  els.settingsPage.classList.remove("visible");
  els.mainView.style.display = "";

  // 恢复窗口大小
  if (window.__TAURI__?.core?.invoke) {
    window.__TAURI__.core.invoke("set_window_size", { width: 340, height: 80 });
  }
}

els.settingsClose.addEventListener("click", closeSettings);

// 初始化设置页面的 hook 路径
const hookPathInput = document.getElementById("settHookPath");
if (hookPathInput) {
  hookPathInput.value = "hooks/vibehub-hook.ps1";
}

// 置顶显示开关
els.settAlwaysOnTop.addEventListener("change", async () => {
  if (window.__TAURI__?.core?.invoke) {
    try {
      await window.__TAURI__.core.invoke("set_always_on_top", {
        alwaysOnTop: els.settAlwaysOnTop.checked,
      });
    } catch (e) {
      console.error("[VibeHub] set_always_on_top failed:", e);
    }
  }
});
