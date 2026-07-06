//! VibeHub 状态：单一数据源 + Agent 状态机。
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Agent 当前所处状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Running,
    NeedsInput,
    Completed,
    Error,
}

impl AgentStatus {
    /// 从事件类型字符串解析状态。未知值回退到 Idle。
    pub fn from_event(s: &str) -> AgentStatus {
        match s {
            "running" => AgentStatus::Running,
            "needs_input" => AgentStatus::NeedsInput,
            "completed" => AgentStatus::Completed,
            "error" => AgentStatus::Error,
            _ => AgentStatus::Idle,
        }
    }
}

/// 单个 Agent 的完整状态，会被序列化后 emit 给前端。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub agent_id: String,
    pub agent_name: String,
    pub task: String,
    pub status: AgentStatus,
    /// 进入 Running 的 unix 秒；前端据此本地累加计时。
    pub started_at: Option<i64>,
    pub message: String,
    /// 当前待审批的 decision_id，无则为 None。
    pub decision_id: Option<String>,
    // --- 全活动监控新增字段 ---
    /// 当前正在执行的工具名（如 "Edit"、"Bash"），工具完成后清空。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_tool: Option<String>,
    /// 工具输入预览（如文件路径、命令内容）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_preview: Option<String>,
    /// 最后一条助手消息（Stop 时提取）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message: Option<String>,
    /// 错误信息（PostToolUseFailure / StopFailure）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// 原始 hook 事件名（如 "PreToolUse"、"SubagentStart"）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_event: Option<String>,
}

impl AgentState {
    pub fn new(agent_id: &str, agent_name: &str) -> Self {
        AgentState {
            agent_id: agent_id.to_string(),
            agent_name: agent_name.to_string(),
            task: String::new(),
            status: AgentStatus::Idle,
            started_at: None,
            message: String::new(),
            decision_id: None,
            current_tool: None,
            tool_preview: None,
            last_message: None,
            error: None,
            hook_event: None,
        }
    }

