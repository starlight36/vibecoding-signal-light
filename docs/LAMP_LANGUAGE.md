# AI Agent Traffic Signal Status Language

This project uses a three-light traffic signal model as an ambient status display for Codex or other AI agents.

The language is deliberately small: the current light must always describe the current state. There are no important startup animations and no "blink first, meaning later" patterns. If Codex is working or needs you, the pattern keeps running until another Codex event changes the state. The one transient cue is session completion: a short green blink can acknowledge that one session ended, then the light returns to the current aggregate state.

## Status Semantics

| Light | Meaning | Human action |
| --- | --- | --- |
| steady green | Codex is idle | Nothing |
| slow green-yellow-red cycle | Codex is thinking, using tools, or otherwise working | Wait |
| flashing yellow | Codex explicitly needs you to read or continue | Look at Codex when convenient |
| flashing red | Codex needs permission, is blocked, or hit a failure | Look at Codex now |
| off | Manual clear | Nothing |

That is the whole language.

## Signal Names

The CLI still exposes named signals so hooks and other agents can use stable words:

| Signal | Light | Meaning |
| --- | --- | --- |
| `idle` | steady green | Agent is idle |
| `thinking` | slow green-yellow-red cycle | Agent has received the prompt and is thinking |
| `working` | slow green-yellow-red cycle | Agent is using tools, editing, running commands, or testing |
| `tool_done` | slow green-yellow-red cycle | A tool call finished, but the agent is still in an active workflow |
| `attention` | flashing yellow | Agent explicitly expects you to read or continue |
| `done` | flashing yellow | Task completed; read the final answer |
| `permission` | flashing red | Codex requests permission |
| `blocked` | flashing red | Agent cannot continue without intervention |
| `session_start` | steady green | Codex session started and is idle |
| `session_end` | brief green completion blink, then aggregate state | Codex session ended |
| `session_done` | brief green blink | Internal completion cue for one ended session |
| `off` | off | Clear all lights |

## Codex Hook Mapping

| Codex event | Signal | Light |
| --- | --- | --- |
| `SessionStart` | `session_start` | steady green |
| `UserPromptSubmit` | `thinking` | slow green-yellow-red cycle |
| `PreToolUse` | `working` | slow green-yellow-red cycle |
| `PostToolUse` | `tool_done` | slow green-yellow-red cycle |
| `PermissionRequest` | `permission` | flashing red |
| `Stop` | `turn_end` | green completion cue, then aggregate state without that session's non-urgent work |
| `SessionEnd` | `session_end` | session cleanup; if still tracked, brief green completion blink, then aggregate state |

`turn_end` is a hook-only control state. It is not a public lamp pattern: it removes that session's non-urgent working state, briefly shows the green completion cue, then restores the aggregate state. Existing `permission` or `blocked` red alerts stay intact.

If the hook payload reports failure through structured fields such as `status`, `state`, `error`, `failure`, `exception`, or a non-zero `exit_status`, the adapter uses `blocked`, which starts flashing the red light.

Codex and Claude Code hook commands act as lightweight clients. They send events to one local Signal Light server process, which owns the global session state, the direct display override from `play`, and the display animation loop. `status` reports both the session aggregate and actual `display_signal`. `Stop` is treated as the end of a normal turn, so it clears that session's working state, shows the green completion cue, and then restores the aggregate result instead of flashing yellow after every response.

The work cycle includes brightness levels for drivers that can dim LEDs. The current MCP2221A GPIO driver uses plain on/off output instead of software PWM, because USB GPIO timing makes simulated dimming visibly flicker.

Codex hook state is session-aware. Each session stores its own latest signal, then the physical light shows the highest-priority aggregate:

```text
flashing red > flashing yellow > green-yellow-red work cycle > steady green
```

For example, if one Codex session is waiting for permission and another session starts working, the light stays flashing red. If one session is waiting for you to read a result and another session is working, the light stays flashing yellow.

When a tracked session or turn ends, the runtime briefly flashes green to make the completion visible. After that cue, it recomputes the aggregate: if other sessions are still working, the green-yellow-red cycle resumes; if no sessions remain, the light settles on steady green. If the light stays idle, the server turns it fully off after `SIGNAL_LIGHT_IDLE_SLEEP_SECONDS` (10 minutes by default). The server also drops sessions whose recorded owner process has exited, so a killed local agent does not leave the lamp stuck in an old state. Red and yellow alerts stay higher priority, so the green completion cue does not interrupt an active permission, blocked, attention, or done state.

## Wiring Defaults

The CLI assumes active-low MCP2221A GPIO wiring:

- `gp0`: green
- `gp1`: yellow
- `gp2`: red
- GPIO `LOW`: light on
- GPIO `HIGH`: light off

Override these with environment variables:

```bash
export SIGNAL_LIGHT_GREEN_PIN=gp0
export SIGNAL_LIGHT_YELLOW_PIN=gp1
export SIGNAL_LIGHT_RED_PIN=gp2
export SIGNAL_LIGHT_ACTIVE_LOW=1
```

Set `SIGNAL_LIGHT_ACTIVE_LOW=0` if your signal model is wired active-high.

## Try It Without Hardware

```bash
./scripts/signal-light list
./scripts/signal-light play working --dry-run
./scripts/signal-light play attention --dry-run
./scripts/signal-light codex-hook PermissionRequest --dry-run
```

## Try It With Hardware

```bash
./scripts/signal-light test
./scripts/signal-light play working
./scripts/signal-light play attention
./scripts/signal-light play permission
./scripts/signal-light play idle
./scripts/signal-light play off
./scripts/signal-light status
```

If the wrong light turns on, adjust `SIGNAL_LIGHT_*_PIN`. If lights are inverted, adjust `SIGNAL_LIGHT_ACTIVE_LOW`.

The wrapper scripts require the Rust native runtime. They use `SIGNAL_LIGHT_NATIVE_BIN` when set, then `bin/signal-light-native`, then `native/target/release/signal-light-native`, then `native/target/debug/signal-light-native`. If no native binary is available, they exit with a concise build instruction. Build the default release binary with `cargo build --manifest-path native/Cargo.toml --release`.

Runtime state is stored in a per-user directory by default: `$XDG_STATE_HOME/signal-light` when set, `~/Library/Application Support/signal-light` on macOS, or `~/.local/state/signal-light` on Linux. Override it with `SIGNAL_LIGHT_STATE_DIR` if you need a custom location.

## Claude Code Hook Mapping

| Claude Code event | Signal | Light |
| --- | --- | --- |
| `SessionStart` | `session_start` | steady green |
| `UserPromptSubmit` | `thinking` | slow green-yellow-red cycle |
| `PreToolUse` | `working` | slow green-yellow-red cycle |
| `PostToolUse` | `tool_done` | slow green-yellow-red cycle |
| `PostToolUseFailure` | `blocked` | flashing red |
| `PreCompact` | `working` | slow green-yellow-red cycle |
| `SubagentStart` | `working` | slow green-yellow-red cycle |
| `SubagentStop` | `tool_done` | slow green-yellow-red cycle |
| `PermissionRequest` | `permission` | flashing red |
| `Notification` | `attention` | flashing yellow |
| `Stop` | `turn_end` | green completion cue, then aggregate state without that session's non-urgent work |
| `SessionEnd` | `session_end` | session cleanup; if still tracked, brief green completion blink, then aggregate state |

If `Stop` carries a `stop_reason` of `max_tokens` or `error`, the adapter uses `blocked` instead of clearing state.

## Claude Code settings.json Example

Add hooks to `~/.claude/settings.json` (or project `.claude/settings.json`):

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/signal-light/scripts/claude-code-signal-hook",
            "timeout": 5
          }
        ],
        "matcher": ""
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/signal-light/scripts/claude-code-signal-hook",
            "timeout": 5
          }
        ],
        "matcher": ""
      }
    ],
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/signal-light/scripts/claude-code-signal-hook",
            "timeout": 5
          }
        ],
        "matcher": ""
      }
    ],
    "PostToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/signal-light/scripts/claude-code-signal-hook",
            "timeout": 5
          }
        ],
        "matcher": ""
      }
    ],
    "PostToolUseFailure": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/signal-light/scripts/claude-code-signal-hook",
            "timeout": 5
          }
        ],
        "matcher": ""
      }
    ],
    "PermissionRequest": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/signal-light/scripts/claude-code-signal-hook",
            "timeout": 10
          }
        ],
        "matcher": ""
      }
    ],
    "Notification": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/signal-light/scripts/claude-code-signal-hook",
            "timeout": 5
          }
        ],
        "matcher": ""
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/signal-light/scripts/claude-code-signal-hook",
            "timeout": 5
          }
        ],
        "matcher": ""
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/signal-light/scripts/claude-code-signal-hook",
            "timeout": 5
          }
        ],
        "matcher": ""
      }
    ]
  }
}
```

Note: Unlike Codex hooks where the event name must be passed as an argument, Claude Code passes the event as JSON on stdin, so the hook command does not need an event argument.

## Codex hooks.json Example

Add command hooks like this to `~/.codex/hooks.json`, keeping any existing hooks you already use:

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/signal-light/scripts/codex-signal-hook UserPromptSubmit",
            "timeout": 5
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/signal-light/scripts/codex-signal-hook PreToolUse",
            "timeout": 5
          }
        ]
      }
    ],
    "PermissionRequest": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/signal-light/scripts/codex-signal-hook PermissionRequest",
            "timeout": 10
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/signal-light/scripts/codex-signal-hook Stop",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
```
