"""Runtime process management for persistent signal-light states."""

from __future__ import annotations

import json
import os
import shlex
import signal
import subprocess
import sys
import time
from contextlib import contextmanager
from hashlib import sha256
from pathlib import Path
from typing import Iterator

from signal_light.agent_signals import AgentSignal, SIGNALS
from signal_light.hardware import LightMapping, SignalLight, SignalLightError


STATE_DIR = Path(os.environ.get("SIGNAL_LIGHT_STATE_DIR", "/tmp/signal-light"))
PROJECT_ROOT = Path(__file__).resolve().parents[1]
PID_FILE = STATE_DIR / "worker.json"
LOG_FILE = STATE_DIR / "worker.log"
NOTICE_PID_FILE = STATE_DIR / "notice-worker.json"
NOTICE_LOG_FILE = STATE_DIR / "notice-worker.log"
SLEEP_PID_FILE = STATE_DIR / "sleep-worker.json"
SLEEP_LOG_FILE = STATE_DIR / "sleep-worker.log"
SESSION_FILE = STATE_DIR / "sessions.json"
LOCK_FILE = STATE_DIR / "state.lock"
SESSION_TTL_SECONDS = int(os.environ.get("SIGNAL_LIGHT_SESSION_TTL_SECONDS", "86400"))
IDLE_SLEEP_SECONDS = int(os.environ.get("SIGNAL_LIGHT_IDLE_SLEEP_SECONDS", "600"))

RED_SIGNALS = {"blocked"}
YELLOW_SIGNALS = {"permission", "attention", "done"}
WORKING_SIGNALS = {"thinking", "working", "tool_done"}
SESSION_END_SIGNALS = {"session_end"}
# Explicit clears should not look like session-completion cues.
SESSION_CLEAR_SIGNALS = {"off"}
SESSION_END_NOTICE_SIGNAL = "session_done"
IDLE_SLEEP_SIGNAL = "idle_sleep"
TURN_END_SIGNALS = {"turn_end"}
# Sessions still waiting for user action should survive turn_end.
TURN_END_KEEP_SIGNALS = {"permission", "blocked"}
REPEATING_WORKER_SIGNALS = {name for name, agent_signal in SIGNALS.items() if agent_signal.repeat}


def apply_signal(signal: AgentSignal, *, speed: float = 1.0) -> None:
    """Apply a signal as the current persistent status."""
    stop_notice_worker()
    stop_sleep_worker()
    if signal.repeat:
        if _worker_matches(signal.name):
            return
        stop_worker()
        try:
            start_worker(signal.name, speed=speed)
        except SignalLightError:
            pass
        return

    stop_worker()
    try:
        _play_with_retries(signal, speed=speed)
    except SignalLightError:
        pass
    if signal.name == "idle":
        start_sleep_worker()


def apply_signal_now(signal: AgentSignal, *, speed: float = 1.0) -> None:
    """Apply a signal without stopping any in-flight notice worker."""
    stop_sleep_worker()
    if signal.repeat:
        if _worker_matches(signal.name):
            return
        stop_worker()
        try:
            start_worker(signal.name, speed=speed)
        except SignalLightError:
            pass
        return

    stop_worker()
    try:
        _play_with_retries(signal, speed=speed)
    except SignalLightError:
        pass
    if signal.name == "idle":
        start_sleep_worker()


def apply_session_signal(session_key: str, signal_name: str, *, speed: float = 1.0) -> str:
    """Update one Codex session state, then apply the aggregated global state."""
    with _state_lock():
        state = _read_session_state()
        sessions = state.setdefault("sessions", {})
        now = time.time()
        _prune_sessions(sessions, now)
        should_show_session_end_notice = False

        if signal_name in SESSION_END_SIGNALS:
            should_show_session_end_notice = session_key in sessions
            sessions.pop(session_key, None)
        elif signal_name in SESSION_CLEAR_SIGNALS:
            sessions.pop(session_key, None)
        elif signal_name in TURN_END_SIGNALS:
            current = sessions.get(session_key)
            current_signal = current.get("signal") if isinstance(current, dict) else None
            if current_signal not in TURN_END_KEEP_SIGNALS:
                should_show_session_end_notice = session_key in sessions
                sessions.pop(session_key, None)
        else:
            sessions[session_key] = {
                "signal": signal_name,
                "updated_at": now,
            }

        aggregate = aggregate_sessions(sessions)
        _write_session_state(state)
        if should_show_session_end_notice:
            apply_session_end_notice(aggregate, speed=speed)
        else:
            apply_signal(SIGNALS[aggregate], speed=speed)
        return aggregate


