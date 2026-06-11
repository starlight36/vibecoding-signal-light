mod common;

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use signal_light_native::runtime::ipc;

fn base_command(state_dir: &str) -> Command {
    let mut command = Command::new(common::native_binary());
    command
        .env("SIGNAL_LIGHT_STATE_DIR", state_dir)
        .env("SIGNAL_LIGHT_SERVER_DRY_RUN", "1");
    command
}

fn script_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("scripts")
        .join(name)
}

fn wait_with_output_timeout(mut child: Child, timeout: Duration) -> Output {
    let _ = child.stdin.take();
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child did not exit within {:?}", timeout);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn play_starts_runtime_within_startup_budget_and_status_matches_contract() {
    let _guard = common::env_lock();
    let (_tempdir, config) = common::temp_config();
    let state_dir = config.state.root.to_string_lossy().to_string();

    let started = Instant::now();
    let output = base_command(&state_dir)
        .arg("play")
        .arg("working")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(started.elapsed() < Duration::from_secs(3));

    let status_output = base_command(&state_dir).arg("status").output().unwrap();
    assert!(status_output.status.success());
    let payload = serde_json::from_slice::<Value>(&status_output.stdout).unwrap();
    assert_eq!(payload["aggregate"], "idle");
    assert_eq!(payload["display_signal"], "working");
    assert!(payload["sessions"].as_object().unwrap().is_empty());

    ipc::cleanup_unreachable_server(&config);
}

