# CLI Interface Contract

## Scope

This contract defines the user-visible command behavior that must remain stable throughout the runtime migration. Wrapper paths may change internally, but documented command names, signal semantics, and exit behavior must remain compatible.

## Command Families

| Command family | Purpose | Success output | Error behavior |
| --- | --- | --- | --- |
| `signal-light list` | Show available signal names and meanings | Human-readable signal list | Exit non-zero with concise error if command cannot initialize |
| `signal-light status` | Report aggregate and display state | Machine-readable JSON snapshot | Exit non-zero with runtime or parse error |
| `signal-light play <signal>` | Request a direct display signal | Optional human-readable confirmation unless quiet | Exit non-zero for unknown signal or runtime failure |
| `signal-light play <signal> --dry-run` | Preview signal without hardware | Human-readable logical or brightness frames | Exit non-zero for invalid signal |
| `signal-light codex-hook` | Consume Codex hook input | Normally quiet success | Exit non-zero for invalid input or runtime failure |
| `signal-light claude-code-hook` | Consume Claude Code hook input | Normally quiet success | Exit non-zero for invalid input or runtime failure |
| `signal-light install-hooks` | Install or repair local hook config | Human-readable install summary | Exit non-zero for invalid selection or write failure |
| `signal-light test` | Validate hardware output order | Human-readable or visible test sequence | Exit non-zero for hardware access failure |
| `signal-light server` | Run the persistent local runtime | No interactive output required on success | Exit non-zero if runtime cannot start |

## Required Compatibility Rules

1. Existing wrapper commands in `scripts/` remain the supported invocation path during migration.
2. Signal names remain stable: `idle`, `thinking`, `working`, `tool_done`, `attention`, `permission`, `blocked`, `done`, `session_start`, `session_end`, `session_done`, and `off`.
3. `status` remains JSON-shaped and includes aggregate state, display state, and active session records.
4. `play`, hook commands, and `test` retain the current exit-code contract:
   - `0` for success
   - `1` for runtime, hardware, or execution failure
   - `2` for invalid usage or unsupported signal or selection
5. Quiet hook invocations must stay concise enough for use inside agent hook logs.

## Status Output Contract

The `status` command must emit a JSON object compatible with `status-output-schema.json` and contain at least:

| Field | Description |
| --- | --- |
| `aggregate` | Current prioritized session-derived signal |
| `display_signal` | Signal currently rendered after overrides or notices |
| `sessions` | Object keyed by session key with signal, update time, and optional owner metadata |

Additional diagnostic fields are allowed if they do not break existing consumers of the required keys.

## Wrapper Compatibility Contract

- The documented command path remains `./scripts/signal-light`.
- The wrapper must invoke a native binary internally.
- The documented wrapper control is `SIGNAL_LIGHT_NATIVE_BIN` for an explicit native binary.
- If the native runtime path is unavailable, the wrapper must fail with a concise repair instruction that tells the user how to build or point at `signal-light-native`.

## Performance Contract

- Warm hook commands must return within 250 ms when the runtime is already available.
- First visible transition after a valid accepted signal must occur within 1 second.
- Startup must either succeed or fail clearly within 3 seconds.
