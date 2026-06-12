use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};

use crate::error::{Result, SignalLightError};

pub const NATIVE_BINARY_ENV: &str = "SIGNAL_LIGHT_NATIVE_BIN";
pub const PROJECT_ROOT_ENV: &str = "SIGNAL_LIGHT_PROJECT_ROOT";
const CODEX_EVENTS: &[(&str, u64)] = &[
    ("SessionStart", 5),
    ("UserPromptSubmit", 5),
    ("PreToolUse", 5),
    ("PostToolUse", 5),
    ("PermissionRequest", 10),
    ("Stop", 5),
    ("SessionEnd", 5),
];

const CLAUDE_CODE_EVENTS: &[(&str, u64)] = &[
    ("SessionStart", 5),
    ("UserPromptSubmit", 5),
    ("PreToolUse", 5),
    ("PostToolUse", 5),
    ("PostToolUseFailure", 5),
    ("PreCompact", 5),
    ("SubagentStart", 5),
    ("SubagentStop", 5),
    ("PermissionRequest", 10),
    ("Notification", 5),
    ("Stop", 5),
    ("SessionEnd", 5),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSpec {
    pub key: &'static str,
    pub name: &'static str,
    pub config_path: PathBuf,
    pub hook_script: PathBuf,
    pub events: &'static [(&'static str, u64)],
    pub passes_event_arg: bool,
    pub uses_matcher: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatus {
    pub spec: AgentSpec,
    pub installed: bool,
    pub config_exists: bool,
    pub valid_json: bool,
    pub missing_events: Vec<String>,
    pub broken_events: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallResult {
    pub status: AgentStatus,
    pub changed: bool,
    pub backup_path: Option<PathBuf>,
}

pub fn run_cli(
    selected_agents: Vec<String>,
    all_agents: bool,
    yes: bool,
    dry_run: bool,
) -> Result<i32> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut out = stdout.lock();
    run_install_wizard(
        &selected_agents,
        all_agents,
        yes,
        dry_run,
        None,
        &mut input,
        &mut out,
    )
}

pub fn run_install_wizard(
    selected_agents: &[String],
    all_agents: bool,
    yes: bool,
    dry_run: bool,
    home: Option<&Path>,
    input: &mut dyn BufRead,
    out: &mut dyn Write,
) -> Result<i32> {
    let agents = supported_agents(home)?;
    let mut statuses = Vec::new();
    for spec in &agents {
        statuses.push(inspect_agent(spec)?);
    }

    writeln!(out, "Signal Light hook installer")?;
    writeln!(out)?;
    for (index, status) in statuses.iter().enumerate() {
        let marker = if status.installed {
            "ok"
        } else {
            "needs repair"
        };
        let exists = if status.config_exists {
            "found"
        } else {
            "missing"
        };
        writeln!(
            out,
            "{}. {}: {} ({}; config {})",
            index + 1,
            status.spec.name,
            marker,
            status.message,
            exists
        )?;
        writeln!(out, "   {}", status.spec.config_path.display())?;
    }

    let selected = resolve_selection(&statuses, selected_agents, all_agents, yes, input, out)?;
    if selected.is_empty() {
        writeln!(out)?;
        writeln!(out, "No agents selected.")?;
        return Ok(0);
    }

    writeln!(out)?;
    for key in selected {
        let spec = agents
            .iter()
            .find(|spec| spec.key == key)
            .ok_or_else(|| SignalLightError::InvalidUsage(format!("Unsupported agent: {key}")))?;
        if dry_run {
            writeln!(
                out,
                "Would install/repair {}: {}",
                spec.name,
                spec.config_path.display()
            )?;
            continue;
        }
        let result = install_agent(spec, true)?;
        writeln!(out, "Installed {}: {}", spec.name, result.status.message)?;
        if let Some(backup_path) = result.backup_path {
            writeln!(out, "  backup: {}", backup_path.display())?;
        }
    }

    writeln!(out)?;
    writeln!(out, "{}", native_runtime_repair_hint())?;
    Ok(0)
}

