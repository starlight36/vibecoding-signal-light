use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::ensure_private_state_dir;
use crate::config::RuntimeConfig;
use crate::error::Result;
use crate::model::{RuntimeSnapshot, SessionRecord, StateDocument};
use crate::signals;

pub const OWNER_PID_SOURCE: &str = "explicit";
const WORKING_SIGNALS: &[&str] = &["thinking", "working", "tool_done"];

#[derive(Debug, Clone)]
pub struct SessionStore {
    config: RuntimeConfig,
}

impl SessionStore {
    pub fn new(config: RuntimeConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn ensure_state_dir(&self) -> Result<()> {
        ensure_private_state_dir(&self.config.state.root)
    }

    pub fn read_state(&self) -> Result<StateDocument> {
        self.ensure_state_dir()?;
        let path = &self.config.state.session_file;
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(StateDocument::default());
            }
            Err(error) => return Err(error.into()),
        };
        let parsed = serde_json::from_str::<StateDocument>(&content).unwrap_or_default();
        Ok(parsed)
    }

    pub fn write_state(&self, state: &StateDocument) -> Result<()> {
        self.ensure_state_dir()?;
        let mut document = state.clone();
        document.updated_at = Some(unix_timestamp());
        atomic_write_json(&self.config.state.session_file, &document)
    }

    pub fn update_runtime_pid(&self, runtime_pid: Option<u32>) -> Result<()> {
        let mut state = self.read_state()?;
        state.runtime_pid = runtime_pid;
        self.write_state(&state)
    }

    pub fn update_display_signal(&self, display_signal: &str) -> Result<()> {
        let mut state = self.read_state()?;
        state.display_signal = Some(display_signal.to_string());
        self.write_state(&state)
    }

    pub fn clear(&self) -> Result<()> {
        self.write_state(&StateDocument::default())
    }

    pub fn read_snapshot(&self) -> Result<RuntimeSnapshot> {
        let mut state = self.read_state()?;
        let changed = self.prune_state(&mut state);
        let snapshot = self.snapshot_from_state(&state);
        if changed {
            self.write_state(&state)?;
        }
        Ok(snapshot)
    }

    pub fn reconcile(&self) -> Result<(RuntimeSnapshot, bool)> {
        let mut state = self.read_state()?;
        let mut changed = self.prune_state(&mut state);
        let aggregate = aggregate_for_sessions(&state);

        if state.direct_signal.is_none() {
            let current_display = state
                .display_signal
                .clone()
                .unwrap_or_else(|| aggregate.clone());
            let should_keep_notice = current_display == signals::SESSION_END_NOTICE_SIGNAL;
            let should_keep_idle_sleep_off = current_display == "off" && aggregate == "idle";

            if !(should_keep_notice || should_keep_idle_sleep_off) && current_display != aggregate {
                state.display_signal = Some(aggregate.clone());
                changed = true;
            }

            if current_display == "off" && aggregate != "idle" {
                state.display_signal = Some(aggregate.clone());
                changed = true;
            }
        }

        let snapshot = self.snapshot_from_state(&state);
        if changed {
            self.write_state(&state)?;
        }
        Ok((snapshot, changed))
    }

    pub fn snapshot_from_state(&self, state: &StateDocument) -> RuntimeSnapshot {
        let aggregate = aggregate_for_sessions(state);
        let display_signal = state
            .direct_signal
            .clone()
            .or_else(|| state.display_signal.clone())
            .filter(|value| {
                value == signals::SESSION_END_NOTICE_SIGNAL
                    || value == "off"
                    || signals::is_public_signal(value)
            })
            .unwrap_or_else(|| aggregate.clone());

        RuntimeSnapshot {
            aggregate,
            display_signal,
            sessions: state.sessions.clone(),
            runtime_pid: state.runtime_pid,
            updated_at: state.updated_at,
        }
    }

    pub fn prune_state(&self, state: &mut StateDocument) -> bool {
        let before_len = state.sessions.len();
        let had_invalid_runtime_pid = state.runtime_pid.is_some_and(|pid| !is_pid_running(pid));
        let now = unix_timestamp();
        state
            .sessions
            .retain(|_, record| should_keep_session(record, now, &self.config));
        let mut changed = before_len != state.sessions.len();
        if state
            .direct_signal
            .as_deref()
            .is_some_and(|signal_name| !signals::is_public_signal(signal_name))
        {
            state.direct_signal = None;
            changed = true;
        }
        if had_invalid_runtime_pid {
            state.runtime_pid = None;
            changed = true;
        }
        changed
    }
}

pub fn aggregate_for_sessions(state: &StateDocument) -> String {
    signals::aggregate_signals(state.sessions.values().map(|record| record.signal.as_str()))
        .to_string()
}

pub fn should_keep_session(record: &SessionRecord, now: f64, config: &RuntimeConfig) -> bool {
    if !signals::is_public_signal(&record.signal) {
        return false;
    }
    if !record.updated_at.is_finite() {
        return false;
    }
    let age_seconds = now - record.updated_at;
    if age_seconds > config.timing.session_ttl_seconds as f64 {
        return false;
    }
    if WORKING_SIGNALS.contains(&record.signal.as_str())
        && age_seconds > config.timing.work_session_stale_seconds as f64
    {
        return false;
    }

    match (record.owner_pid, record.owner_pid_source.as_deref()) {
        (Some(owner_pid), Some(OWNER_PID_SOURCE)) => is_pid_running(owner_pid),
        (Some(_), Some(_)) | (Some(_), None) => !WORKING_SIGNALS.contains(&record.signal.as_str()),
        _ => true,
    }
}

pub fn is_pid_running(pid: u32) -> bool {
    unsafe {
        libc::kill(pid as i32, 0) == 0
            || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}

pub fn unix_timestamp() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

fn atomic_write_json(path: &Path, document: &StateDocument) -> Result<()> {
    let serialized = serde_json::to_string_pretty(document)? + "\n";
    let temp_path = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&temp_path, serialized)?;
    fs::rename(temp_path, path)?;
    Ok(())
}
