use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::config::{ensure_private_state_dir, RuntimeConfig};
use crate::error::{Result, SignalLightError};
use crate::model::{RuntimeCommand, RuntimeResponse, RuntimeSnapshot};
use crate::runtime::session_store::{is_pid_running, SessionStore};

pub struct FileLock {
    file: File,
}

impl Drop for FileLock {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

pub fn status(config: &RuntimeConfig) -> Result<RuntimeSnapshot> {
    match request(
        config,
        &RuntimeCommand {
            action: "status".to_string(),
            session_key: None,
            signal_name: None,
            owner_pid: None,
            speed: None,
            reply_to: None,
        },
        false,
    ) {
        Ok(response) => response_into_snapshot(response),
        Err(_) => SessionStore::new(config.clone()).read_snapshot(),
    }
}

pub fn request(
    config: &RuntimeConfig,
    command: &RuntimeCommand,
    start_if_missing: bool,
) -> Result<RuntimeResponse> {
    if start_if_missing {
        return match request_once_transport(config, command) {
            Ok(response) => response_into_result(response),
            Err(error) => {
                cleanup_unreachable_server(config);
                ensure_server_running(config)?;
                request_once_transport(config, command)
                    .and_then(response_into_result)
                    .map_err(|second_error| {
                        SignalLightError::Runtime(format!(
                            "Cannot reach signal server: {}; retry failed with {}",
                            error, second_error
                        ))
                    })
            }
        };
    }

    if !server_running(config) {
        return Err(SignalLightError::Runtime(
            "Signal server is not running.".to_string(),
        ));
    }

    request_once_transport(config, command).and_then(response_into_result)
}

pub fn ensure_server_running(config: &RuntimeConfig) -> Result<()> {
    if server_running(config) {
        return Ok(());
    }

    let _startup_lock = acquire_startup_lock(config)?;
    if server_running(config) {
        return Ok(());
    }

    cleanup_unreachable_server(config);
    ensure_private_state_dir(&config.state.root)?;

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.state.server_log_file)?;
    let log_file_err = log_file.try_clone()?;
    let binary = native_binary_path()?;
    let mut command = Command::new(binary);
    command
        .arg("server")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_file_err));
    command.env("SIGNAL_LIGHT_STATE_DIR", &config.state.root);
    command.env("SIGNAL_LIGHT_GREEN_PIN", &config.hardware.green_pin);
    command.env("SIGNAL_LIGHT_YELLOW_PIN", &config.hardware.yellow_pin);
    command.env("SIGNAL_LIGHT_RED_PIN", &config.hardware.red_pin);
    command.env(
        "SIGNAL_LIGHT_ACTIVE_LOW",
        if config.hardware.active_low { "1" } else { "0" },
    );
    if matches!(
        std::env::var("SIGNAL_LIGHT_SERVER_DRY_RUN")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    ) {
        command.arg("--dry-run");
    }
    let mut child = command.spawn().map_err(|error| {
        SignalLightError::Runtime(format!("Failed to start signal server: {error}"))
    })?;

    let deadline = Instant::now() + Duration::from_millis(config.timing.startup_timeout_millis);
    while Instant::now() < deadline {
        if server_running(config) {
            return Ok(());
        }
        if let Ok(Some(_status)) = child.try_wait() {
            return Err(SignalLightError::Runtime(server_error_message(config)));
        }
        thread::sleep(Duration::from_millis(50));
    }

    let _ = stop_process(child.id());
    Err(SignalLightError::Timeout(
        "Signal server did not start within the 3 second budget.".to_string(),
    ))
}

pub fn request_once(config: &RuntimeConfig, command: &RuntimeCommand) -> Result<RuntimeResponse> {
    request_once_transport(config, command).and_then(response_into_result)
}