pub fn supported_agents(home: Option<&Path>) -> Result<Vec<AgentSpec>> {
    let home_dir = resolve_home(home)?;
    let project_root = project_root();
    Ok(vec![
        AgentSpec {
            key: "codex",
            name: "Codex",
            config_path: home_dir.join(".codex").join("hooks.json"),
            hook_script: project_root.join("scripts").join("codex-signal-hook"),
            events: CODEX_EVENTS,
            passes_event_arg: true,
            uses_matcher: false,
        },
        AgentSpec {
            key: "claude-code",
            name: "Claude Code",
            config_path: home_dir.join(".claude").join("settings.json"),
            hook_script: project_root.join("scripts").join("claude-code-signal-hook"),
            events: CLAUDE_CODE_EVENTS,
            passes_event_arg: false,
            uses_matcher: true,
        },
    ])
}

pub fn inspect_agent(spec: &AgentSpec) -> Result<AgentStatus> {
    let config_exists = spec.config_path.exists();
    let (config, valid_json) = load_json_config(&spec.config_path)?;

    if !config_exists {
        return Ok(AgentStatus {
            spec: spec.clone(),
            installed: false,
            config_exists: false,
            valid_json: true,
            missing_events: spec
                .events
                .iter()
                .map(|(event, _)| (*event).to_string())
                .collect(),
            broken_events: Vec::new(),
            message: "config missing".to_string(),
        });
    }

    if !valid_json {
        return Ok(AgentStatus {
            spec: spec.clone(),
            installed: false,
            config_exists: true,
            valid_json: false,
            missing_events: spec
                .events
                .iter()
                .map(|(event, _)| (*event).to_string())
                .collect(),
            broken_events: Vec::new(),
            message: "invalid JSON".to_string(),
        });
    }

    let Some(hooks) = config.get("hooks").and_then(Value::as_object) else {
        return Ok(AgentStatus {
            spec: spec.clone(),
            installed: false,
            config_exists: true,
            valid_json: true,
            missing_events: spec
                .events
                .iter()
                .map(|(event, _)| (*event).to_string())
                .collect(),
            broken_events: Vec::new(),
            message: "hooks missing".to_string(),
        });
    };

    let mut missing_events = Vec::new();
    let mut broken_events = Vec::new();
    for (event, timeout) in spec.events {
        match hooks.get(*event) {
            None => missing_events.push((*event).to_string()),
            Some(entries) if !event_has_expected_hook(entries, spec, event, *timeout) => {
                broken_events.push((*event).to_string())
            }
            Some(_) => {}
        }
    }

    let installed = missing_events.is_empty() && broken_events.is_empty();
    let message = if installed {
        "installed".to_string()
    } else if !missing_events.is_empty() && !broken_events.is_empty() {
        format!(
            "{}, {} broken",
            count_message(&missing_events),
            broken_events.len()
        )
    } else if !missing_events.is_empty() {
        count_message(&missing_events)
    } else {
        format!("{} broken", broken_events.len())
    };

    Ok(AgentStatus {
        spec: spec.clone(),
        installed,
        config_exists: true,
        valid_json: true,
        missing_events,
        broken_events,
        message,
    })
}