#[test]
fn warm_codex_hook_updates_status_quickly() {
    let _guard = common::env_lock();
    let (_tempdir, config) = common::temp_config();
    let state_dir = config.state.root.to_string_lossy().to_string();

    let warmup = base_command(&state_dir)
        .arg("play")
        .arg("working")
        .output()
        .unwrap();
    assert!(warmup.status.success());

    let started = Instant::now();
    let mut child = base_command(&state_dir)
        .arg("codex-hook")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(br#"{"event":"PermissionRequest","session_id":"warm-session"}"#)
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(started.elapsed() < Duration::from_millis(250));

    let status_output = base_command(&state_dir).arg("status").output().unwrap();
    let payload = serde_json::from_slice::<Value>(&status_output.stdout).unwrap();
    assert_eq!(payload["aggregate"], "permission");
    assert_eq!(payload["display_signal"], "permission");
    assert_eq!(payload["sessions"]["warm-session"]["signal"], "permission");

    ipc::cleanup_unreachable_server(&config);
}

#[test]
fn codex_hook_honors_signal_light_dry_run_env() {
    let _guard = common::env_lock();
    let (_tempdir, config) = common::temp_config();
    let state_dir = config.state.root.to_string_lossy().to_string();

    let mut child = base_command(&state_dir)
        .env("SIGNAL_LIGHT_DRY_RUN", "1")
        .arg("codex-hook")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(br#"{"event":"PermissionRequest","session_id":"dry-run-session"}"#)
        .unwrap();
    let output = wait_with_output_timeout(child, Duration::from_secs(2));

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("yellow="));

    let status_output = base_command(&state_dir).arg("status").output().unwrap();
    assert!(status_output.status.success());
    let payload = serde_json::from_slice::<Value>(&status_output.stdout).unwrap();
    assert_eq!(payload["aggregate"], "idle");
    assert_eq!(payload["display_signal"], "idle");
    assert!(payload["sessions"].as_object().unwrap().is_empty());
}

#[test]
fn second_server_process_exits_while_runtime_is_alive() {
    let _guard = common::env_lock();
    let (_tempdir, config) = common::temp_config();
    let state_dir = config.state.root.to_string_lossy().to_string();
    common::autostart_runtime(&config);

    let child = base_command(&state_dir)
        .arg("server")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let output = wait_with_output_timeout(child, Duration::from_secs(2));

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already running"));

    ipc::cleanup_unreachable_server(&config);
}

#[test]
fn concurrent_cold_start_hooks_share_one_runtime_state() {
    let _guard = common::env_lock();
    let (_tempdir, config) = common::temp_config();
    let state_dir = config.state.root.to_string_lossy().to_string();
    let barrier = Arc::new(Barrier::new(3));

    let mut handles = Vec::new();
    for session_id in ["cold-start-a", "cold-start-b"] {
        let barrier = Arc::clone(&barrier);
        let state_dir = state_dir.clone();
        handles.push(thread::spawn(move || {
            barrier.wait();
            let mut child = base_command(&state_dir)
                .arg("codex-hook")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let payload = format!(r#"{{"event":"PermissionRequest","session_id":"{session_id}"}}"#);
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(payload.as_bytes())
                .unwrap();
            wait_with_output_timeout(child, Duration::from_secs(3))
        }));
    }

    barrier.wait();
    for output in handles.into_iter().map(|handle| handle.join().unwrap()) {
        assert!(
            output.status.success(),
            "stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let status_output = base_command(&state_dir).arg("status").output().unwrap();
    assert!(status_output.status.success());
    let payload = serde_json::from_slice::<Value>(&status_output.stdout).unwrap();
    assert_eq!(payload["aggregate"], "permission");
    assert_eq!(payload["display_signal"], "permission");
    assert_eq!(payload["sessions"]["cold-start-a"]["signal"], "permission");
    assert_eq!(payload["sessions"]["cold-start-b"]["signal"], "permission");
    assert!(payload["runtime_pid"].is_number());

    ipc::cleanup_unreachable_server(&config);
}

#[test]
fn install_hooks_cli_writes_codex_config() {
    let _guard = common::env_lock();
    let tempdir = tempfile::tempdir().unwrap();

    let output = Command::new(common::native_binary())
        .arg("install-hooks")
        .arg("--agent")
        .arg("codex")
        .arg("-y")
        .env("HOME", tempdir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Installed Codex: installed"));

    let config_path = tempdir.path().join(".codex").join("hooks.json");
    let payload = serde_json::from_str::<Value>(&fs::read_to_string(config_path).unwrap()).unwrap();
    assert!(
        payload["hooks"]["PermissionRequest"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .ends_with("scripts/codex-signal-hook PermissionRequest")
    );
    assert_eq!(
        payload["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"],
        10
    );
}

#[test]
fn install_hooks_cli_dry_run_does_not_write_config() {
    let _guard = common::env_lock();
    let tempdir = tempfile::tempdir().unwrap();

    let output = Command::new(common::native_binary())
        .arg("install-hooks")
        .arg("--agent")
        .arg("claude-code")
        .arg("--dry-run")
        .arg("-y")
        .env("HOME", tempdir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Would install/repair Claude Code"));
    assert!(!tempdir
        .path()
        .join(".claude")
        .join("settings.json")
        .exists());
}

#[test]
fn native_help_includes_install_hooks() {
    let output = Command::new(common::native_binary())
        .arg("--help")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("install-hooks"));
}

#[test]
fn signal_light_wrapper_uses_native_help() {
    let output = Command::new(script_path("signal-light"))
        .arg("--help")
        .env("SIGNAL_LIGHT_NATIVE_BIN", common::native_binary())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("install-hooks"));
}

#[test]
fn install_hooks_wrapper_uses_native_installer() {
    let tempdir = tempfile::tempdir().unwrap();
    let output = Command::new(script_path("install-hooks"))
        .arg("--agent")
        .arg("codex")
        .arg("--dry-run")
        .arg("-y")
        .env("HOME", tempdir.path())
        .env("SIGNAL_LIGHT_NATIVE_BIN", common::native_binary())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Would install/repair Codex"));
}

#[test]
fn codex_hook_wrapper_uses_native_dry_run() {
    let output =
        Command::new(script_path("codex-signal-hook"))
            .arg("--dry-run")
            .env("SIGNAL_LIGHT_NATIVE_BIN", common::native_binary())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                child.stdin.as_mut().unwrap().write_all(
                    br#"{"event":"PermissionRequest","session_id":"wrapper-session"}"#,
                )?;
                child.wait_with_output()
            })
            .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("yellow="));
}
