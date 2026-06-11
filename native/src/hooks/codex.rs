use std::collections::BTreeMap;

use serde_json::Value;

use crate::hooks::{find_failure_marker, find_nested_string, first_string};
use crate::signals;

const EVENT_TO_SIGNAL: &[(&str, &str)] = &[
    ("SessionStart", "session_start"),
    ("UserPromptSubmit", "thinking"),
    ("PreToolUse", "working"),
    ("PostToolUse", "tool_done"),
    ("PermissionRequest", "permission"),
    ("Stop", signals::TURN_END_SIGNAL),
    ("SessionEnd", "session_end"),
];

#[derive(Debug, Clone, PartialEq)]
pub struct CodexHookInput {
    pub event_name: String,
    pub payload: Value,
}

pub fn read_input(
    argv: &[String],
    stdin_text: &str,
    env: &BTreeMap<String, String>,
) -> CodexHookInput {
    let mut event_name = event_from_args(argv);
    let payload = crate::hooks::parse_json_or_raw(stdin_text);
    if event_name.is_none() {
        event_name = event_from_payload(&payload);
    }
    let resolved = event_name
        .or_else(|| env.get("CODEX_HOOK_EVENT").cloned())
        .or_else(|| env.get("HOOK_EVENT").cloned())
        .unwrap_or_else(|| "Stop".to_string());
    CodexHookInput {
        event_name: resolved,
        payload,
    }
}

pub fn choose_signal(input: &CodexHookInput) -> String {
    if let Some(explicit) = first_string(&input.payload, &["signal", "signal_name", "lamp_signal"])
    {
        let normalized = explicit.to_ascii_lowercase();
        if signals::is_public_signal(&normalized) {
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

    if find_failure_marker(&input.payload).is_some() {
        return "blocked".to_string();
    }

    EVENT_TO_SIGNAL
        .iter()
        .find_map(|(event_name, signal_name)| {
            (*event_name == input.event_name).then_some(*signal_name)
        })
        .unwrap_or("attention")
        .to_string()
}

pub fn session_key(input: &CodexHookInput, env: &BTreeMap<String, String>) -> String {
    if let Some(turn_id) = first_string(&input.payload, &["turn_id", "request_id"]) {
        return format!("turn:{turn_id}");
    }
    if let Some(turn_id) = find_nested_string(&input.payload, &["turn_id", "request_id"]) {
        return format!("turn:{turn_id}");
    }
    for key in ["CODEX_TURN_ID", "CODEX_REQUEST_ID"] {
        if let Some(value) = env
            .get(key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return format!("turn:{value}");
        }
    }
    if let Some(session_id) = first_string(
        &input.payload,
        &[
            "session_id",
            "conversation_id",
            "thread_id",
            "chat_id",
            "codex_session_id",
        ],
    ) {
        return session_id.to_string();
    }
    if let Some(session_id) = find_nested_string(
        &input.payload,
        &[
            "session_id",
            "conversation_id",
            "thread_id",
            "codex_session_id",
        ],
    ) {
        return session_id;
    }
    for key in [
        "CODEX_SESSION_ID",
        "CODEX_CONVERSATION_ID",
        "CODEX_THREAD_ID",
    ] {
        if let Some(value) = env
            .get(key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return value.to_string();
        }
    }
    if let Some(cwd) = first_string(
        &input.payload,
        &["cwd", "workspace", "workspace_dir", "project_dir"],
    ) {
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

fn event_from_payload(payload: &Value) -> Option<String> {
    first_string(
        payload,
        &["hook_event_name", "event_name", "event", "hook", "type"],
    )
    .map(str::to_string)
}