pub fn install_agent(spec: &AgentSpec, backup: bool) -> Result<InstallResult> {
    let (mut config, valid_json) = load_json_config(&spec.config_path)?;
    if !valid_json {
        config = Map::new();
    }

    let original_text = match fs::read_to_string(&spec.config_path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };

    let hooks_value = config
        .entry("hooks".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !hooks_value.is_object() {
        *hooks_value = Value::Object(Map::new());
    }
    let hooks = hooks_value.as_object_mut().expect("hooks object");

    for (event, timeout) in spec.events {
        let merged = merge_event_groups(hooks.get(*event), spec, event, *timeout);
        hooks.insert((*event).to_string(), merged);
    }

    let new_text = serde_json::to_string_pretty(&Value::Object(config))? + "\n";
    if original_text.as_deref() == Some(new_text.as_str()) {
        let status = inspect_agent(spec)?;
        return Ok(InstallResult {
            status,
            changed: false,
            backup_path: None,
        });
    }

    let backup_path = if backup && spec.config_path.exists() {
        Some(backup_config(&spec.config_path)?)
    } else {
        None
    };

    if let Some(parent) = spec.config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&spec.config_path, new_text)?;
    let status = inspect_agent(spec)?;
    Ok(InstallResult {
        status,
        changed: true,
        backup_path,
    })
}

pub fn native_runtime_repair_hint() -> String {
    let preferred = native_runtime_candidates()
        .into_iter()
        .find(|candidate| candidate.exists());
    let target = preferred
        .map(|candidate| candidate.display().to_string())
        .unwrap_or_else(|| "native/target/release/signal-light-native".to_string());
    format!(
        "Wrappers require the native runtime. Build it with `cargo build --manifest-path native/Cargo.toml --release`, or set {NATIVE_BINARY_ENV} to a custom binary. Expected binary path: {target}"
    )
}

fn resolve_home(home: Option<&Path>) -> Result<PathBuf> {
    if let Some(home_dir) = home {
        return Ok(home_dir.to_path_buf());
    }
    env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
        SignalLightError::Configuration("Cannot determine home directory.".to_string())
    })
}

fn project_root() -> PathBuf {
    if let Some(root) = env::var_os(PROJECT_ROOT_ENV).map(PathBuf::from) {
        return root;
    }
    if let Some(root) = release_layout_root() {
        return root;
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("project root")
        .to_path_buf()
}

fn release_layout_root() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    release_layout_root_from_executable(&executable)
}

fn release_layout_root_from_executable(executable: &Path) -> Option<PathBuf> {
    let bin_dir = executable.parent()?;
    let root = bin_dir.parent()?;
    let scripts_dir = root.join("scripts");
    let native_binary = bin_dir.join("signal-light-native");
    if scripts_dir.join("codex-signal-hook").is_file()
        && scripts_dir.join("claude-code-signal-hook").is_file()
        && native_binary.is_file()
    {
        Some(root.to_path_buf())
    } else {
        None
    }
}

fn native_runtime_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(explicit) = env::var_os(NATIVE_BINARY_ENV) {
        candidates.push(PathBuf::from(explicit));
    }
    let project_root = project_root();
    candidates.push(
        project_root
            .join("native")
            .join("target")
            .join("release")
            .join("signal-light-native"),
    );
    candidates.push(
        project_root
            .join("native")
            .join("target")
            .join("debug")
            .join("signal-light-native"),
    );
    candidates
}

fn count_message(events: &[String]) -> String {
    format!("{} missing", events.len())
}

fn resolve_selection(
    statuses: &[AgentStatus],
    selected_agents: &[String],
    all_agents: bool,
    yes: bool,
    input: &mut dyn BufRead,
    out: &mut dyn Write,
) -> Result<Vec<String>> {
    if !selected_agents.is_empty() {
        return selected_agents
            .iter()
            .map(|agent| normalize_agent_key(agent, statuses))
            .collect();
    }

    if all_agents {
        return Ok(statuses
            .iter()
            .map(|status| status.spec.key.to_string())
            .collect());
    }

    let suggested: Vec<String> = statuses
        .iter()
        .filter(|status| !status.installed)
        .map(|status| status.spec.key.to_string())
        .collect();
    if yes {
        if suggested.is_empty() {
            return Ok(statuses
                .iter()
                .map(|status| status.spec.key.to_string())
                .collect());
        }
        return Ok(suggested);
    }

    let mut default_selection = statuses
        .iter()
        .enumerate()
        .filter(|(_, status)| !status.installed)
        .map(|(index, _)| (index + 1).to_string())
        .collect::<Vec<_>>()
        .join(",");
    if default_selection.is_empty() {
        default_selection = format!("1-{}", statuses.len());
    }

    writeln!(out)?;
    writeln!(
        out,
        "Select agents to install/repair [{}] (comma separated, or 'all'):",
        default_selection
    )?;
    let mut answer = String::new();
    input.read_line(&mut answer)?;
    if answer.trim().is_empty() {
        answer = default_selection;
    }
    parse_selection(answer.trim(), statuses)
}

