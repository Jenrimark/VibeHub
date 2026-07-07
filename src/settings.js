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
  if (!tauri?.core?.invoke) {
    console.warn("[Settings] Tauri API not available");
    return;
  }

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
  }).catch((e) => console.warn("[Settings] get_hook_status:", e));

  // 窗口置顶
  tauri.core.invoke("get_always_on_top").then((v) => {
    const el = document.getElementById("settAlwaysOnTop");
    if (el) el.checked = v;
  }).catch((e) => console.warn("[Settings] get_always_on_top:", e));

  // 自动批准（localStorage 共享）
  const autoApprove = localStorage.getItem("vibehub_auto_approve") === "true";
  const el = document.getElementById("settAutoApprove");
  if (el) el.checked = autoApprove;
}

// ============== 事件绑定 ==============
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

document.getElementById("settAutoApprove")?.addEventListener("change", (e) => {
  localStorage.setItem("vibehub_auto_approve", e.target.checked ? "true" : "false");
});

// ============== 启动 ==============
// 等待 Tauri API 就绪
if (window.__TAURI__) {
  initSettings();
} else {
  let tries = 0;
  const t = setInterval(() => {
    if (window.__TAURI__ || ++tries > 50) {
      clearInterval(t);
      if (window.__TAURI__) initSettings();
    }
  }, 100);
}