    /// 应用一个事件，推进状态机。`now` 为当前 unix 秒（便于测试注入）。
    pub fn apply(&mut self, ev: &IncomingEvent, now: i64) {
        let status = AgentStatus::from_event(&ev.event_type);
        let hook_event = ev.hook_event_name.clone();

        if let Some(name) = &ev.agent_name {
            if !name.is_empty() {
                self.agent_name = name.clone();
            }
        }
        // 仅在 task/message 非空时更新，避免 hook 发送空字符串时清空 UI 显示。
        if let Some(task) = &ev.task {
            if !task.is_empty() {
                self.task = task.clone();
            }
        }
        if let Some(msg) = &ev.message {
            if !msg.is_empty() {
                self.message = msg.clone();
            }
        }

        // 更新 hook_event（始终记录最新的原始事件名）。
        self.hook_event = hook_event.clone();

        // ============ preservesActionableState 保护 ============
        // 审批等待期间，普通的 running 事件（PostToolUse、SubagentStart/Stop 等）
        // 不应覆盖 needs_input 状态。只更新元信息，不改变 status。
        // 但新的审批请求（event_type == "needs_input"）允许覆盖。
        let is_new_needs_input = status == AgentStatus::NeedsInput;
        let is_activity_event = matches!(
            hook_event.as_deref(),
            Some("PreToolUse")
                | Some("PostToolUse")
                | Some("Notification")
                | Some("SubagentStart")
                | Some("SubagentStop")
                | Some("PreCompact")
        );
        if is_activity_event
            && self.status == AgentStatus::NeedsInput
            && self.decision_id.is_some()
            && !is_new_needs_input
        {
            // 只更新 tool_preview 等辅助信息，保持 needs_input 不变。
            self.update_tool_context(ev);
            return;
        }

        // ============ 按 hook_event_name 精细化映射 ============
        // 先用 event_type 设定默认状态，match 块可按需覆盖。
        self.status = status;

        // 当 event_type 已明确为 needs_input/error 时，优先信任它，
        // 因为这是 hook 脚本经过业务逻辑（写工具判断、PermissionRequest 等）
        // 映射后的结果，比 hook_event_name 更精确。
        match hook_event.as_deref() {
            // 工具开始：记录当前工具名和预览。
            Some("PreToolUse") => {
                // event_type 为 needs_input 时信任 hook 的映射（审批场景）。
                if status != AgentStatus::NeedsInput {
                    self.status = AgentStatus::Running;
                }
                self.update_tool_context(ev);
                self.error = None;
            }
            // 工具完成：清除当前工具，更新 message 为完成摘要。
            Some("PostToolUse") => {
                self.status = AgentStatus::Running;
                self.current_tool = None;
                if let Some(preview) = &ev.response_preview {
                    if !preview.is_empty() {
                        self.message = format!("done: {}", preview);
                    }
                }
                self.tool_preview = None;
                self.error = None;
            }
            // 工具失败：进入 Error 状态。
            Some("PostToolUseFailure") => {
                self.status = AgentStatus::Error;
                self.current_tool = None;
                self.error = ev.error.clone();
                self.message = ev
                    .error
                    .clone()
                    .unwrap_or_else(|| "Tool failed".to_string());
            }
            // 权限请求：需要用户输入。
            Some("PermissionRequest") => {
                self.status = AgentStatus::NeedsInput;
                self.update_tool_context(ev);
            }
            // 权限被拒绝：回到 running。
            Some("PermissionDenied") => {
                self.status = AgentStatus::Running;
            }
            // 通知：有消息体时需要用户注意。
            Some("Notification") => {
                // 保持当前状态，除非消息非空（由 hook 映射为 needs_input）。
                if status == AgentStatus::NeedsInput {
                    self.status = AgentStatus::NeedsInput;
                }
                self.update_tool_context(ev);
            }
            // Subagent 开始：更新 task 信息。
            Some("SubagentStart") => {
                self.status = AgentStatus::Running;
                if let Some(at) = &ev.agent_type {
                    if !at.is_empty() {
                        self.task = format!("Subagent: {}", at);
                    }
                }
            }
            // Subagent 结束：保持 running。
            Some("SubagentStop") => {
                // 不改变状态，只记录事件。
            }
            // Stop / StopFailure：会话完成。
            Some("Stop") => {
                self.status = AgentStatus::Completed;
                self.current_tool = None;
                self.last_message = ev.last_message.clone();
            }
            Some("StopFailure") => {
                self.status = AgentStatus::Error;
                self.current_tool = None;
                self.error = ev.error.clone();
                self.message = ev
                    .error
                    .clone()
                    .unwrap_or_else(|| "Stop failed".to_string());
            }
            // SessionEnd：回到空闲。
            Some("SessionEnd") => {
                self.status = AgentStatus::Idle;
                self.started_at = None;
                self.task.clear();
                self.decision_id = None;
                self.current_tool = None;
                self.tool_preview = None;
                self.last_message = None;
                self.error = None;
            }
            // 其他 / 未知：使用 event_type 映射的状态。
            _ => {
                self.status = status;
                match status {
                    AgentStatus::Idle => {
                        self.started_at = None;
                        self.task.clear();
                        self.decision_id = None;
                    }
                    _ => {}
                }
                self.update_tool_context(ev);
            }
        }

        // 计时逻辑：仅在状态实际变化时处理。
        match self.status {
            AgentStatus::Running => {
                if self.started_at.is_none() {
                    self.started_at = Some(now);
                }
            }
            AgentStatus::Idle => {
                self.started_at = None;
            }
            _ => {}
        }
    }

    /// 更新工具上下文信息（current_tool、tool_preview）。
    fn update_tool_context(&mut self, ev: &IncomingEvent) {
        if let Some(tool) = &ev.tool_name {
            if !tool.is_empty() {
                self.current_tool = Some(tool.clone());
            }
        }
        if let Some(preview) = &ev.tool_preview {
            if !preview.is_empty() {
                self.tool_preview = Some(preview.clone());
            }
        }
    }
}

