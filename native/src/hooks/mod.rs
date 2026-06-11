pub mod claude_code;
pub mod codex;

use serde_json::Value;

pub fn parse_json_or_raw(stdin_text: &str) -> Value {
    if stdin_text.trim().is_empty() {
        return Value::Object(Default::default());
    }
    serde_json::from_str(stdin_text).unwrap_or_else(|_| {
        let mut payload = serde_json::Map::new();
        payload.insert("raw".to_string(), Value::String(stdin_text.to_string()));
        Value::Object(payload)
    })
}

pub fn owner_pid_from_payload_or_env(
    payload: &Value,
    env: &std::collections::BTreeMap<String, String>,
) -> Option<u32> {
    for key in ["owner_pid", "session_pid", "agent_pid", "process_pid"] {
        if let Some(pid) = coerce_pid(payload.get(key)) {
            return Some(pid);
        }
    }
    for key in [
        "SIGNAL_LIGHT_OWNER_PID",
        "CODEX_OWNER_PID",
        "CLAUDE_CODE_OWNER_PID",
        "CLAUDE_OWNER_PID",
    ] {
        if let Some(pid) = env
            .get(key)
            .and_then(|value| coerce_pid(Some(&Value::String(value.clone()))))
        {
            return Some(pid);
        }
    }
    None
}

pub fn first_string<'a>(payload: &'a Value, keys: &[&str]) -> Option<&'a str> {
    let Value::Object(map) = payload else {
        return None;
    };
    keys.iter()
        .find_map(|key| map.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub fn find_nested_string(value: &Value, keys: &[&str]) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(found) = keys
                .iter()
                .find_map(|key| map.get(*key).and_then(Value::as_str))
                .map(str::trim)
                .filter(|candidate| !candidate.is_empty())
            {
                return Some(found.to_string());
            }
            map.values()
                .find_map(|child| find_nested_string(child, keys))
        }
        Value::Array(items) => items
            .iter()
            .find_map(|child| find_nested_string(child, keys)),
        _ => None,
    }
}

pub fn find_failure_marker(value: &Value) -> Option<&'static str> {
    const FAILURE_KEYS: &[&str] = &[
        "error",
        "failure",
        "exception",
        "error_type",
        "error_message",
        "failure_reason",
        "exit_status",
        "tool_error",
    ];
    const FAILURE_MARKERS: &[(&str, &str)] = &[
        ("error", "blocked"),
        ("failed", "blocked"),
        ("failure", "blocked"),
        ("exception", "blocked"),
    ];

    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let normalized = key.trim().to_ascii_lowercase();
                if FAILURE_KEYS.contains(&normalized.as_str())
                    || FAILURE_MARKERS
                        .iter()
                        .any(|(marker, _)| *marker == normalized)
                {
                    if let Some(marker) = failure_marker_from_value(child) {
                        return Some(marker);
                    }
                }
                if let Some(marker) = find_failure_marker(child) {
                    return Some(marker);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(find_failure_marker),
        _ => None,
    }
}

fn failure_marker_from_value(value: &Value) -> Option<&'static str> {
    match value {
        Value::Bool(true) => Some("error"),
        Value::Number(number) => {
            if number.as_i64().unwrap_or_default() != 0 {
                Some("failed")
            } else {
                None
            }
        }
        Value::String(text) => {
            let normalized = text.trim().to_ascii_lowercase();
            if normalized.is_empty()
                || matches!(
                    normalized.as_str(),
                    "0" | "false" | "no" | "none" | "null" | "success" | "ok"
                )
            {
                return None;
            }
            if normalized.contains("failed") {
                return Some("failed");
            }
            if normalized.contains("failure") {
                return Some("failure");
            }
            if normalized.contains("exception") {
                return Some("exception");
            }
            if matches!(
                normalized.as_str(),
                "error" | "errored" | "fatal" | "timed out" | "timeout" | "denied" | "blocked"
            ) {
                return Some("error");
            }
            None
        }
        Value::Null | Value::Bool(false) => None,
        _ => Some("error"),
    }
}

fn coerce_pid(value: Option<&Value>) -> Option<u32> {
    match value {
        Some(Value::Number(number)) => number
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0),
        Some(Value::String(text)) => text.trim().parse::<u32>().ok().filter(|value| *value > 0),
        _ => None,
    }
}
