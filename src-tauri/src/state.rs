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
        }
    }

    /// 应用一个事件，推进状态机。`now` 为当前 unix 秒（便于测试注入）。
    pub fn apply(&mut self, ev: &IncomingEvent, now: i64) {
        let status = AgentStatus::from_event(&ev.event_type);
        if let Some(name) = &ev.agent_name {
            if !name.is_empty() {
                self.agent_name = name.clone();
            }
        }
        if let Some(task) = &ev.task {
            self.task = task.clone();
        }
        if let Some(msg) = &ev.message {
            self.message = msg.clone();
        }
        match status {
            // 进入 Running 时（且此前未在计时）记录起点。
            AgentStatus::Running => {
                if self.status != AgentStatus::Running || self.started_at.is_none() {
                    self.started_at = Some(now);
                }
            }
            // 回到空闲清空任务与计时。
            AgentStatus::Idle => {
                self.started_at = None;
                self.task.clear();
                self.decision_id = None;
            }
            // 完成/错误/需输入保留计时起点，前端可继续显示用时。
            _ => {}
        }
        self.status = status;
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
        }
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
}