fn normalize_agent_key(value: &str, statuses: &[AgentStatus]) -> Result<String> {
    let key = match value.trim().to_ascii_lowercase().as_str() {
        "claude" | "claudecode" => "claude-code".to_string(),
        other => other.to_string(),
    };
    if statuses.iter().any(|status| status.spec.key == key) {
        Ok(key)
    } else {
        Err(SignalLightError::InvalidUsage(format!(
            "Unsupported agent: {value}"
        )))
    }
}

fn parse_selection(answer: &str, statuses: &[AgentStatus]) -> Result<Vec<String>> {
    let normalized = answer.trim().to_ascii_lowercase();
    if matches!(normalized.as_str(), "all" | "a" | "*") {
        return Ok(statuses
            .iter()
            .map(|status| status.spec.key.to_string())
            .collect());
    }
    if matches!(normalized.as_str(), "none" | "n" | "skip" | "q" | "quit") {
        return Ok(Vec::new());
    }

    let mut selected = Vec::new();
    for chunk in normalized.replace(' ', "").split(',') {
        if chunk.is_empty() {
            continue;
        }
        if let Some((start, end)) = chunk.split_once('-') {
            if start.chars().all(|ch| ch.is_ascii_digit())
                && end.chars().all(|ch| ch.is_ascii_digit())
            {
                let start = start.parse::<usize>().map_err(invalid_selection)?;
                let end = end.parse::<usize>().map_err(invalid_selection)?;
                for number in start..=end {
                    push_unique(&mut selected, key_by_index(statuses, number)?);
                }
                continue;
            }
        }
        if chunk.chars().all(|ch| ch.is_ascii_digit()) {
            let number = chunk.parse::<usize>().map_err(invalid_selection)?;
            push_unique(&mut selected, key_by_index(statuses, number)?);
            continue;
        }

        let key = match chunk {
            "claude" | "claudecode" => "claude-code".to_string(),
            _ => chunk.to_string(),
        };
        if !statuses.iter().any(|status| status.spec.key == key) {
            return Err(SignalLightError::InvalidUsage(format!(
                "Unsupported selection: {chunk}"
            )));
        }
        push_unique(&mut selected, key);
    }

    Ok(selected)
}

fn invalid_selection(error: std::num::ParseIntError) -> SignalLightError {
    SignalLightError::InvalidUsage(error.to_string())
}

fn push_unique(selected: &mut Vec<String>, key: String) {
    if !selected.contains(&key) {
        selected.push(key);
    }
}

fn key_by_index(statuses: &[AgentStatus], number: usize) -> Result<String> {
    if number == 0 || number > statuses.len() {
        return Err(SignalLightError::InvalidUsage(format!(
            "Selection index out of range: {number}"
        )));
    }
    Ok(statuses[number - 1].spec.key.to_string())
}

fn load_json_config(path: &Path) -> Result<(Map<String, Value>, bool)> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((Map::new(), true)),
        Err(error) => return Err(error.into()),
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(object)) => Ok((object, true)),
        Ok(_) | Err(_) => Ok((Map::new(), false)),
    }
}

fn backup_config(path: &Path) -> Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let file_name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "config".to_string());
    let backup_path = path.with_file_name(format!("{file_name}.bak-signal-light-install-{stamp}"));
    fs::copy(path, &backup_path)?;
    Ok(backup_path)
}

