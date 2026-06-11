use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::config::RuntimeConfig;
use crate::drivers::{create_driver, ChannelLevels, LightDriver};
use crate::error::{Result, SignalLightError};
use crate::model::{Frame, RuntimeCommand, RuntimeResponse, RuntimeSnapshot};
use crate::runtime::commands::{self, apply_direct_signal, apply_session_signal};
use crate::runtime::ipc::{acquire_process_lock, create_request_pipe};
use crate::runtime::session_store::SessionStore;
use crate::signals;

pub fn run(config: RuntimeConfig, dry_run: bool) -> Result<()> {
    let store = SessionStore::new(config.clone());
    store.ensure_state_dir()?;
    let _process_lock = acquire_process_lock(&config)?;
    create_request_pipe(&config.state.server_socket_file)?;
    store.update_runtime_pid(Some(std::process::id()))?;

    let snapshot = store.read_snapshot()?;
    let mut driver = create_driver(&config, dry_run)?;
    let mut display =
        DisplayController::new(snapshot.clone(), 1.0, config.timing.idle_sleep_seconds);
    display.render_now(driver.as_mut())?;

    let request_pipe = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&config.state.server_socket_file)?;
    let (sender, receiver) = mpsc::channel::<String>();
    thread::spawn(move || {
        let mut reader = BufReader::new(request_pipe);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(read) if read > 0 => {
                    let _ = sender.send(line);
                }
                Ok(_) => thread::sleep(Duration::from_millis(10)),
                Err(_) => break,
            }
        }
    });

    loop {
        while let Ok(line) = receiver.try_recv() {
            handle_line(&line, &store, &mut display, driver.as_mut())?;
        }

        let (snapshot, changed) = store.reconcile()?;
        if changed {
            display.set_snapshot(snapshot);
            display.render_now(driver.as_mut())?;
        }
        if let Some(display_signal) = display.tick(driver.as_mut())? {
            let _ = store.update_display_signal(&display_signal);
        }

        thread::sleep(Duration::from_millis(
            display
                .next_sleep_millis()
                .min(config.timing.server_poll_millis),
        ));
    }
}

fn handle_line(
    line: &str,
    store: &SessionStore,
    display: &mut DisplayController,
    driver: &mut dyn LightDriver,
) -> Result<()> {
    let request = serde_json::from_str::<RuntimeCommand>(line)
        .map_err(|_| SignalLightError::InvalidRequest("Invalid request payload.".to_string()));
    let (reply_to, response) = match request {
        Ok(request) => {
            let reply_to = request.reply_to.clone();
            (reply_to, handle_request(store, display, driver, request))
        }
        Err(error) => (None, error_response(error)),
    };

    if let Some(reply_to) = reply_to {
        write_response(Path::new(&reply_to), &response)?;
    }
    Ok(())
}

fn handle_request(
    store: &SessionStore,
    display: &mut DisplayController,
    driver: &mut dyn LightDriver,
    request: RuntimeCommand,
) -> RuntimeResponse {
    match request.action.as_str() {
        "status" => match commands::read_status(store) {
            Ok(snapshot) => response_from_snapshot(snapshot, None),
            Err(error) => error_response(error),
        },
        "direct_signal" => match request.signal_name.as_deref() {
            Some(signal_name) => match apply_direct_signal(store, signal_name) {
                Ok(snapshot) => {
                    display.set_speed(request.speed.unwrap_or(1.0));
                    display.set_snapshot(snapshot.clone());
                    if let Err(error) = display.render_now(driver) {
                        return error_response(error);
                    }
                    response_from_snapshot(snapshot, Some(signal_name.to_string()))
                }
                Err(error) => error_response(error),
            },
            None => error_response(SignalLightError::InvalidRequest(
                "Missing signal name.".to_string(),
            )),
        },
        "session_signal" => match (
            request.session_key.as_deref(),
            request.signal_name.as_deref(),
        ) {
            (Some(session_key), Some(signal_name)) => {
                match apply_session_signal(store, session_key, signal_name, request.owner_pid) {
                    Ok(result) => {
                        display.set_speed(request.speed.unwrap_or(1.0));
                        display.set_snapshot(result.snapshot.clone());
                        if let Err(error) = display.render_now(driver) {
                            return error_response(error);
                        }
                        response_from_snapshot(result.snapshot, Some(signal_name.to_string()))
                    }
                    Err(error) => error_response(error),
                }
            }
            _ => error_response(SignalLightError::InvalidRequest(
                "Missing session key or signal name.".to_string(),
            )),
        },
        _ => error_response(SignalLightError::InvalidRequest(format!(
            "Unsupported action: {}",
            request.action
        ))),
    }
}