/// 审批决定结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Pending,
    Allowed,
    Denied,
}

/// hook 推送过来的事件载荷。除 event_type 外均可选。
#[derive(Debug, Clone, Deserialize)]
pub struct IncomingEvent {
    pub event_type: String,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub task: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    /// 需要审批时由 hook 生成并附带，用于关联用户决定。
    #[serde(default)]
    pub decision_id: Option<String>,
    // --- 全活动监控新增字段 ---
    /// 原始 hook 事件名（如 "PreToolUse"、"PostToolUse"、"SubagentStart"）。
    #[serde(default)]
    pub hook_event_name: Option<String>,
    /// 当前工具名（如 "Write"、"Bash"）。
    #[serde(default)]
    pub tool_name: Option<String>,
    /// 工具输入预览（hook 脚本提取的摘要）。
    #[serde(default)]
    pub tool_preview: Option<String>,
    /// 工具响应预览。
    #[serde(default)]
    pub response_preview: Option<String>,
    /// 错误信息。
    #[serde(default)]
    pub error: Option<String>,
    /// 最后一条助手消息。
    #[serde(default)]
    pub last_message: Option<String>,
    /// subagent 类型。
    #[serde(default)]
    pub agent_type: Option<String>,
    /// 是否中断（hook 发送，预留供 UI 展示）。
    #[serde(default)]
    #[allow(dead_code)]
    pub is_interrupt: Option<bool>,
}

impl IncomingEvent {
    /// 取 agent_id，缺省为 "claude"。
    pub fn agent_id(&self) -> String {
        self.agent_id
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "claude".to_string())
    }
}

/// 全局状态：多 agent 用 map 管理（MVP 实际只有 claude 一条）。
#[derive(Debug, Default)]
pub struct AppState {
    pub agents: HashMap<String, AgentState>,
    /// 待审批决定：decision_id -> Decision。
    pub decisions: HashMap<String, Decision>,
    /// 当前活跃的 decision_id（供前端展示）。
    pub active_decision_id: Option<String>,
}

impl AppState {
    /// 应用事件并返回更新后的 AgentState 克隆（供 emit）。
    pub fn handle(&mut self, ev: &IncomingEvent, now: i64) -> AgentState {
        let id = ev.agent_id();
        let default_name = capitalize(&id);
        let agent = self
            .agents
            .entry(id.clone())
            .or_insert_with(|| AgentState::new(&id, &default_name));
        agent.apply(ev, now);

        // 若附带 decision_id，登记为 Pending 并记为当前活跃。
        // 仅在新 decision_id 时插入，不覆盖已决定的结果。
        if let Some(did) = &ev.decision_id {
            if !did.is_empty() {
                self.decisions.entry(did.clone()).or_insert(Decision::Pending);
                self.active_decision_id = Some(did.clone());
                agent.decision_id = Some(did.clone());
            }
        } else if ev.event_type != "needs_input" {
            // 非审批事件时清空活跃 decision_id。
            self.active_decision_id = None;
        }

        agent.clone()
    }

    /// 用户提交决定，返回是否找到该 decision_id。
    /// 决策完成后同时清除 active_decision_id 和对应 agent 的 decision_id。
    pub fn submit_decision(&mut self, decision_id: &str, decision: Decision) -> bool {
        if let Some(d) = self.decisions.get_mut(decision_id) {
            *d = decision;
            // 清除活跃 decision_id。
            if self.active_decision_id.as_deref() == Some(decision_id) {
                self.active_decision_id = None;
            }
            // 清除持有该 decision_id 的 agent 状态。
            for agent in self.agents.values_mut() {
                if agent.decision_id.as_deref() == Some(decision_id) {
                    agent.decision_id = None;
                    break;
                }
            }
            true
        } else {
            false
        }
    }

