use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::error::{Result, SignalLightError};
use crate::model::HardwareMapping;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatePaths {
    pub root: PathBuf,
    pub server_pid_file: PathBuf,
    pub server_log_file: PathBuf,
    pub server_lock_file: PathBuf,
    pub server_startup_lock_file: PathBuf,
    pub server_socket_file: PathBuf,
    pub session_file: PathBuf,
}

impl StatePaths {
    pub fn new(root: PathBuf) -> Self {
        Self {
            server_pid_file: root.join("server.json"),
            server_log_file: root.join("server.log"),
            server_lock_file: root.join("server.lock"),
            server_startup_lock_file: root.join("server-startup.lock"),
            server_socket_file: root.join("server.sock"),
            session_file: root.join("sessions.json"),
            root,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimingConfig {
    pub session_ttl_seconds: u64,
    pub work_session_stale_seconds: u64,
    pub idle_sleep_seconds: u64,
    pub request_timeout_millis: u64,
    pub request_retry_poll_millis: u64,
    pub startup_timeout_millis: u64,
    pub server_poll_millis: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub state: StatePaths,
    pub timing: TimingConfig,
    pub hardware: HardwareMapping,
}

impl RuntimeConfig {
    pub fn from_env() -> Result<Self> {
        Self::from_pairs(std::env::vars())
    }

    pub fn from_pairs<I, K, V>(pairs: I) -> Result<Self>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let env = pairs
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect::<BTreeMap<String, String>>();
        let state_root = env
            .get("SIGNAL_LIGHT_STATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(default_state_root);
        let hardware = HardwareMapping {
            green_pin: env
                .get("SIGNAL_LIGHT_GREEN_PIN")
                .cloned()
                .unwrap_or_else(|| "gp0".to_string()),
            yellow_pin: env
                .get("SIGNAL_LIGHT_YELLOW_PIN")
                .cloned()
                .unwrap_or_else(|| "gp1".to_string()),
            red_pin: env
                .get("SIGNAL_LIGHT_RED_PIN")
                .cloned()
                .unwrap_or_else(|| "gp2".to_string()),
            active_low: parse_bool(env.get("SIGNAL_LIGHT_ACTIVE_LOW"), true)?,
        };
        Ok(Self {
            state: StatePaths::new(state_root),
            timing: TimingConfig {
                session_ttl_seconds: parse_u64(
                    env.get("SIGNAL_LIGHT_SESSION_TTL_SECONDS"),
                    86_400,
                )?,
                work_session_stale_seconds: parse_u64(
                    env.get("SIGNAL_LIGHT_WORK_SESSION_STALE_SECONDS"),
                    1_800,
                )?,
                idle_sleep_seconds: parse_u64(env.get("SIGNAL_LIGHT_IDLE_SLEEP_SECONDS"), 600)?,
                request_timeout_millis: parse_u64(
                    env.get("SIGNAL_LIGHT_SERVER_REQUEST_TIMEOUT_SECONDS"),
                    1,
                )?
                .saturating_mul(1_000),
                request_retry_poll_millis: parse_f64(
                    env.get("SIGNAL_LIGHT_SERVER_REQUEST_POLL_SECONDS"),
                    0.01,
                )?
                .mul_add(1_000.0, 0.0) as u64,
                startup_timeout_millis: 3_000,
                server_poll_millis: parse_f64(env.get("SIGNAL_LIGHT_SERVER_POLL_SECONDS"), 0.05)?
                    .mul_add(1_000.0, 0.0) as u64,
            },
            hardware,
        })
    }
}

pub fn default_state_root() -> PathBuf {
    if cfg!(target_os = "macos") {
        PathBuf::from("/private/tmp/signal-light")
    } else {
        PathBuf::from("/tmp/signal-light")
    }
}

fn parse_bool(raw: Option<&String>, default: bool) -> Result<bool> {
    let Some(value) = raw else {
        return Ok(default);
    };
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Ok(default);
    }
    match normalized.as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(SignalLightError::Configuration(format!(
            "Invalid boolean value: {value}"
        ))),
    }
}

fn parse_u64(raw: Option<&String>, default: u64) -> Result<u64> {
    let Some(value) = raw else {
        return Ok(default);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(default);
    }
    trimmed
        .parse::<u64>()
        .map_err(|_| SignalLightError::Configuration(format!("Invalid integer value: {trimmed}")))
}

fn parse_f64(raw: Option<&String>, default: f64) -> Result<f64> {
    let Some(value) = raw else {
        return Ok(default);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(default);
    }
    trimmed
        .parse::<f64>()
        .map_err(|_| SignalLightError::Configuration(format!("Invalid float value: {trimmed}")))
}
