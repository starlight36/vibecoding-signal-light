use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::model::{AttentionLevel, ChannelState, Frame, SignalDefinition};

pub const TURN_END_SIGNAL: &str = "turn_end";
pub const SESSION_END_NOTICE_SIGNAL: &str = "session_done";

pub fn definitions() -> &'static BTreeMap<String, SignalDefinition> {
    static SIGNALS: OnceLock<BTreeMap<String, SignalDefinition>> = OnceLock::new();
    SIGNALS.get_or_init(build_definitions)
}

pub fn signal(name: &str) -> Option<&'static SignalDefinition> {
    definitions().get(name)
}

pub fn is_public_signal(name: &str) -> bool {
    definitions().contains_key(name)
}

pub fn is_hook_control_signal(name: &str) -> bool {
    matches!(name, TURN_END_SIGNAL | "session_end")
}

pub fn aggregate_signals<'a>(signals: impl IntoIterator<Item = &'a str>) -> &'static str {
    let values = signals.into_iter().collect::<Vec<_>>();
    if values.contains(&"blocked") {
        return "blocked";
    }
    if values.contains(&"permission") {
        return "permission";
    }
    if values
        .iter()
        .any(|signal_name| matches!(*signal_name, "attention" | "done"))
    {
        return "attention";
    }
    if values
        .iter()
        .any(|signal_name| matches!(*signal_name, "thinking" | "working" | "tool_done"))
    {
        return "working";
    }
    "idle"
}

pub fn build_definitions() -> BTreeMap<String, SignalDefinition> {
    let mut map = BTreeMap::new();

    map.insert(
        "idle".to_string(),
        SignalDefinition {
            name: "idle",
            summary: "Agent 空闲。",
            attention: "不需要关注。",
            attention_level: AttentionLevel::Idle,
            frames: vec![],
            loops: 1,
            steady_state: Some(ChannelState::new(true, false, false)),
            repeat: false,
        },
    );
    map.insert(
        "thinking".to_string(),
        SignalDefinition {
            name: "thinking",
            summary: "Agent 已收到任务，正在思考或工作。",
            attention: "不用处理。",
            attention_level: AttentionLevel::Working,
            frames: work_cycle(),
            loops: 1,
            steady_state: None,
            repeat: true,
        },
    );
    map.insert(
        "working".to_string(),
        SignalDefinition {
            name: "working",
            summary: "Agent 正在执行工具、读写文件、跑命令或测试。",
            attention: "不用处理。",
            attention_level: AttentionLevel::Working,
            frames: work_cycle(),
            loops: 1,
            steady_state: None,
            repeat: true,
        },
    );
    map.insert(
        "tool_done".to_string(),
        SignalDefinition {
            name: "tool_done",
            summary: "一次工具调用完成，Agent 仍处于工作流中。",
            attention: "不用处理。",
            attention_level: AttentionLevel::Working,
            frames: work_cycle(),
            loops: 1,
            steady_state: None,
            repeat: true,
        },
    );
    map.insert(
        "attention".to_string(),
        SignalDefinition {
            name: "attention",
            summary: "Agent 停下来等你读结果或继续回复。",
            attention: "需要你看一眼 Codex。",
            attention_level: AttentionLevel::Attention,
            frames: flash(false, true, false),
            loops: 8,
            steady_state: None,
            repeat: true,
        },
    );
    map.insert(
        "permission".to_string(),
        SignalDefinition {
            name: "permission",
            summary: "Codex 请求授权或需要你明确批准。",
            attention: "需要立即关注。",
            attention_level: AttentionLevel::Permission,
            frames: flash(false, true, false),
            loops: 12,
            steady_state: None,
            repeat: true,
        },
    );
    map.insert(
        "blocked".to_string(),
        SignalDefinition {
            name: "blocked",
            summary: "Agent 遇到阻塞、失败或无法继续。",
            attention: "需要你处理。",
            attention_level: AttentionLevel::Blocked,
            frames: flash(false, false, true),
            loops: 12,
            steady_state: None,
            repeat: true,
        },
    );
    map.insert(
        "done".to_string(),
        SignalDefinition {
            name: "done",
            summary: "任务已完成。",
            attention: "建议查看最终答复。",
            attention_level: AttentionLevel::Attention,
            frames: flash(false, true, false),
            loops: 8,
            steady_state: None,
            repeat: true,
        },
    );
    map.insert(
        "session_start".to_string(),
        SignalDefinition {
            name: "session_start",
            summary: "Codex 会话开始。",
            attention: "不用处理。",
            attention_level: AttentionLevel::Idle,
            frames: vec![],
            loops: 1,
            steady_state: Some(ChannelState::new(true, false, false)),
            repeat: false,
        },
    );
    map.insert(
        "session_end".to_string(),
        SignalDefinition {
            name: "session_end",
            summary: "Codex 会话结束，回到当前聚合状态。",
            attention: "不需要关注。",
            attention_level: AttentionLevel::Completion,
            frames: vec![],
            loops: 1,
            steady_state: Some(ChannelState::new(true, false, false)),
            repeat: false,
        },
    );
    map.insert(
        "session_done".to_string(),
        SignalDefinition {
            name: "session_done",
            summary: "一个 Agent 会话结束。",
            attention: "不用处理。",
            attention_level: AttentionLevel::Completion,
            frames: notice_flash(true, false, false),
            loops: 6,
            steady_state: None,
            repeat: false,
        },
    );
    map.insert(
        "off".to_string(),
        SignalDefinition {
            name: "off",
            summary: "关闭所有灯。",
            attention: "不需要关注。",
            attention_level: AttentionLevel::Idle,
            frames: vec![Frame::solid(false, false, false, 10)],
            loops: 1,
            steady_state: Some(ChannelState::new(false, false, false)),
            repeat: false,
        },
    );

    map
}

fn flash(green_on: bool, yellow_on: bool, red_on: bool) -> Vec<Frame> {
    vec![
        Frame::solid(green_on, yellow_on, red_on, 120),
        Frame::solid(false, false, false, 100),
    ]
}

fn notice_flash(green_on: bool, yellow_on: bool, red_on: bool) -> Vec<Frame> {
    vec![
        Frame::solid(green_on, yellow_on, red_on, 180),
        Frame::solid(false, false, false, 140),
    ]
}

fn soft_pulse(green_on: bool, yellow_on: bool, red_on: bool) -> Vec<Frame> {
    [0.10_f32, 0.18, 0.32, 0.50, 0.68, 0.50, 0.32, 0.18, 0.10]
        .into_iter()
        .map(|brightness| Frame::with_brightness(green_on, yellow_on, red_on, 160, brightness))
        .collect()
}

fn work_cycle() -> Vec<Frame> {
    let mut frames = Vec::new();
    frames.extend(soft_pulse(true, false, false));
    frames.extend(soft_pulse(false, true, false));
    frames.extend(soft_pulse(false, false, true));
    frames
}
