mod common;

use std::fs;

use signal_light_native::model::RuntimeCommand;
use signal_light_native::runtime::ipc;

#[test]
fn status_returns_snapshot_after_autostart() {
    let _guard = common::env_lock();
    let (_tempdir, config) = common::temp_config();
    common::autostart_runtime(&config);

    let snapshot = ipc::status(&config).unwrap();

    assert_eq!(snapshot.aggregate, "idle");
    assert_eq!(snapshot.display_signal, "idle");
    assert!(snapshot.sessions.is_empty());

    common::stop_runtime(&config);
}

#[test]
fn session_signal_updates_aggregate_and_session_records() {
    let _guard = common::env_lock();
    let (_tempdir, config) = common::temp_config();
    common::autostart_runtime(&config);

    let response = ipc::request(
        &config,
        &RuntimeCommand {
            action: "session_signal".to_string(),
            session_key: Some("turn:demo".to_string()),
            signal_name: Some("working".to_string()),
            owner_pid: Some(std::process::id()),
            speed: Some(0.05),
            reply_to: None,
        },
        true,
    )
    .unwrap();

    assert_eq!(response.aggregate.as_deref(), Some("working"));
    assert_eq!(response.display_signal.as_deref(), Some("working"));

    let snapshot = ipc::status(&config).unwrap();
    assert_eq!(snapshot.aggregate, "working");
    assert_eq!(snapshot.display_signal, "working");
    assert_eq!(snapshot.sessions["turn:demo"].signal, "working");
    assert_eq!(
        snapshot.sessions["turn:demo"].owner_pid,
        Some(std::process::id())
    );

    common::stop_runtime(&config);
}

#[test]
fn response_write_is_atomic_without_temp_file_leak() {
    let _guard = common::env_lock();
    let (_tempdir, config) = common::temp_config();
    common::autostart_runtime(&config);

    let response = ipc::request(
        &config,
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
    let leftovers = fs::read_dir(&config.state.root)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.contains(".response.json."))
        .collect::<Vec<_>>();
    assert!(
        leftovers.is_empty(),
        "leftover temp response files: {leftovers:?}"
    );

    common::stop_runtime(&config);
}
