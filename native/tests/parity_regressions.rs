use serde_json::Value;

use signal_light_native::hooks::{claude_code, codex};
use signal_light_native::model::{RuntimeSnapshot, SessionRecord};
use signal_light_native::signals;

#[test]
fn native_signal_names_match_documented_contract() {
    let names = signals::definitions()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "attention",
            "blocked",
            "done",
            "idle",
            "off",
            "permission",
            "session_done",
            "session_end",
            "session_start",
            "thinking",
            "tool_done",
            "working",
        ]
    );
}

#[test]
fn hook_mappings_preserve_existing_semantics() {
    let codex_input = codex::CodexHookInput {
        event_name: "PostToolUse".to_string(),
        payload: serde_json::json!({}),
    };
    let claude_input = claude_code::ClaudeCodeHookInput {
        event_name: "Notification".to_string(),
        payload: serde_json::json!({"session_id": "claude-session"}),
    };

    assert_eq!(codex::choose_signal(&codex_input), "tool_done");
    assert_eq!(claude_code::choose_signal(&claude_input), "attention");
}

#[test]
fn status_output_serialization_keeps_required_fields() {
    let snapshot = RuntimeSnapshot {
        aggregate: "working".to_string(),
        display_signal: "working".to_string(),
        sessions: [(
            "turn:demo".to_string(),
            SessionRecord {
                signal: "working".to_string(),
                updated_at: 123.0,
                owner_pid: Some(99),
                owner_pid_source: Some("explicit".to_string()),
            },
        )]
        .into_iter()
        .collect(),
        runtime_pid: Some(10),
        updated_at: Some(124.0),
    };

    let payload = serde_json::to_value(snapshot).unwrap();
    assert!(matches!(payload, Value::Object(_)));
    assert_eq!(payload["aggregate"], "working");
    assert_eq!(payload["display_signal"], "working");
    assert!(payload["sessions"].is_object());
}
