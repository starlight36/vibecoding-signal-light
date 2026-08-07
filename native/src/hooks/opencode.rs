use std::collections::BTreeMap;

use serde_json::Value;

use crate::hooks::{find_failure_marker, find_nested_string, first_string};
use crate::signals;

const EVENT_TO_SIGNAL: &[(&str, &str)] = &[
    ("session.created", "session_start"),
    ("session.idle", signals::TURN_END_SIGNAL),
    ("session.error", "blocked"),
    ("tool.execute.before", "working"),
    ("tool.execute.after", "tool_done"),
    ("permission.asked", "permission"),
    ("command.executed", "working"),
];

#[derive(Debug, Clone, PartialEq)]
pub struct OpenCodeHookInput {
    pub event_name: String,
    pub payload: Value,
}

pub fn read_input(argv: &[String], stdin_text: &str) -> OpenCodeHookInput {
    let payload = crate::hooks::parse_json_or_raw(stdin_text);
    let event_name = first_string(&payload, &["hook_event_name", "event", "type"])
        .map(str::to_string)
        .or_else(|| event_from_args(argv));
    OpenCodeHookInput {
        event_name: event_name.unwrap_or_else(|| "session.idle".to_string()),
        payload,
    }
}

pub fn choose_signal(input: &OpenCodeHookInput) -> String {
    if let Some(explicit) = first_string(&input.payload, &["signal", "signal_name", "lamp_signal"])
    {
        let normalized = explicit.to_ascii_lowercase();
        if signals::is_public_signal(&normalized) || signals::is_hook_control_signal(&normalized) {
            return normalized;
        }
    }

    if let Some(status) = first_string(&input.payload, &["status", "state"]) {
        let normalized = status.to_ascii_lowercase();
        if signals::is_public_signal(&normalized) {
            return normalized;
        }
        if matches!(
            normalized.as_str(),
            "error" | "failed" | "failure" | "exception"
        ) {
            return "blocked".to_string();
        }
    }

    if input.event_name == "session.status" {
        if let Some(status_type) = find_nested_string(&input.payload, &["status", "type"]) {
            return match status_type.as_str() {
                "idle" => signals::TURN_END_SIGNAL.to_string(),
                "busy" => "working".to_string(),
                "retry" => "blocked".to_string(),
                _ => "attention".to_string(),
            };
        }
    }

    if find_failure_marker(&input.payload).is_some() {
        return "blocked".to_string();
    }

    if input.event_name == "tool.execute.after" {
        for key in ["output", "result"] {
            if let Some(text) = first_string(&input.payload, &[key]) {
                let normalized = text.trim().to_ascii_lowercase();
                if normalized.contains("error")
                    || normalized.contains("failed")
                    || normalized.contains("exception")
                {
                    return "blocked".to_string();
                }
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

pub fn session_key(input: &OpenCodeHookInput, env: &BTreeMap<String, String>) -> String {
    if let Some(session_id) = first_string(&input.payload, &["session_id", "sessionID"]) {
        return session_id.to_string();
    }
    if let Some(session_id) = find_nested_string(&input.payload, &["session_id", "sessionID"]) {
        return session_id;
    }
    for key in ["OPENCODE_SESSION_ID", "OPC_SESSION_ID"] {
        if let Some(value) = env
            .get(key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return value.to_string();
        }
    }
    if let Some(cwd) = first_string(&input.payload, &["cwd", "directory", "worktree"]) {
        return format!("cwd:{cwd}");
    }
    if let Some(cwd) = find_nested_string(&input.payload, &["cwd", "directory", "worktree"]) {
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