fn event_has_expected_hook(entries: &Value, spec: &AgentSpec, event: &str, timeout: u64) -> bool {
    let Some(entries) = entries.as_array() else {
        return false;
    };
    let expected = hook_command(spec, event);
    for group in entries {
        let Some(group) = group.as_object() else {
            continue;
        };
        if spec.uses_matcher && group.get("matcher").and_then(Value::as_str) != Some("") {
            continue;
        }
        let Some(hooks) = group.get("hooks").and_then(Value::as_array) else {
            continue;
        };
        for hook in hooks {
            let Some(hook) = hook.as_object() else {
                continue;
            };
            if hook.get("type").and_then(Value::as_str) == Some("command")
                && hook.get("command").and_then(Value::as_str) == Some(expected.as_str())
                && hook.get("timeout").and_then(Value::as_u64) == Some(timeout)
            {
                return true;
            }
        }
    }
    false
}

fn merge_event_groups(
    existing_entries: Option<&Value>,
    spec: &AgentSpec,
    event: &str,
    timeout: u64,
) -> Value {
    let replacement = hook_group(spec, event, timeout);
    let Some(existing_entries) = existing_entries.and_then(Value::as_array) else {
        return Value::Array(vec![replacement]);
    };

    let mut merged = Vec::new();
    let mut replaced = false;
    for group in existing_entries {
        let (replacement_group, cleaned_group, had_signal_light_hook) =
            replace_signal_light_hooks(group, spec, &replacement);
        if had_signal_light_hook {
            if let Some(group) = replacement_group {
                merged.push(group);
                replaced = true;
            }
            if let Some(group) = cleaned_group {
                merged.push(group);
            }
            continue;
        }
        merged.push(group.clone());
    }

    if !replaced {
        merged.push(replacement);
    }

    Value::Array(merged)
}

fn replace_signal_light_hooks(
    group: &Value,
    spec: &AgentSpec,
    replacement: &Value,
) -> (Option<Value>, Option<Value>, bool) {
    let Some(group_object) = group.as_object() else {
        return (None, Some(group.clone()), false);
    };
    let Some(hooks) = group_object.get("hooks").and_then(Value::as_array) else {
        return (None, Some(group.clone()), false);
    };
    let replacement_hooks = replacement
        .get("hooks")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut updated_hooks = Vec::new();
    let mut kept_hooks = Vec::new();
    let mut replaced = false;
    for hook in hooks {
        let is_signal_light = hook
            .as_object()
            .filter(|hook| hook.get("type").and_then(Value::as_str) == Some("command"))
            .and_then(|hook| hook.get("command"))
            .is_some_and(|command| is_signal_light_command(command, spec));
        if is_signal_light {
            if !replaced {
                updated_hooks.extend(replacement_hooks.clone());
                replaced = true;
            }
            continue;
        }
        kept_hooks.push(hook.clone());
        updated_hooks.push(hook.clone());
    }

    if !replaced {
        return (None, Some(group.clone()), false);
    }

    let mut replacement_group = group_object.clone();
    if let Some(matcher) = replacement.get("matcher") {
        replacement_group.insert("matcher".to_string(), matcher.clone());
    }

    if kept_hooks.is_empty() {
        replacement_group.insert("hooks".to_string(), Value::Array(replacement_hooks));
        return (Some(Value::Object(replacement_group)), None, true);
    }

    replacement_group.insert("hooks".to_string(), Value::Array(updated_hooks));

    let mut cleaned_group = group_object.clone();
    cleaned_group.insert("hooks".to_string(), Value::Array(kept_hooks));

    (
        Some(Value::Object(replacement_group)),
        Some(Value::Object(cleaned_group)),
        true,
    )
}