fn response_from_snapshot(snapshot: RuntimeSnapshot, signal: Option<String>) -> RuntimeResponse {
    RuntimeResponse {
        ok: true,
        error: None,
        signal,
        aggregate: Some(snapshot.aggregate),
        display_signal: Some(snapshot.display_signal),
        sessions: snapshot.sessions,
        runtime_pid: snapshot.runtime_pid,
        updated_at: snapshot.updated_at,
    }
}

fn error_response(error: SignalLightError) -> RuntimeResponse {
    RuntimeResponse {
        ok: false,
        error: Some(error.to_string()),
        signal: None,
        aggregate: None,
        display_signal: None,
        sessions: Default::default(),
        runtime_pid: None,
        updated_at: None,
    }
}

fn write_response(path: &Path, response: &RuntimeResponse) -> Result<()> {
    let payload = serde_json::to_string(response)? + "\n";
    fs::write(path, payload)?;
    Ok(())
}

#[derive(Debug, Clone)]
struct DisplayController {
    snapshot: RuntimeSnapshot,
    mode: String,
    speed_factor: f32,
    idle_sleep_seconds: u64,
    frame_index: usize,
    next_frame_at: Instant,
    notice_until: Option<Instant>,
    idle_since: Instant,
    steady_rendered: bool,
}

impl DisplayController {
    fn new(snapshot: RuntimeSnapshot, speed_factor: f32, idle_sleep_seconds: u64) -> Self {
        let now = Instant::now();
        Self {
            mode: snapshot.display_signal.clone(),
            snapshot,
            speed_factor,
            idle_sleep_seconds,
            frame_index: 0,
            next_frame_at: now,
            notice_until: None,
            idle_since: now,
            steady_rendered: false,
        }
    }

    fn set_speed(&mut self, speed_factor: f32) {
        self.speed_factor = speed_factor.max(0.05);
    }

    fn set_snapshot(&mut self, snapshot: RuntimeSnapshot) {
        self.snapshot = snapshot;
        self.mode = self.snapshot.display_signal.clone();
        self.frame_index = 0;
        self.next_frame_at = Instant::now();
        self.notice_until = if self.mode == signals::SESSION_END_NOTICE_SIGNAL {
            signals::signal(signals::SESSION_END_NOTICE_SIGNAL).map(|signal| {
                Instant::now() + Duration::from_millis(signal.duration_ms(self.speed_factor))
            })
        } else {
            None
        };
        self.idle_since = Instant::now();
        self.steady_rendered = false;
    }

    fn render_now(&mut self, driver: &mut dyn LightDriver) -> Result<()> {
        self.render_current(driver)
    }

    fn tick(&mut self, driver: &mut dyn LightDriver) -> Result<Option<String>> {
        if self.mode == signals::SESSION_END_NOTICE_SIGNAL {
            if let Some(notice_until) = self.notice_until {
                if Instant::now() >= notice_until {
                    self.mode = self.snapshot.aggregate.clone();
                    self.snapshot.display_signal = self.snapshot.aggregate.clone();
                    self.frame_index = 0;
                    self.next_frame_at = Instant::now();
                    self.notice_until = None;
                    self.steady_rendered = false;
                    self.render_current(driver)?;
                    return Ok(Some(self.snapshot.aggregate.clone()));
                }
            }
        }

        if self.mode == "idle"
            && self.idle_since.elapsed() >= Duration::from_secs(self.idle_sleep_seconds)
        {
            driver.off()?;
            self.mode = "off".to_string();
            self.snapshot.display_signal = "off".to_string();
            self.steady_rendered = true;
            return Ok(Some("off".to_string()));
        }

        if Instant::now() >= self.next_frame_at {
            self.render_current(driver)?;
        }
        Ok(None)
    }

