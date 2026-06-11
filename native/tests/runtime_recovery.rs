mod common;

use std::fs;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use signal_light_native::model::RuntimeCommand;
use signal_light_native::runtime::ipc;
use signal_light_native::runtime::session_store::SessionStore;

#[test]
fn invalid_request_returns_error_response() {
    let _guard = common::env_lock();
    let (_tempdir, config) = common::temp_config();
    common::autostart_runtime(&config);
    let before = ipc::status(&config).unwrap();

    let error = ipc::request(
        &config,
        &RuntimeCommand {
            action: "session_signal".to_string(),
            session_key: None,
            signal_name: Some("working".to_string()),
            owner_pid: None,
            speed: None,
            reply_to: None,
        },
        true,
    )
    .unwrap_err();

    assert!(error.to_string().contains("Missing session key"));
    let after = ipc::status(&config).unwrap();
    assert_eq!(after.runtime_pid, before.runtime_pid);

    common::stop_runtime(&config);
}

#[test]
fn request_without_running_server_and_no_autostart_is_an_error() {
    let _guard = common::env_lock();
    let (_tempdir, config) = common::temp_config();

    let error = ipc::request(
        &config,
        &RuntimeCommand {
            action: "status".to_string(),
            session_key: None,
            signal_name: None,
            owner_pid: None,
            speed: None,
            reply_to: None,
        },
        false,
    )
    .unwrap_err();

    assert!(error.to_string().contains("not running"));
}

#[test]
fn request_autostarts_server_without_preflight_status_probe() {
    let _guard = common::env_lock();
    let (_tempdir, config) = common::temp_config();
    std::env::set_var(
        "SIGNAL_LIGHT_NATIVE_BIN",
        env!("CARGO_BIN_EXE_signal-light-native"),
    );
    std::env::set_var("SIGNAL_LIGHT_SERVER_DRY_RUN", "1");

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
    assert_eq!(response.aggregate.as_deref(), Some("idle"));
    common::stop_runtime(&config);
}

#[test]
fn request_times_out_when_no_server_response_arrives() {
    let _guard = common::env_lock();
    let (_tempdir, mut config) = common::temp_config();
    config.timing.request_timeout_millis = 50;
    config.timing.request_retry_poll_millis = 10;
    ipc::create_request_pipe(&config.state.server_socket_file).unwrap();

    let error = ipc::request_once(
        &config,
        &RuntimeCommand {
            action: "status".to_string(),
            session_key: None,
            signal_name: None,
            owner_pid: None,
            speed: None,
            reply_to: None,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("did not respond in time"));
}

#[test]
fn session_end_notice_restores_idle_without_stale_alert() {
    let _guard = common::env_lock();
    let (_tempdir, config) = common::temp_config();
    common::autostart_runtime(&config);

    ipc::request(
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
    ipc::request(
        &config,
        &RuntimeCommand {
            action: "session_signal".to_string(),
            session_key: Some("turn:demo".to_string()),
            signal_name: Some("session_end".to_string()),
            owner_pid: Some(std::process::id()),
            speed: Some(0.05),
            reply_to: None,
        },
        true,
    )
    .unwrap();

    let immediate = ipc::status(&config).unwrap();
    assert_eq!(immediate.aggregate, "idle");
    assert_eq!(immediate.display_signal, "session_done");
    assert!(immediate.sessions.is_empty());

    thread::sleep(Duration::from_millis(200));
    let settled = ipc::status(&config).unwrap();
    assert_eq!(settled.aggregate, "idle");
    assert_eq!(settled.display_signal, "idle");
    assert!(settled.sessions.is_empty());

    common::stop_runtime(&config);
}

#[test]
fn cleanup_clears_stale_runtime_pid_without_stopping_unrelated_process() {
    let _guard = common::env_lock();
    let (_tempdir, config) = common::temp_config();
    let store = SessionStore::new(config.clone());
    let mut child = Command::new("sleep").arg("30").spawn().unwrap();
    store.update_runtime_pid(Some(child.id())).unwrap();
    fs::write(&config.state.server_socket_file, "").unwrap();
    fs::write(&config.state.server_pid_file, "{}\n").unwrap();

    ipc::cleanup_unreachable_server(&config);

    assert!(child.try_wait().unwrap().is_none());
    assert_eq!(store.read_state().unwrap().runtime_pid, None);
    assert!(!config.state.server_socket_file.exists());
    assert!(!config.state.server_pid_file.exists());

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn status_fallback_clears_stale_runtime_pid() {
    let _guard = common::env_lock();
    let (_tempdir, config) = common::temp_config();
    let store = SessionStore::new(config.clone());
    let mut child = Command::new("sleep").arg("30").spawn().unwrap();
    let stale_pid = child.id();
    child.kill().unwrap();
    child.wait().unwrap();

    store.update_runtime_pid(Some(stale_pid)).unwrap();

    let snapshot = ipc::status(&config).unwrap();

    assert_eq!(snapshot.runtime_pid, None);
    assert_eq!(store.read_state().unwrap().runtime_pid, None);
}

#[test]
fn cleanup_targets_lock_holder_pid_instead_of_stale_runtime_pid() {
    let _guard = common::env_lock();
    let (_tempdir, config) = common::temp_config();
    let store = SessionStore::new(config.clone());
    let mut unrelated = Command::new("sleep").arg("30").spawn().unwrap();
    let mut lock_holder = Command::new("perl")
        .arg("-e")
        .arg(
            "use Fcntl ':flock'; my ($lock, $pid_file) = @ARGV; open my $fh, '>', $lock or die $!; flock($fh, LOCK_EX) or die $!; open my $pf, '>', $pid_file or die $!; print $pf qq({\"pid\": $$}\\n); close $pf; sleep 30;",
        )
        .arg(&config.state.server_lock_file)
        .arg(&config.state.server_pid_file)
        .spawn()
        .unwrap();
    let wait_deadline = Instant::now() + Duration::from_secs(2);
    while !config.state.server_pid_file.exists() {
        assert!(
            Instant::now() < wait_deadline,
            "lock holder did not publish pid file"
        );
        thread::sleep(Duration::from_millis(20));
    }

    store.update_runtime_pid(Some(unrelated.id())).unwrap();
    fs::write(&config.state.server_socket_file, "").unwrap();

    ipc::cleanup_unreachable_server(&config);

    let wait_deadline = Instant::now() + Duration::from_secs(2);
    let lock_holder_status = loop {
        if let Some(status) = lock_holder.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < wait_deadline,
            "cleanup did not stop lock holder"
        );
        thread::sleep(Duration::from_millis(20));
    };
    assert!(!lock_holder_status.success());
    assert!(unrelated.try_wait().unwrap().is_none());
    assert_eq!(store.read_state().unwrap().runtime_pid, None);
    assert!(!config.state.server_socket_file.exists());
    assert!(!config.state.server_pid_file.exists());

    let _ = unrelated.kill();
    let _ = unrelated.wait();
}
