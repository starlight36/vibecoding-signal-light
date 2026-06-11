#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use serde_json::json;
use tempfile::TempDir;

use signal_light_native::config::RuntimeConfig;
use signal_light_native::model::RuntimeCommand;
use signal_light_native::model::{RuntimeSnapshot, SessionRecord};
use signal_light_native::runtime::ipc;

pub fn sample_runtime_snapshot() -> RuntimeSnapshot {
    let mut sessions = BTreeMap::new();
    sessions.insert(
        "turn:demo".to_string(),
        SessionRecord {
            signal: "working".to_string(),
            updated_at: 1_700_000_000.0,
            owner_pid: Some(4321),
            owner_pid_source: Some("explicit".to_string()),
        },
    );
    RuntimeSnapshot {
        aggregate: "working".to_string(),
        display_signal: "working".to_string(),
        sessions,
        runtime_pid: Some(8765),
        updated_at: Some(1_700_000_001.0),
    }
}

pub fn codex_permission_payload() -> serde_json::Value {
    json!({
        "event": "PermissionRequest",
        "session_id": "codex-session",
        "turn_id": "turn-42",
        "owner_pid": 4321
    })
}

pub fn claude_notification_payload() -> serde_json::Value {
    json!({
        "event": "Notification",
        "session_id": "claude-session",
        "cwd": "/tmp/project"
    })
}

pub fn env_lock() -> MutexGuard<'static, ()> {
    static ENV_LOCK: Mutex<()> = Mutex::new(());
    ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
}

pub fn temp_config() -> (TempDir, RuntimeConfig) {
    let tempdir = tempfile::tempdir().unwrap();
    let config = RuntimeConfig::from_pairs([(
        "SIGNAL_LIGHT_STATE_DIR".to_string(),
        tempdir.path().to_string_lossy().to_string(),
    )])
    .unwrap();
    (tempdir, config)
}

pub fn autostart_runtime(config: &RuntimeConfig) {
    std::env::set_var(
        "SIGNAL_LIGHT_NATIVE_BIN",
        env!("CARGO_BIN_EXE_signal-light-native"),
    );
    std::env::set_var("SIGNAL_LIGHT_SERVER_DRY_RUN", "1");
    let response = ipc::request(
        config,
        &RuntimeCommand {
            action: "status".to_string(),
            session_key: None,
            signal_name: None,
            owner_pid: None,
            speed: None,
            reply_to: None,
        },
        true,
    )
    .unwrap();
    assert!(response.ok);
}

pub fn stop_runtime(config: &RuntimeConfig) {
    ipc::cleanup_unreachable_server(config);
    std::env::remove_var("SIGNAL_LIGHT_SERVER_DRY_RUN");
    std::env::remove_var("SIGNAL_LIGHT_NATIVE_BIN");
}

pub fn native_binary() -> &'static str {
    env!("CARGO_BIN_EXE_signal-light-native")
}