    fn next_sleep_millis(&self) -> u64 {
        let now = Instant::now();
        if self.mode == "idle" {
            let idle_deadline = self.idle_since + Duration::from_secs(self.idle_sleep_seconds);
            return idle_deadline
                .saturating_duration_since(now)
                .as_millis()
                .max(50) as u64;
        }
        self.next_frame_at
            .saturating_duration_since(now)
            .as_millis()
            .max(50) as u64
    }

    fn render_current(&mut self, driver: &mut dyn LightDriver) -> Result<()> {
        if self.mode == "off" {
            self.steady_rendered = true;
            driver.off()?;
            return Ok(());
        }

        let signal = signals::signal(&self.mode).ok_or_else(|| {
            SignalLightError::Runtime(format!("Unknown display signal: {}", self.mode))
        })?;

        if signal.repeat || self.mode == signals::SESSION_END_NOTICE_SIGNAL {
            let frame = &signal.frames[self.frame_index % signal.frames.len()];
            render_frame(driver, frame)?;
            let duration_ms = ((frame.duration_ms as f32) * self.speed_factor.max(0.05)) as u64;
            self.frame_index += 1;
            self.next_frame_at = Instant::now() + Duration::from_millis(duration_ms.max(1));
            self.steady_rendered = false;
            return Ok(());
        }

        if !self.steady_rendered {
            if let Some(state) = signal.steady_state {
                driver.write_levels(ChannelLevels::from_state(state))?;
            } else {
                driver.off()?;
            }
            self.steady_rendered = true;
        }
        self.next_frame_at = Instant::now() + Duration::from_millis(250);
        Ok(())
    }
}

fn render_frame(driver: &mut dyn LightDriver, frame: &Frame) -> Result<()> {
    let mut levels = ChannelLevels::from_frame(frame);
    if !driver.supports_brightness() {
        levels = ChannelLevels {
            green: if levels.green > 0.0 { 1.0 } else { 0.0 },
            yellow: if levels.yellow > 0.0 { 1.0 } else { 0.0 },
            red: if levels.red > 0.0 { 1.0 } else { 0.0 },
        };
    }
    if levels.green == 0.0 && levels.yellow == 0.0 && levels.red == 0.0 {
        driver.off()
    } else {
        driver.write_levels(levels)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::RuntimeConfig;
    use crate::drivers::{ChannelLevels, LightDriver};
    use crate::model::RuntimeSnapshot;

    use super::{handle_request, DisplayController};

    struct RecordingDriver {
        frames: Vec<ChannelLevels>,
    }

    impl RecordingDriver {
        fn new() -> Self {
            Self { frames: Vec::new() }
        }
    }

    impl LightDriver for RecordingDriver {
        fn write_levels(&mut self, levels: ChannelLevels) -> crate::error::Result<()> {
            self.frames.push(levels);
            Ok(())
        }
    }

    fn idle_snapshot() -> RuntimeSnapshot {
        RuntimeSnapshot {
            aggregate: "idle".to_string(),
            display_signal: "idle".to_string(),
            sessions: Default::default(),
            runtime_pid: None,
            updated_at: None,
        }
    }

    #[test]
    fn direct_signal_request_updates_display_speed_factor() {
        let tempdir = tempfile::tempdir().unwrap();
        let config = RuntimeConfig::from_pairs([(
            "SIGNAL_LIGHT_STATE_DIR".to_string(),
            tempdir.path().to_string_lossy().to_string(),
        )])
        .unwrap();
        let store = crate::runtime::session_store::SessionStore::new(config);
        let mut display = DisplayController::new(idle_snapshot(), 1.0, 600);
        let mut driver = RecordingDriver::new();

        let response = handle_request(
            &store,
            &mut display,
            &mut driver,
            crate::model::RuntimeCommand {
                action: "direct_signal".to_string(),
                session_key: None,
                signal_name: Some("working".to_string()),
                owner_pid: None,
                speed: Some(0.05),
                reply_to: None,
            },
        );

        assert!(response.ok);
        assert_eq!(display.speed_factor, 0.05);
        assert!(!driver.frames.is_empty());
    }
}
