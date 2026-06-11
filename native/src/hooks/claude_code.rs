use std::collections::BTreeMap;

use serde_json::Value;

use crate::hooks::first_string;
use crate::signals;

const EVENT_TO_SIGNAL: &[(&str, &str)] = &[
    ("SessionStart", "session_start"),
    ("UserPromptSubmit", "thinking"),
    ("PreToolUse", "working"),
    ("PostToolUse", "tool_done"),
    ("PostToolUseFailure", "blocked"),
    ("PreCompact", "working"),
    ("SubagentStart", "working"),
    ("SubagentStop", "tool_done"),
    ("Stop", signals::TURN_END_SIGNAL),
    ("Notification", "attention"),
    ("PermissionRequest", "permission"),
    ("SessionEnd", "session_end"),
];

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCodeHookInput {
    pub event_name: String,
    pub payload: Value,
}

pub fn read_input(argv: &[String], stdin_text: &str) -> ClaudeCodeHookInput {
    let mut event_name = event_from_args(argv);
    let payload = crate::hooks::parse_json_or_raw(stdin_text);
    if event_name.is_none() {
        event_name = first_string(&payload, &["event", "hook_event_name"]).map(str::to_string);
    }
    ClaudeCodeHookInput {
        event_name: event_name.unwrap_or_else(|| "Stop".to_string()),
        payload,
    }
}

pub fn choose_signal(input: &ClaudeCodeHookInput) -> String {
    if let Some(explicit) = first_string(&input.payload, &["signal", "signal_name"]) {
        let normalized = explicit.to_ascii_lowercase();
        if signals::is_public_signal(&normalized) {
            return normalized;
        }
    }
    if input.event_name == "Stop" {
        if let Some(stop_reason) = first_string(&input.payload, &["stop_reason"]) {
            if matches!(stop_reason, "max_tokens" | "error") {
                return "blocked".to_string();
            }
        }
    }
    EVENT_TO_SIGNAL
        .iter()
        .find_map(|(event_name, signal_name)| {
            (*event_name == input.event_name).then_some(*signal_name)
        })
        .unwrap_or("attention")
        .to_string()
}

pub fn session_key(input: &ClaudeCodeHookInput, env: &BTreeMap<String, String>) -> String {
    if let Some(session_id) = first_string(&input.payload, &["session_id"]) {
        return session_id.to_string();
    }
    for key in ["CLAUDE_CODE_SESSION_ID", "CLAUDE_SESSION_ID"] {
        if let Some(value) = env
            .get(key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return value.to_string();
        }
    }
    if let Some(cwd) = first_string(&input.payload, &["cwd"]) {
        return format!("cwd:{cwd}");
    }
    "global".to_string()
}

fn event_from_args(argv: &[String]) -> Option<String> {
    for (index, value) in argv.iter().enumerate() {
        if matches!(value.as_str(), "--event" | "-e") && index + 1 < argv.len() {
            return Some(argv[index + 1].clone());
        }
        if let Some(rest) = value.strip_prefix("--event=") {
            return Some(rest.to_string());
        }
    }
    argv.get(1).filter(|value| !value.starts_with('-')).cloned()
}
