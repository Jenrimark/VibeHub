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

let current = { status: "idle", startedAt: null, decisionId: null };

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
  };

  els.pill.dataset.status = state.status;

  if (state.status === "idle") {
    els.name.textContent = "VibeHub";
    els.task.textContent = "idle";
    els.sep.style.display = "";
    els.statusText.textContent = "";
    els.timer.textContent = "";
    els.line2.style.display = "none";
  } else {
    els.name.textContent = state.agent_name || "Agent";
    const taskText = state.task || state.message || "...";
    els.task.textContent = taskText;
    els.sep.style.display = "";
    const statusLabel = state.message || STATUS_LABEL[state.status] || "";
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
    return true;
  }
  return false;
}

if (!bindTauri()) {
  let tries = 0;
  const t = setInterval(() => {
    if (bindTauri() || ++tries > 20) clearInterval(t);
  }, 100);
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
