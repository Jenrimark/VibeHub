// VibeHub Settings Window Logic

// ============== 导航切换 ==============
const navItems = document.querySelectorAll(".nav-item");
const pages = document.querySelectorAll(".page");

navItems.forEach((item) => {
  item.addEventListener("click", () => {
    const target = item.dataset.page;

    // 更新导航高亮
    navItems.forEach((n) => n.classList.remove("active"));
    item.classList.add("active");

    // 切换页面
    pages.forEach((p) => p.classList.remove("active"));
    const page = document.getElementById(`page-${target}`);
    if (page) page.classList.add("active");
  });
});

// ============== 初始化设置值 ==============
function initSettings() {
  const tauri = window.__TAURI__;
  if (!tauri?.core?.invoke) return;

  // Hook 状态
  tauri.core.invoke("get_hook_status").then((status) => {
    const el = document.getElementById("hookStatus");
    if (el) {
      if (status.configured) {
        el.textContent = "已连接";
        el.className = "status-badge ok";
      } else {
        el.textContent = "未配置";
        el.className = "status-badge warn";
      }
    }
    const hookPath = document.getElementById("settHookPath");
    if (hookPath && status.hook_path) {
      hookPath.value = status.hook_path;
    }
  }).catch(() => {});

  // 窗口置顶
  tauri.core.invoke("get_always_on_top").then((v) => {
    const el = document.getElementById("settAlwaysOnTop");
    if (el) el.checked = v;
  }).catch(() => {});

  // 自动批准（从主窗口 localStorage 同步）
  // 注意：Tauri 多窗口共享同一个 WebView 上下文，localStorage 是共享的
  const autoApprove = localStorage.getItem("vibehub_auto_approve") === "true";
  const el = document.getElementById("settAutoApprove");
  if (el) el.checked = autoApprove;
}

// ============== 事件绑定 ==============
// 窗口置顶
document.getElementById("settAlwaysOnTop")?.addEventListener("change", async (e) => {
  const tauri = window.__TAURI__;
  if (tauri?.core?.invoke) {
    try {
      await tauri.core.invoke("set_always_on_top", { alwaysOnTop: e.target.checked });
    } catch (err) {
      console.error("[Settings] set_always_on_top failed:", err);
    }
  }
});

// 自动批准
document.getElementById("settAutoApprove")?.addEventListener("change", (e) => {
  localStorage.setItem("vibehub_auto_approve", e.target.checked ? "true" : "false");
});

// ============== 暗色模式检测 ==============
function applyTheme() {
  const isDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  document.documentElement.setAttribute("data-theme", isDark ? "dark" : "light");
}
applyTheme();
window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", applyTheme);

// ============== 启动 ==============
initSettings();