fn is_signal_light_command(command: &Value, spec: &AgentSpec) -> bool {
    let Some(command) = command.as_str() else {
        return false;
    };
    if command.trim().is_empty() {
        return false;
    }
    let parts = split_command_words(command);
    let Some(executable) = parts.first() else {
        return false;
    };
    let executable = Path::new(executable);
    executable.file_name() == spec.hook_script.file_name()
        && executable
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "scripts")
}

fn split_command_words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars();
    let mut quoted_by = None;

    while let Some(ch) = chars.next() {
        match quoted_by {
            Some('\'') => {
                if ch == '\'' {
                    quoted_by = None;
                } else {
                    current.push(ch);
                }
            }
            Some('"') => {
                if ch == '"' {
                    quoted_by = None;
                } else if ch == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else {
                    current.push(ch);
                }
            }
            None => match ch {
                '\'' | '"' => quoted_by = Some(ch),
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                value if value.is_whitespace() => {
                    if !current.is_empty() {
                        words.push(std::mem::take(&mut current));
                    }
                }
                value => current.push(value),
            },
            Some(_) => {}
        }
    }

    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn hook_group(spec: &AgentSpec, event: &str, timeout: u64) -> Value {
    let mut group = Map::new();
    group.insert(
        "hooks".to_string(),
        Value::Array(vec![Value::Object(Map::from_iter([
            ("type".to_string(), Value::String("command".to_string())),
            (
                "command".to_string(),
                Value::String(hook_command(spec, event)),
            ),
            (
                "timeout".to_string(),
                Value::Number(serde_json::Number::from(timeout)),
            ),
        ]))]),
    );
    if spec.uses_matcher {
        group.insert("matcher".to_string(), Value::String(String::new()));
    }
    Value::Object(group)
}