def apply_session_end_notice(aggregate: str, *, speed: float = 1.0) -> None:
    """Briefly acknowledge a completed session, then restore the aggregate state."""
    if aggregate in RED_SIGNALS or aggregate in YELLOW_SIGNALS:
        apply_signal(SIGNALS[aggregate], speed=speed)
        return

    start_notice_worker(speed=speed)


def start_notice_worker(*, speed: float = 1.0) -> None:
    stop_notice_worker()
    _spawn_worker_process(
        signal_name=SESSION_END_NOTICE_SIGNAL,
        speed=speed,
        pid_file=NOTICE_PID_FILE,
        log_file=NOTICE_LOG_FILE,
        verify_startup=False,
    )


def start_sleep_worker() -> None:
    stop_sleep_worker()
    _spawn_worker_process(
        signal_name=IDLE_SLEEP_SIGNAL,
        speed=1.0,
        pid_file=SLEEP_PID_FILE,
        log_file=SLEEP_LOG_FILE,
        verify_startup=False,
    )


def stop_sleep_worker() -> None:
    _stop_worker_process(pid_file=SLEEP_PID_FILE, orphan_signal_names={IDLE_SLEEP_SIGNAL})


def run_idle_sleep_worker() -> int:
    """Wait for IDLE_SLEEP_SECONDS, then turn off the lights if still idle."""
    try:
        time.sleep(IDLE_SLEEP_SECONDS)
        with _state_lock():
            if not _worker_pid_matches(SLEEP_PID_FILE, os.getpid()):
                return 0
            snapshot = _read_session_snapshot_unlocked()
            if snapshot["aggregate"] != "idle":
                return 0
            stop_worker()
            _play_with_retries(SIGNALS["off"], speed=1.0)
        return 0
    finally:
        _clear_worker_pid_file(SLEEP_PID_FILE, expected_pid=os.getpid())


def clear_session_state() -> None:
    """Clear all tracked Codex session states."""
    with _state_lock():
        _write_session_state({"sessions": {}})


def aggregate_sessions(sessions: dict[str, object]) -> str:
    signals = []
    for value in sessions.values():
        if isinstance(value, dict):
            signal_name = value.get("signal")
            if isinstance(signal_name, str):
                signals.append(signal_name)

    if any(signal_name in RED_SIGNALS for signal_name in signals):
        return "blocked"
    if any(signal_name == "permission" for signal_name in signals):
        return "permission"
    if any(signal_name in YELLOW_SIGNALS for signal_name in signals):
        return "attention"
    if any(signal_name in WORKING_SIGNALS for signal_name in signals):
        return "working"
    return "idle"


def read_session_snapshot() -> dict[str, object]:
    with _state_lock():
        return _read_session_snapshot_unlocked()


def _read_session_snapshot_unlocked() -> dict[str, object]:
    state = _read_session_state()
    sessions = state.get("sessions", {})
    if not isinstance(sessions, dict):
        sessions = {}
    now = time.time()
    _prune_sessions(sessions, now)
    aggregate = aggregate_sessions(sessions)
    return {
        "aggregate": aggregate,
        "sessions": sessions,
    }


def run_worker(signal_name: str, *, speed: float = 1.0) -> int:
    if signal_name == SESSION_END_NOTICE_SIGNAL:
        return run_session_end_notice_worker(speed=speed)
    if signal_name == IDLE_SLEEP_SIGNAL:
        return run_idle_sleep_worker()

    signal_to_run = SIGNALS[signal_name]
    if not signal_to_run.repeat:
        raise SignalLightError(f"Signal {signal_name} is not a repeating signal.")

    with SignalLight(LightMapping.from_env(os.environ)) as light:
        signal_to_run.play_forever(light, speed=speed)
    return 0


