mod common;

use std::collections::BTreeMap;

use signal_light_native::hooks::{claude_code, codex, opencode};
use signal_light_native::signals;

#[test]
fn codex_stop_maps_to_turn_end_control_signal() {
    let input = codex::CodexHookInput {
        event_name: "Stop".to_string(),
        payload: serde_json::json!({}),
    };
    assert_eq!(codex::choose_signal(&input), signals::TURN_END_SIGNAL);
}

#[test]
fn codex_failure_payload_maps_to_blocked() {
    let input = codex::CodexHookInput {
        event_name: "PostToolUse".to_string(),
        payload: serde_json::json!({"status": "failed"}),
    };
    assert_eq!(codex::choose_signal(&input), "blocked");
}

#[test]
fn codex_transient_error_message_does_not_force_blocked() {
    let input = codex::CodexHookInput {
        event_name: "PostToolUse".to_string(),
        payload: serde_json::json!({
            "error_message": "retrying after transient error",
            "session_id": "codex-session"
        }),
    };
    assert_eq!(codex::choose_signal(&input), "tool_done");
}

#[test]
fn codex_session_key_prefers_turn_id_over_session_and_cwd() {
    let key = codex::session_key(
        &codex::CodexHookInput {
            event_name: "Stop".to_string(),
            payload: serde_json::json!({"session_id": "session-a", "turn_id": "turn-a", "cwd": "/tmp/project"}),
        },
        &BTreeMap::new(),
    );
    assert_eq!(key, "turn:turn-a");
}

#[test]
fn codex_session_key_uses_env_turn_id_before_cwd() {
    let env = BTreeMap::from([("CODEX_TURN_ID".to_string(), "turn-env".to_string())]);
    let key = codex::session_key(
        &codex::CodexHookInput {
            event_name: "PreToolUse".to_string(),
            payload: serde_json::json!({"cwd": "/tmp/project"}),
        },
        &env,
    );
    assert_eq!(key, "turn:turn-env");
}

#[test]
fn claude_notification_maps_to_attention() {
    let input = claude_code::ClaudeCodeHookInput {
        event_name: "Notification".to_string(),
        payload: common::claude_notification_payload(),
    };
    assert_eq!(claude_code::choose_signal(&input), "attention");
}

#[test]
fn claude_stop_with_error_reason_stays_blocked() {
    let input = claude_code::ClaudeCodeHookInput {
        event_name: "Stop".to_string(),
        payload: serde_json::json!({"stop_reason": "error"}),
    };
    assert_eq!(claude_code::choose_signal(&input), "blocked");
}

#[test]
fn claude_session_key_falls_back_to_env_then_cwd_then_global() {
    let env = BTreeMap::from([(
        "CLAUDE_CODE_SESSION_ID".to_string(),
        "claude-env-session".to_string(),
    )]);
    let key = claude_code::session_key(
        &claude_code::ClaudeCodeHookInput {
            event_name: "Stop".to_string(),
            payload: serde_json::json!({"cwd": "/tmp/project"}),
        },
        &env,
    );
    assert_eq!(key, "claude-env-session");
}

#[test]
fn opencode_session_created_maps_to_session_start() {
    let input = opencode::OpenCodeHookInput {
        event_name: "session.created".to_string(),
        payload: serde_json::json!({"session_id": "opc-session-1"}),
    };
    assert_eq!(opencode::choose_signal(&input), "session_start");
}

#[test]
fn opencode_session_idle_maps_to_turn_end() {
    let input = opencode::OpenCodeHookInput {
        event_name: "session.idle".to_string(),
        payload: serde_json::json!({}),
    };
    assert_eq!(opencode::choose_signal(&input), signals::TURN_END_SIGNAL);
}

#[test]
fn opencode_session_error_maps_to_blocked() {
    let input = opencode::OpenCodeHookInput {
        event_name: "session.error".to_string(),
        payload: serde_json::json!({"error": "connection failed"}),
    };
    assert_eq!(opencode::choose_signal(&input), "blocked");
}

#[test]
fn opencode_tool_error_output_maps_to_blocked() {
    let input = opencode::OpenCodeHookInput {
        event_name: "tool.execute.after".to_string(),
        payload: serde_json::json!({"output": "error: command not found"}),
    };
    assert_eq!(opencode::choose_signal(&input), "blocked");
}

#[test]
fn opencode_session_key_prefers_session_id_then_env_then_cwd() {
    let key = opencode::session_key(
        &opencode::OpenCodeHookInput {
            event_name: "session.created".to_string(),
            payload: serde_json::json!({"session_id": "opc-123", "cwd": "/tmp/project"}),
        },
        &BTreeMap::from([(
            "OPENCODE_SESSION_ID".to_string(),
            "opc-env-session".to_string(),
        )]),
    );
    assert_eq!(key, "opc-123");
}

#[test]
fn opencode_session_status_busy_maps_to_working() {
    let input = opencode::OpenCodeHookInput {
        event_name: "session.status".to_string(),
        payload: serde_json::json!({"session_id": "opc-s1", "status": {"type": "busy"}}),
    };
    assert_eq!(opencode::choose_signal(&input), "working");
}

#[test]
fn opencode_session_status_idle_maps_to_turn_end() {
    let input = opencode::OpenCodeHookInput {
        event_name: "session.status".to_string(),
        payload: serde_json::json!({"status": {"type": "idle"}}),
    };
    assert_eq!(opencode::choose_signal(&input), signals::TURN_END_SIGNAL);
}

#[test]
fn opencode_session_status_retry_maps_to_blocked() {
    let input = opencode::OpenCodeHookInput {
        event_name: "session.status".to_string(),
        payload: serde_json::json!({"status": {"type": "retry", "attempt": 2}}),
    };
    assert_eq!(opencode::choose_signal(&input), "blocked");
}

#[test]
fn opencode_explicit_signal_overrides_session_status() {
    let input = opencode::OpenCodeHookInput {
        event_name: "session.status".to_string(),
        payload: serde_json::json!({"signal": "turn_end", "status": {"type": "busy"}}),
    };
    assert_eq!(opencode::choose_signal(&input), signals::TURN_END_SIGNAL);
}

#[test]
fn opencode_read_input_prefers_payload_hook_event_name_over_positional_arg() {
    let input = opencode::read_input(
        &[
            "signal-light-native".to_string(),
            "session.error".to_string(),
        ],
        r#"{"hook_event_name":"session.created","session_id":"opc-1"}"#,
    );
    assert_eq!(input.event_name, "session.created");
}

#[test]
fn opencode_read_input_uses_positional_arg_when_payload_has_no_event() {
    let input = opencode::read_input(
        &[
            "signal-light-native".to_string(),
            "session.error".to_string(),
        ],
        r#"{"session_id":"opc-1"}"#,
    );
    assert_eq!(input.event_name, "session.error");
}

#[test]
fn opencode_session_key_falls_back_to_global() {
    let key = opencode::session_key(
        &opencode::OpenCodeHookInput {
            event_name: "session.status".to_string(),
            payload: serde_json::json!({}),
        },
        &BTreeMap::new(),
    );
    assert_eq!(key, "global");
}
