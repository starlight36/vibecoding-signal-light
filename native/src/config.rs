use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

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
            .unwrap_or_else(|| default_state_root_from_env(&env));
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
    let env = std::env::vars().collect::<BTreeMap<String, String>>();
    default_state_root_from_env(&env)
}

pub fn ensure_private_state_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    harden_state_dir(path)
}

fn default_state_root_from_env(env: &BTreeMap<String, String>) -> PathBuf {
    if let Some(state_home) = env_path(env, "XDG_STATE_HOME") {
        return state_home.join("signal-light");
    }
    if let Some(home) = env_path(env, "HOME") {
        return if cfg!(target_os = "macos") {
            home.join("Library")
                .join("Application Support")
                .join("signal-light")
        } else {
            home.join(".local").join("state").join("signal-light")
        };
    }

    fallback_state_root()
}

fn env_path(env: &BTreeMap<String, String>, key: &str) -> Option<PathBuf> {
    env.get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn fallback_state_root() -> PathBuf {
    let suffix = current_user_suffix();
    std::env::temp_dir().join(format!("signal-light-{suffix}"))
}

#[cfg(unix)]
fn current_user_suffix() -> String {
    unsafe { libc::geteuid() }.to_string()
}

#[cfg(not(unix))]
fn current_user_suffix() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "current-user".to_string())
}

#[cfg(unix)]
fn harden_state_dir(path: &Path) -> Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Err(SignalLightError::Configuration(format!(
            "Signal Light state path is not a directory: {}",
            path.display()
        )));
    }
    let current_uid = unsafe { libc::geteuid() };
    if metadata.uid() != current_uid {
        return Err(SignalLightError::Configuration(format!(
            "Signal Light state directory is owned by uid {}, expected uid {}: {}",
            metadata.uid(),
            current_uid,
            path.display()
        )));
    }

    let mode = metadata.permissions().mode() & 0o777;
    if mode != 0o700 {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn harden_state_dir(_path: &Path) -> Result<()> {
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::{default_state_root_from_env, ensure_private_state_dir};
    use std::collections::BTreeMap;

    #[test]
    fn default_state_root_prefers_xdg_state_home() {
        let env = BTreeMap::from([
            ("XDG_STATE_HOME".to_string(), "/state-home".to_string()),
            ("HOME".to_string(), "/home/demo".to_string()),
        ]);

        assert_eq!(
            default_state_root_from_env(&env),
            std::path::PathBuf::from("/state-home").join("signal-light")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn default_state_root_uses_macos_application_support() {
        let env = BTreeMap::from([("HOME".to_string(), "/Users/demo".to_string())]);

        assert_eq!(
            default_state_root_from_env(&env),
            std::path::PathBuf::from("/Users/demo")
                .join("Library")
                .join("Application Support")
                .join("signal-light")
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn default_state_root_uses_user_local_state() {
        let env = BTreeMap::from([("HOME".to_string(), "/home/demo".to_string())]);

        assert_eq!(
            default_state_root_from_env(&env),
            std::path::PathBuf::from("/home/demo")
                .join(".local")
                .join("state")
                .join("signal-light")
        );
    }

    #[test]
    fn ensure_private_state_dir_creates_directory() {
        let tempdir = tempfile::tempdir().unwrap();
        let state_dir = tempdir.path().join("state");

        ensure_private_state_dir(&state_dir).unwrap();

        assert!(state_dir.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = state_dir.metadata().unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700);
        }
    }
}