def run_session_end_notice_worker(*, speed: float = 1.0) -> int:
    notice_signal = SIGNALS[SESSION_END_NOTICE_SIGNAL]
    try:
        stop_worker()
        try:
            with SignalLight(LightMapping.from_env(os.environ)) as light:
                notice_signal.play(light, speed=speed)
        finally:
            _restore_session_end_notice(speed=speed)
        return 0
    finally:
        _clear_worker_pid_file(NOTICE_PID_FILE, expected_pid=os.getpid())


def _restore_session_end_notice(*, speed: float) -> None:
    with _state_lock():
        if not _worker_pid_matches(NOTICE_PID_FILE, os.getpid()):
            return

        aggregate = _read_session_snapshot_unlocked()["aggregate"]
        apply_signal_now(SIGNALS[aggregate], speed=speed)


@contextmanager
def _state_lock() -> Iterator[None]:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    with LOCK_FILE.open("a+") as lock_file:
        try:
            import fcntl

            fcntl.flock(lock_file, fcntl.LOCK_EX)
            yield
        finally:
            try:
                fcntl.flock(lock_file, fcntl.LOCK_UN)
            except Exception:
                pass


def _read_session_state() -> dict[str, object]:
    try:
        state = json.loads(SESSION_FILE.read_text())
    except (FileNotFoundError, json.JSONDecodeError):
        return {"sessions": {}}

    if not isinstance(state, dict):
        return {"sessions": {}}
    if not isinstance(state.get("sessions"), dict):
        state["sessions"] = {}
    return state


def _write_session_state(state: dict[str, object]) -> None:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    SESSION_FILE.write_text(json.dumps(state, ensure_ascii=False, indent=2) + "\n")


def _prune_sessions(sessions: dict[str, object], now: float) -> None:
    expired = []
    for session_key, value in sessions.items():
        if not isinstance(value, dict):
            expired.append(session_key)
            continue
        updated_at = value.get("updated_at")
        if not isinstance(updated_at, (int, float)) or now - updated_at > SESSION_TTL_SECONDS:
            expired.append(session_key)

    for session_key in expired:
        sessions.pop(session_key, None)


def start_worker(signal_name: str, *, speed: float = 1.0) -> None:
    _spawn_worker_process(
        signal_name=signal_name,
        speed=speed,
        pid_file=PID_FILE,
        log_file=LOG_FILE,
        verify_startup=True,
    )


def stop_notice_worker() -> None:
    _stop_worker_process(pid_file=NOTICE_PID_FILE, orphan_signal_names={SESSION_END_NOTICE_SIGNAL})


def _worker_matches(signal_name: str) -> bool:
    state = _read_worker_state(PID_FILE)
    pid = state.get("pid")
    return state.get("signal") == signal_name and isinstance(pid, int) and _is_running(pid)


def _worker_pid_matches(pid_file: Path, expected_pid: int) -> bool:
    return _read_worker_state(pid_file).get("pid") == expected_pid


def _play_with_retries(signal: AgentSignal, *, speed: float) -> None:
    last_error: SignalLightError | None = None
    for _ in range(12):
        try:
            with SignalLight(LightMapping.from_env(os.environ)) as light:
                signal.play(light, speed=speed)
            return
        except SignalLightError as exc:
            last_error = exc
            time.sleep(0.15)

    raise last_error or SignalLightError("Failed to apply signal state.")


def _spawn_worker_process(
    *,
    signal_name: str,
    speed: float,
    pid_file: Path,
    log_file: Path,
    verify_startup: bool = True,
) -> None:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    command = [
        sys.executable,
        "-m",
        "signal_light",
        "worker",
        "--owner-token",
        _worker_owner_token(),
        signal_name,
        "--speed",
        str(speed),
    ]
    log = log_file.open("ab")
    try:
        process = subprocess.Popen(
            command,
            stdin=subprocess.DEVNULL,
            stdout=log,
            stderr=log,
            cwd=Path(__file__).resolve().parents[1],
            env=os.environ.copy(),
            start_new_session=True,
        )
    finally:
        log.close()

    pid_file.write_text(
        json.dumps(
            {
                "pid": process.pid,
                "signal": signal_name,
                "owner_token": _worker_owner_token(),
                "started_at": time.time(),
            },
            ensure_ascii=False,
        )
    )

    if not verify_startup:
        return

    time.sleep(0.2)
    if process.poll() is not None:
        _clear_worker_pid_file(pid_file, expected_pid=process.pid)
        raise SignalLightError(_worker_error_message(signal_name, log_file))