fn request_once_transport(
    config: &RuntimeConfig,
    command: &RuntimeCommand,
) -> Result<RuntimeResponse> {
    let request_pipe = &config.state.server_socket_file;
    if !request_pipe.exists() {
        return Err(SignalLightError::Runtime(
            "Signal server request pipe is missing.".to_string(),
        ));
    }

    let response_path = config
        .state
        .root
        .join(format!("{}.response.json", unique_request_id()));
    let mut transport_command = command.clone();
    transport_command.reply_to = Some(response_path.to_string_lossy().to_string());

    // Open both ends of the FIFO from the client side so a stale pipe cannot
    // block forever when the server exits before attaching a reader.
    let mut pipe = OpenOptions::new()
        .read(true)
        .write(true)
        .open(request_pipe)
        .map_err(|error| {
            SignalLightError::Runtime(format!("Signal server is unreachable: {error}"))
        })?;
    let payload = serde_json::to_vec(&transport_command)?;
    pipe.write_all(&payload)?;
    pipe.write_all(b"\n")?;
    pipe.flush()?;
    drop(pipe);

    let deadline = Instant::now() + Duration::from_millis(config.timing.request_timeout_millis);
    loop {
        match fs::read(&response_path) {
            Ok(buffer) => {
                let _ = fs::remove_file(&response_path);
                return Ok(serde_json::from_slice::<RuntimeResponse>(&buffer)?);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if Instant::now() >= deadline {
                    let _ = fs::remove_file(&response_path);
                    return Err(SignalLightError::Timeout(
                        "Signal server did not respond in time.".to_string(),
                    ));
                }
                thread::sleep(Duration::from_millis(
                    config.timing.request_retry_poll_millis.max(10),
                ));
            }
            Err(error) => return Err(error.into()),
        }
    }
}

pub fn server_running(config: &RuntimeConfig) -> bool {
    let pid = read_runtime_pid(config);
    if let Some(pid) = pid {
        if !is_pid_running(pid) {
            return false;
        }
    }
    if !config.state.server_socket_file.exists() {
        return false;
    }

    let command = RuntimeCommand {
        action: "status".to_string(),
        session_key: None,
        signal_name: None,
        owner_pid: None,
        speed: None,
        reply_to: None,
    };
    request_once_transport(config, &command)
        .map(|response| response.ok)
        .unwrap_or(false)
}

pub fn cleanup_unreachable_server(config: &RuntimeConfig) {
    match acquire_file_lock(&config.state.server_lock_file, true) {
        Ok(Some(lock)) => {
            cleanup_state_under_lock(config, lock);
        }
        Ok(None) => {
            if let Some(pid) = read_lock_holder_pid(config).filter(|pid| *pid != std::process::id())
            {
                let _ = stop_process(pid);
                if let Some(lock) = acquire_cleanup_lock(config, Duration::from_secs(1)) {
                    cleanup_state_under_lock(config, lock);
                }
            }
        }
        Err(_) => {}
    }
}

pub fn create_request_pipe(path: &Path) -> Result<()> {
    let _ = fs::remove_file(path);
    let c_path = CString::new(path.as_os_str().as_encoded_bytes()).map_err(|_| {
        SignalLightError::Runtime("Request pipe path contains an embedded NUL byte.".to_string())
    })?;
    let result = unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    Err(SignalLightError::Runtime(format!(
        "Failed to create request pipe: {error}"
    )))
}

pub fn acquire_process_lock(config: &RuntimeConfig) -> Result<FileLock> {
    ensure_private_state_dir(&config.state.root)?;
    let lock = acquire_file_lock(&config.state.server_lock_file, true)?.ok_or_else(|| {
        SignalLightError::Runtime("Signal server is already running.".to_string())
    })?;
    write_lock_holder_pid(config, std::process::id())?;
    Ok(lock)
}

pub fn acquire_startup_lock(config: &RuntimeConfig) -> Result<FileLock> {
    ensure_private_state_dir(&config.state.root)?;
    match acquire_file_lock(&config.state.server_startup_lock_file, false)? {
        Some(lock) => Ok(lock),
        None => unreachable!("blocking startup lock acquisition always returns a lock"),
    }
}

fn read_runtime_pid(config: &RuntimeConfig) -> Option<u32> {
    let store = SessionStore::new(config.clone());
    store.read_state().ok().and_then(|state| state.runtime_pid)
}