    /// 查询决定结果。
    pub fn get_decision(&self, decision_id: &str) -> Decision {
        self.decisions
            .get(decision_id)
            .copied()
            .unwrap_or(Decision::Pending)
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// 启动时发现的活跃会话信息，emit 给前端。
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredSession {
    pub session_id: String,
    pub project: String,
    pub last_task: String,
    pub last_activity: i64,
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(t: &str) -> IncomingEvent {
        IncomingEvent {
            event_type: t.to_string(),
            agent_id: None,
            agent_name: None,
            task: None,
            message: None,
            decision_id: None,
            hook_event_name: None,
            tool_name: None,
            tool_preview: None,
            response_preview: None,
            error: None,
            last_message: None,
            agent_type: None,
            is_interrupt: None,
        }
    }

    fn ev_with_hook(t: &str, hook: &str) -> IncomingEvent {
        let mut e = ev(t);
        e.hook_event_name = Some(hook.to_string());
        e
    }

    #[test]
    fn full_lifecycle() {
        let mut app = AppState::default();

        // running 开始计时
        let mut e = ev("running");
        e.task = Some("Fix auth bug".into());
        let s = app.handle(&e, 1000);
        assert_eq!(s.status, AgentStatus::Running);
        assert_eq!(s.started_at, Some(1000));
        assert_eq!(s.task, "Fix auth bug");

        // needs_input 保留计时起点
        let s = app.handle(&ev("needs_input"), 1010);
        assert_eq!(s.status, AgentStatus::NeedsInput);
        assert_eq!(s.started_at, Some(1000));

        // completed 仍保留起点
        let s = app.handle(&ev("completed"), 1020);
        assert_eq!(s.status, AgentStatus::Completed);
        assert_eq!(s.started_at, Some(1000));

        // idle 清空
        let s = app.handle(&ev("idle"), 1030);
        assert_eq!(s.status, AgentStatus::Idle);
        assert_eq!(s.started_at, None);
        assert!(s.task.is_empty());
    }

    #[test]
    fn running_does_not_reset_timer_when_already_running() {
        let mut app = AppState::default();
        app.handle(&ev("running"), 500);
        let s = app.handle(&ev("running"), 600);
        assert_eq!(s.started_at, Some(500), "持续 running 不应重置计时");
    }

    #[test]
    fn unknown_event_falls_back_to_idle() {
        assert_eq!(AgentStatus::from_event("garbage"), AgentStatus::Idle);
    }

    #[test]
    fn unknown_agent_id_creates_entry() {
        let mut app = AppState::default();
        let mut e = ev("running");
        e.agent_id = Some("codex".into());
        let s = app.handle(&e, 1);
        assert_eq!(s.agent_id, "codex");
        assert_eq!(s.agent_name, "Codex");
    }

    #[test]
    fn default_agent_id_is_claude() {
        assert_eq!(ev("running").agent_id(), "claude");
    }

    #[test]
    fn decision_not_overwritten_by_duplicate_event() {
        let mut app = AppState::default();

        // 第一个事件：提交 decision_id，应为 Pending
        let mut e1 = ev("needs_input");
        e1.decision_id = Some("abc123".into());
        app.handle(&e1, 100);
        assert_eq!(app.decisions.get("abc123"), Some(&Decision::Pending));

        // 用户已批准
        app.submit_decision("abc123", Decision::Allowed);
        assert_eq!(app.decisions.get("abc123"), Some(&Decision::Allowed));

        // 重复/迟到事件：不应把已决定的结果覆盖回 Pending
        let mut e2 = ev("needs_input");
        e2.decision_id = Some("abc123".into());
        app.handle(&e2, 200);
        assert_eq!(
            app.decisions.get("abc123"),
            Some(&Decision::Allowed),
            "已决定的 decision 不应被重复事件覆盖"
        );
    }

    // ===== 全活动监控新测试 =====

    #[test]
    fn pre_tool_use_records_current_tool() {
        let mut app = AppState::default();
        let mut e = ev_with_hook("running", "PreToolUse");
        e.tool_name = Some("Edit".into());
        e.tool_preview = Some("src/main.rs".into());
        let s = app.handle(&e, 100);
        assert_eq!(s.status, AgentStatus::Running);
        assert_eq!(s.current_tool, Some("Edit".into()));
        assert_eq!(s.tool_preview, Some("src/main.rs".into()));
    }

    #[test]
    fn post_tool_use_clears_current_tool() {
        let mut app = AppState::default();
        // 先触发 PreToolUse
        let mut e1 = ev_with_hook("running", "PreToolUse");
        e1.tool_name = Some("Edit".into());
        app.handle(&e1, 100);

        // PostToolUse 清除 current_tool
        let mut e2 = ev_with_hook("running", "PostToolUse");
        e2.response_preview = Some("3 lines changed".into());
        let s = app.handle(&e2, 110);
        assert_eq!(s.current_tool, None);
        assert_eq!(s.message, "done: 3 lines changed");
    }

    #[test]
    fn post_tool_use_failure_sets_error() {
        let mut app = AppState::default();
        let mut e = ev_with_hook("error", "PostToolUseFailure");
        e.error = Some("Permission denied".into());
        let s = app.handle(&e, 100);
        assert_eq!(s.status, AgentStatus::Error);
        assert_eq!(s.error, Some("Permission denied".into()));
    }

    #[test]
    fn preserves_actionable_state() {
        let mut app = AppState::default();

        // 先触发一个写工具的 PreToolUse（hook 映射为 needs_input + decision_id）
        let mut e1 = ev_with_hook("needs_input", "PreToolUse");
        e1.decision_id = Some("dec001".into());
        e1.tool_name = Some("Bash".into());
        let s = app.handle(&e1, 100);
        assert_eq!(s.status, AgentStatus::NeedsInput);
        assert_eq!(s.decision_id, Some("dec001".into()));

        // 后续的 PostToolUse（running，无 decision_id）不应覆盖 needs_input
        // 这是真实的场景：hook 只在 PreToolUse 写工具时生成 decision_id
        let mut e2 = ev_with_hook("running", "PostToolUse");
        e2.tool_name = Some("Read".into());
        let s = app.handle(&e2, 110);
        assert_eq!(
            s.status,
            AgentStatus::NeedsInput,
            "审批等待期间 running 事件不应覆盖 needs_input"
        );
        assert_eq!(s.decision_id, Some("dec001".into()));

        // 新的审批请求（event_type = needs_input）应能覆盖
        let mut e3 = ev_with_hook("needs_input", "PreToolUse");
        e3.decision_id = Some("dec002".into());
        e3.tool_name = Some("Write".into());
        let s = app.handle(&e3, 120);
        assert_eq!(s.status, AgentStatus::NeedsInput);
        assert_eq!(s.decision_id, Some("dec002".into()));
    }

    #[test]
    fn stop_extracts_last_message() {
        let mut app = AppState::default();
        let mut e = ev_with_hook("completed", "Stop");
        e.last_message = Some("All tests passed.".into());
        let s = app.handle(&e, 100);
        assert_eq!(s.status, AgentStatus::Completed);
        assert_eq!(s.last_message, Some("All tests passed.".into()));
    }

    #[test]
    fn subagent_start_updates_task() {
        let mut app = AppState::default();
        let mut e = ev_with_hook("running", "SubagentStart");
        e.agent_type = Some("code-reviewer".into());
        let s = app.handle(&e, 100);
        assert_eq!(s.status, AgentStatus::Running);
        assert_eq!(s.task, "Subagent: code-reviewer");
    }

    #[test]
    fn session_end_clears_everything() {
        let mut app = AppState::default();

        // 先设置一些状态
        let mut e1 = ev_with_hook("running", "PreToolUse");
        e1.tool_name = Some("Edit".into());
        e1.tool_preview = Some("file.rs".into());
        app.handle(&e1, 100);

        // SessionEnd 清空所有
        let s = app.handle(&ev_with_hook("idle", "SessionEnd"), 200);
        assert_eq!(s.status, AgentStatus::Idle);
        assert_eq!(s.started_at, None);
        assert!(s.task.is_empty());
        assert_eq!(s.current_tool, None);
        assert_eq!(s.tool_preview, None);
    }
}