def _worker_error_message(signal_name: str, log_file: Path) -> str:
    detail = ""
    try:
        detail = log_file.read_text(errors="replace").strip().splitlines()[-1]
    except (FileNotFoundError, IndexError):
        pass

    if detail:
        return f"Signal worker for {signal_name} exited immediately: {detail}"
    return f"Signal worker for {signal_name} exited immediately."


def stop_worker() -> None:
    _stop_worker_process(pid_file=PID_FILE, orphan_signal_names=REPEATING_WORKER_SIGNALS)


def _stop_worker_process(*, pid_file: Path, orphan_signal_names: set[str]) -> None:
    state = _read_worker_state(pid_file)
    pid = state.get("pid")
    stopped_pids: set[int] = set()
    if isinstance(pid, int) and pid > 0 and pid != os.getpid():
        _terminate(pid)
        stopped_pids.add(pid)

    for orphan_pid in _find_worker_pids(orphan_signal_names):
        if orphan_pid not in stopped_pids and orphan_pid != os.getpid():
            _terminate(orphan_pid)

    _clear_worker_pid_file(pid_file)


def _clear_worker_pid_file(pid_file: Path, *, expected_pid: int | None = None) -> None:
    if expected_pid is not None:
        state = _read_worker_state(pid_file)
        if state.get("pid") != expected_pid:
            return

    try:
        pid_file.unlink()
    except FileNotFoundError:
        pass


def _read_worker_state(pid_file: Path) -> dict[str, object]:
    try:
        return json.loads(pid_file.read_text())
    except (FileNotFoundError, json.JSONDecodeError):
        return {}


def _find_worker_pids(signal_names: set[str]) -> list[int]:
    try:
        result = subprocess.run(
            ["ps", "-axo", "pid=,command="],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError:
        return []

    if result.returncode != 0:
        return []

    pids: list[int] = []
    for line in result.stdout.splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        parts = stripped.split(None, 1)
        if len(parts) != 2:
            continue
        pid_text, command = parts
        try:
            pid = int(pid_text)
        except ValueError:
            continue

        if pid != os.getpid() and _is_worker_command(command, signal_names):
            pids.append(pid)

    return pids


def _is_worker_command(command: str, signal_names: set[str]) -> bool:
    try:
        parts = shlex.split(command)
    except ValueError:
        parts = command.split()

    for index in range(len(parts) - 2):
        if parts[index : index + 3] != ["-m", "signal_light", "worker"]:
            continue

        worker_args = parts[index + 3 :]
        if _option_value(worker_args, "--owner-token") != _worker_owner_token():
            continue

        if _worker_signal_from_args(worker_args) in signal_names:
            return True
    return False


def _worker_owner_token() -> str:
    identity = f"{PROJECT_ROOT}|{STATE_DIR.expanduser().resolve()}"
    return sha256(identity.encode("utf-8")).hexdigest()[:16]


def _option_value(args: list[str], option: str) -> str | None:
    prefix = f"{option}="
    for index, arg in enumerate(args):
        if arg == option and index + 1 < len(args):
            return args[index + 1]
        if arg.startswith(prefix):
            return arg.removeprefix(prefix)
    return None


def _worker_signal_from_args(args: list[str]) -> str | None:
    skip_next = False
    for arg in args:
        if skip_next:
            skip_next = False
            continue
        if arg in {"--owner-token", "--speed"}:
            skip_next = True
            continue
        if arg.startswith("--owner-token=") or arg.startswith("--speed="):
            continue
        if not arg.startswith("-"):
            return arg
    return None


def _terminate(pid: int) -> None:
    if not _is_running(pid):
        return

    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    except PermissionError as exc:
        raise SignalLightError(f"Cannot stop existing signal worker {pid}: {exc}") from exc

    deadline = time.monotonic() + 1.0
    while time.monotonic() < deadline:
        if not _is_running(pid):
            return
        time.sleep(0.05)

    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        return
    except PermissionError as exc:
        raise SignalLightError(f"Cannot stop existing signal worker {pid}: {exc}") from exc


def _is_running(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True