fn stop_process(pid: u32) -> bool {
    unsafe {
        if libc::kill(pid as i32, libc::SIGTERM) != 0 {
            return false;
        }
    }
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        if !is_pid_running(pid) {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    unsafe {
        let _ = libc::kill(pid as i32, libc::SIGKILL);
    }
    !is_pid_running(pid)
}

fn server_error_message(config: &RuntimeConfig) -> String {
    match fs::read_to_string(&config.state.server_log_file) {
        Ok(log) => log
            .lines()
            .last()
            .map(|line| format!("Signal server exited immediately: {line}"))
            .unwrap_or_else(|| "Signal server exited immediately.".to_string()),
        Err(_) => "Signal server exited immediately.".to_string(),
    }
}

fn native_binary_path() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("SIGNAL_LIGHT_NATIVE_BIN") {
        return Ok(PathBuf::from(path));
    }
    std::env::current_exe().map_err(|error| {
        SignalLightError::Runtime(format!(
            "Cannot locate signal-light-native executable: {error}"
        ))
    })
}

fn response_into_snapshot(response: RuntimeResponse) -> Result<RuntimeSnapshot> {
    Ok(RuntimeSnapshot {
        aggregate: response.aggregate.ok_or_else(|| {
            SignalLightError::Protocol("Missing aggregate in status response.".to_string())
        })?,
        display_signal: response.display_signal.ok_or_else(|| {
            SignalLightError::Protocol("Missing display_signal in status response.".to_string())
        })?,
        sessions: response.sessions,
        runtime_pid: response.runtime_pid,
        updated_at: response.updated_at,
    })
}

fn response_into_result(response: RuntimeResponse) -> Result<RuntimeResponse> {
    if response.ok {
        Ok(response)
    } else {
        Err(SignalLightError::Runtime(response.error.unwrap_or_else(
            || "Signal server request failed.".to_string(),
        )))
    }
}

fn cleanup_state_under_lock(config: &RuntimeConfig, _lock: FileLock) {
    let store = SessionStore::new(config.clone());
    let _ = store.update_runtime_pid(None);
    let _ = fs::remove_file(&config.state.server_socket_file);
    let _ = fs::remove_file(&config.state.server_pid_file);
}

fn acquire_cleanup_lock(config: &RuntimeConfig, timeout: Duration) -> Option<FileLock> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match acquire_file_lock(&config.state.server_lock_file, true) {
            Ok(Some(lock)) => return Some(lock),
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(_) => return None,
        }
    }
    None
}

fn read_lock_holder_pid(config: &RuntimeConfig) -> Option<u32> {
    let content = fs::read_to_string(&config.state.server_pid_file).ok()?;
    let payload = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    payload
        .get("pid")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
}

fn write_lock_holder_pid(config: &RuntimeConfig, pid: u32) -> Result<()> {
    fs::write(
        &config.state.server_pid_file,
        serde_json::to_string_pretty(&serde_json::json!({
            "pid": pid,
            "started_at": unix_timestamp_for_pid_file(),
        }))? + "\n",
    )?;
    Ok(())
}

fn unique_request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{}-{nanos}", std::process::id())
}

fn unix_timestamp_for_pid_file() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

fn acquire_file_lock(path: &Path, nonblocking: bool) -> Result<Option<FileLock>> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    let flags = if nonblocking {
        libc::LOCK_EX | libc::LOCK_NB
    } else {
        libc::LOCK_EX
    };
    let result = unsafe { libc::flock(file.as_raw_fd(), flags) };
    if result == 0 {
        return Ok(Some(FileLock { file }));
    }
    let error = std::io::Error::last_os_error();
    let raw_error = error.raw_os_error();
    if nonblocking && (raw_error == Some(libc::EWOULDBLOCK) || raw_error == Some(libc::EAGAIN)) {
        return Ok(None);
    }
    Err(SignalLightError::Runtime(format!(
        "Failed to acquire runtime lock: {error}"
    )))
}