fn hook_command(spec: &AgentSpec, event: &str) -> String {
    let script = shell_quote(spec.hook_script.to_string_lossy().as_ref());
    if spec.passes_event_arg {
        format!("{script} {event}")
    } else {
        script
    }
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "_@%+=:,./-".contains(ch))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use serde_json::{Map, Value};
    use tempfile::tempdir;

    use super::{
        hook_command, inspect_agent, install_agent, native_runtime_repair_hint, run_install_wizard,
        supported_agents, AgentSpec,
    };

    #[test]
    fn supported_agents_exposes_codex_and_claude_code() {
        let tempdir = tempdir().unwrap();
        let agents = supported_agents(Some(tempdir.path())).unwrap();

        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].key, "codex");
        assert_eq!(
            agents[0].config_path,
            tempdir.path().join(".codex").join("hooks.json")
        );
        assert_eq!(agents[1].key, "claude-code");
        assert_eq!(
            agents[1].config_path,
            tempdir.path().join(".claude").join("settings.json")
        );
    }

    #[test]
    fn inspect_agent_marks_missing_config_as_needing_install() {
        let tempdir = tempdir().unwrap();
        let agents = supported_agents(Some(tempdir.path())).unwrap();

        let status = inspect_agent(&agents[0]).unwrap();

        assert!(!status.installed);
        assert_eq!(status.message, "config missing");
    }

    #[test]
    fn install_agent_writes_codex_hooks_and_backups_existing_file() {
        let tempdir = tempdir().unwrap();
        let spec = supported_agents(Some(tempdir.path())).unwrap().remove(0);
        fs::create_dir_all(spec.config_path.parent().unwrap()).unwrap();
        let existing_hook = serde_json::json!({
            "hooks": [{"type": "command", "command": "echo keep-me", "timeout": 1}]
        });
        fs::write(
            &spec.config_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "hooks": {"Stop": [existing_hook.clone()]}
            }))
            .unwrap()
                + "\n",
        )
        .unwrap();

        let result = install_agent(&spec, true).unwrap();

        assert!(result.status.installed);
        assert!(result.backup_path.is_some());
        let data =
            serde_json::from_str::<Value>(&fs::read_to_string(&spec.config_path).unwrap()).unwrap();
        let hooks = data["hooks"].as_object().unwrap();
        assert_eq!(
            hooks.keys().cloned().collect::<Vec<_>>(),
            vec![
                "PermissionRequest".to_string(),
                "PostToolUse".to_string(),
                "PreToolUse".to_string(),
                "SessionEnd".to_string(),
                "SessionStart".to_string(),
                "Stop".to_string(),
                "UserPromptSubmit".to_string(),
            ]
        );
        assert!(data["hooks"]["Stop"]
            .as_array()
            .unwrap()
            .contains(&existing_hook));
    }

    #[test]
    fn install_agent_replaces_existing_signal_light_hooks_but_keeps_other_hooks() {
        let tempdir = tempdir().unwrap();
        let spec = supported_agents(Some(tempdir.path())).unwrap().remove(1);
        fs::create_dir_all(spec.config_path.parent().unwrap()).unwrap();
        let existing = serde_json::json!({
            "hooks": {
                "Stop": [
                    {
                        "hooks": [{
                            "type": "command",
                            "command": spec.hook_script,
                            "timeout": 1
                        }],
                        "matcher": ""
                    },
                    {
                        "hooks": [{"type": "command", "command": "echo keep-me", "timeout": 1}],
                        "matcher": ""
                    }
                ]
            }
        });
        fs::write(
            &spec.config_path,
            serde_json::to_string_pretty(&existing).unwrap() + "\n",
        )
        .unwrap();

        install_agent(&spec, true).unwrap();

        let data =
            serde_json::from_str::<Value>(&fs::read_to_string(&spec.config_path).unwrap()).unwrap();
        let stop_groups = data["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop_groups.len(), 2);
        assert_eq!(
            stop_groups[0]["hooks"][0]["command"],
            Value::String(spec.hook_script.display().to_string())
        );
        assert_eq!(stop_groups[0]["hooks"][0]["timeout"], 5);
        assert_eq!(stop_groups[1]["hooks"][0]["command"], "echo keep-me");
    }

    #[test]
    fn install_agent_preserves_existing_hook_order_when_repairing() {
        let tempdir = tempdir().unwrap();
        let spec = supported_agents(Some(tempdir.path())).unwrap().remove(1);
        fs::create_dir_all(spec.config_path.parent().unwrap()).unwrap();
        let existing = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "hooks": [
                        {"type": "command", "command": "echo before", "timeout": 1},
                        {"type": "command", "command": spec.hook_script, "timeout": 1},
                        {"type": "command", "command": "echo after", "timeout": 1}
                    ],
                    "matcher": ""
                }]
            }
        });
        fs::write(
            &spec.config_path,
            serde_json::to_string_pretty(&existing).unwrap() + "\n",
        )
        .unwrap();

        install_agent(&spec, true).unwrap();

        let data =
            serde_json::from_str::<Value>(&fs::read_to_string(&spec.config_path).unwrap()).unwrap();
        let hooks = data["hooks"]["Stop"][0]["hooks"].as_array().unwrap();
        assert_eq!(
            hooks
                .iter()
                .map(|hook| hook["command"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec![
                "echo before",
                spec.hook_script.to_string_lossy().as_ref(),
                "echo after"
            ]
        );
        assert_eq!(hooks[1]["timeout"], 5);
    }

    #[test]
    fn inspect_agent_marks_wrong_timeout_as_broken() {
        let tempdir = tempdir().unwrap();
        let spec = supported_agents(Some(tempdir.path())).unwrap().remove(0);
        fs::create_dir_all(spec.config_path.parent().unwrap()).unwrap();
        let hooks = spec
            .events
            .iter()
            .map(|(event, _)| {
                (
                    (*event).to_string(),
                    Value::Array(vec![serde_json::json!({
                        "hooks": [{
                            "type": "command",
                            "command": format!("{} {}", spec.hook_script.display(), event),
                            "timeout": 5
                        }]
                    })]),
                )
            })
            .collect::<Map<_, _>>();
        let mut data = Map::new();
        data.insert("hooks".to_string(), Value::Object(hooks));
        fs::write(
            &spec.config_path,
            serde_json::to_string_pretty(&Value::Object(data)).unwrap() + "\n",
        )
        .unwrap();

        let status = inspect_agent(&spec).unwrap();

        assert!(!status.installed);
        assert_eq!(status.broken_events, vec!["PermissionRequest".to_string()]);
    }

    #[test]
    fn hook_command_quotes_paths_with_spaces() {
        let spec = AgentSpec {
            key: "codex",
            name: "Codex",
            config_path: PathBuf::from("/tmp/unused.json"),
            hook_script: PathBuf::from("/tmp/signal light/scripts/codex-signal-hook"),
            events: &[],
            passes_event_arg: true,
            uses_matcher: false,
        };

        let command = hook_command(&spec, "Stop");

        assert_eq!(
            command,
            "'/tmp/signal light/scripts/codex-signal-hook' Stop"
        );
    }

    #[test]
    fn install_wizard_selects_missing_agents_by_default() {
        let tempdir = tempdir().unwrap();
        let agents = supported_agents(Some(tempdir.path())).unwrap();
        fs::create_dir_all(agents[0].config_path.parent().unwrap()).unwrap();
        fs::write(&agents[0].config_path, "{\n  \"hooks\": {}\n}\n").unwrap();

        let mut input = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();
        let exit_code = run_install_wizard(
            &[],
            false,
            true,
            true,
            Some(tempdir.path()),
            &mut input,
            &mut output,
        )
        .unwrap();

        assert_eq!(exit_code, 0);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Would install/repair Codex"));
        assert!(output.contains("Would install/repair Claude Code"));
    }

    #[test]
    fn install_wizard_supports_explicit_agent_selection() {
        let tempdir = tempdir().unwrap();
        let mut input = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();
        let exit_code = run_install_wizard(
            &["codex".to_string()],
            false,
            true,
            true,
            Some(tempdir.path()),
            &mut input,
            &mut output,
        )
        .unwrap();

        assert_eq!(exit_code, 0);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Would install/repair Codex"));
        assert!(!output.contains("Would install/repair Claude Code"));
    }

    #[test]
    fn install_wizard_prints_native_runtime_repair_hint() {
        let tempdir = tempdir().unwrap();
        let mut input = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();
        run_install_wizard(
            &["codex".to_string()],
            false,
            true,
            true,
            Some(tempdir.path()),
            &mut input,
            &mut output,
        )
        .unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Wrappers require the native runtime."));
        assert!(output.contains("SIGNAL_LIGHT_NATIVE_BIN"));
        assert_eq!(native_runtime_repair_hint(), native_runtime_repair_hint());
    }

    #[test]
    fn project_root_prefers_explicit_environment_override() {
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let _guard = ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
        let tempdir = tempdir().unwrap();
        std::env::set_var(super::PROJECT_ROOT_ENV, tempdir.path());
        let resolved = super::project_root();
        std::env::remove_var(super::PROJECT_ROOT_ENV);
        assert_eq!(resolved, tempdir.path());
    }

    #[test]
    fn release_layout_root_detects_packaged_archive_layout() {
        let tempdir = tempdir().unwrap();
        let package_root = tempdir.path().join("signal-light-v0.1.1-macos-aarch64");
        let bin_dir = package_root.join("bin");
        let scripts_dir = package_root.join("scripts");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::create_dir_all(&scripts_dir).unwrap();
        fs::write(bin_dir.join("signal-light-native"), "").unwrap();
        fs::write(scripts_dir.join("codex-signal-hook"), "").unwrap();
        fs::write(scripts_dir.join("claude-code-signal-hook"), "").unwrap();

        let fake_exe = bin_dir.join("signal-light-native");
        assert_eq!(
            super::release_layout_root_from_executable(Path::new(&fake_exe)),
            Some(package_root)
        );
    }
}
