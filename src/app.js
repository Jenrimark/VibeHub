// VibeHub 前端：监听后端 agent-update 事件，渲染胶囊四态并本地计时。

const els = {
  pill: document.getElementById("pill"),
  name: document.getElementById("name"),
  task: document.getElementById("task"),
  sep: document.getElementById("sep"),
  statusText: document.getElementById("statusText"),
  timer: document.getElementById("timer"),
  allowBtn: document.getElementById("allowBtn"),
  denyBtn: document.getElementById("denyBtn"),
};

// 各状态的展示文案。
const STATUS_LABEL = {
  idle: "",
  running: "Running",
  needs_input: "⚠ Needs you",
  completed: "✔ Done",
  error: "⚠ Error",
};

let current = { status: "idle", startedAt: null };

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
  };

  els.pill.dataset.status = state.status;

  if (state.status === "idle") {
    els.name.textContent = "VibeHub";
    els.task.textContent = "idle";
    els.sep.style.display = "";
    els.statusText.textContent = "";
    els.timer.textContent = "";
  } else {
    els.name.textContent = state.agent_name || "Agent";
    const taskText = state.task || state.message || "...";
    els.task.textContent = taskText;
    els.sep.style.display = "";
    els.statusText.textContent = state.message || STATUS_LABEL[state.status] || "";
    els.timer.textContent = fmtElapsed(state.started_at);
  }

  // 重置按钮 acked 态
  els.allowBtn.classList.remove("acked");
  els.denyBtn.classList.remove("acked");
}

// 每秒刷新计时（运行/需操作/完成时持续显示用时）。
setInterval(() => {
  if (current.startedAt && current.status !== "idle") {
    els.timer.textContent = fmtElapsed(current.startedAt);
  }
}, 1000);

// MVP：按钮仅 UI 反馈，不回写给 Claude。
els.allowBtn.addEventListener("click", () => els.allowBtn.classList.add("acked"));
els.denyBtn.addEventListener("click", () => els.denyBtn.classList.add("acked"));

// 监听后端事件。优先用全局 __TAURI__（withGlobalTauri）。
function bindTauri() {
  const tauri = window.__TAURI__;
  if (tauri && tauri.event && tauri.event.listen) {
    tauri.event.listen("agent-update", (e) => render(e.payload));
    return true;
  }
  return false;
}

if (!bindTauri()) {
  // __TAURI__ 可能稍后注入，重试几次。
  let tries = 0;
  const t = setInterval(() => {
    if (bindTauri() || ++tries > 20) clearInterval(t);
  }, 100);
}

// 初始渲染空闲态。
render({ status: "idle", agent_name: "VibeHub", task: "idle", started_at: null, message: "" });
