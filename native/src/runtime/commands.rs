use crate::error::{Result, SignalLightError};
use crate::model::{RuntimeSnapshot, SessionRecord};
use crate::runtime::session_store::{
    aggregate_for_sessions, unix_timestamp, SessionStore, OWNER_PID_SOURCE,
};
use crate::signals;

#[derive(Debug, Clone)]
pub struct ApplySessionResult {
    pub snapshot: RuntimeSnapshot,
    pub show_notice: bool,
}

pub fn read_status(store: &SessionStore) -> Result<RuntimeSnapshot> {
    store.read_snapshot()
}

pub fn apply_direct_signal(store: &SessionStore, signal_name: &str) -> Result<RuntimeSnapshot> {
    if !signals::is_public_signal(signal_name) {
        return Err(SignalLightError::InvalidSignal(format!(
            "Unknown direct signal: {signal_name}"
        )));
    }

    let mut state = store.read_state()?;
    store.prune_state(&mut state);
    if matches!(signal_name, "idle" | "off") {
        state.sessions.clear();
    }
    state.direct_signal = Some(signal_name.to_string());
    state.display_signal = Some(signal_name.to_string());
    let snapshot = store.snapshot_from_state(&state);
    store.write_state(&state)?;
    Ok(snapshot)
}

pub fn apply_session_signal(
    store: &SessionStore,
    session_key: &str,
    signal_name: &str,
    owner_pid: Option<u32>,
) -> Result<ApplySessionResult> {
    if session_key.trim().is_empty() {
        return Err(SignalLightError::InvalidRequest(
            "Missing session key.".to_string(),
        ));
    }
    if !(signals::is_public_signal(signal_name) || signals::is_hook_control_signal(signal_name)) {
        return Err(SignalLightError::InvalidSignal(format!(
            "Unknown session signal: {signal_name}"
        )));
    }

    let mut state = store.read_state()?;
    store.prune_state(&mut state);

    let current = state.sessions.get(session_key).cloned();
    let mut show_notice = false;
    let mut clear_direct_override = false;

    match signal_name {
        "session_end" => {
            show_notice = true;
            clear_direct_override = true;
            state.sessions.remove(session_key);
        }
        "off" => {
            clear_direct_override = state.sessions.remove(session_key).is_some();
        }
        name if name == signals::TURN_END_SIGNAL => {
            let current_signal = current.as_ref().map(|record| record.signal.as_str());
            if !matches!(current_signal, Some("permission") | Some("blocked")) {
                show_notice = true;
                clear_direct_override = true;
                state.sessions.remove(session_key);
            }
        }
        _ => {
            clear_direct_override = true;
            let inherited_owner_pid = current
                .as_ref()
                .filter(|record| record.owner_pid_source.as_deref() == Some(OWNER_PID_SOURCE))
                .and_then(|record| record.owner_pid);
            state.sessions.insert(
                session_key.to_string(),
                SessionRecord {
                    signal: signal_name.to_string(),
                    updated_at: unix_timestamp(),
                    owner_pid: owner_pid.or(inherited_owner_pid),
                    owner_pid_source: owner_pid
                        .or(inherited_owner_pid)
                        .map(|_| OWNER_PID_SOURCE.to_string()),
                },
            );
        }
    }

    if clear_direct_override {
        state.direct_signal = None;
    }

    let aggregate = aggregate_for_sessions(&state);
    let display_signal =
        if show_notice && !matches!(aggregate.as_str(), "blocked" | "permission" | "attention") {
            signals::SESSION_END_NOTICE_SIGNAL.to_string()
        } else {
            aggregate.clone()
        };
    state.display_signal = Some(display_signal);

    let snapshot = store.snapshot_from_state(&state);
    store.write_state(&state)?;
    Ok(ApplySessionResult {
        snapshot,
        show_notice,
    })
}
