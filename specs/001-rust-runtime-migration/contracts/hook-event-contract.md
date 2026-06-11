# Hook Event Contract

## Scope

This contract defines how Codex and Claude Code hook payloads are interpreted during the migration. The runtime implementation may change, but the event-to-signal behavior and session-key precedence must remain stable.

## Common Requirements

1. Hook commands must accept either an explicit event argument or a structured payload on standard input.
2. Explicit payload-selected signal names may override default event mappings only when the selected signal is a documented supported signal.
3. Structured failure markers in payload data must continue to map to the blocked signal when they indicate a real failure.
4. Hook commands must derive a stable session key before updating runtime state.
5. Hook commands should remain quiet on success unless invoked in a diagnostic mode.
6. Native wrapper entry points must preserve event mapping, session-key precedence, and quiet-success behavior.

## Codex Event Mapping Contract

| Event | Required signal result |
| --- | --- |
| `SessionStart` | `session_start` |
| `UserPromptSubmit` | `thinking` |
| `PreToolUse` | `working` |
| `PostToolUse` | `tool_done` unless the payload indicates failure |
| `PermissionRequest` | `permission` |
| `Stop` | `turn_end` control behavior |
| `SessionEnd` | `session_end` |
| Unknown event | `attention` |

Structured or explicit failure markers must resolve to `blocked`, even if the nominal event would otherwise produce `tool_done`.

## Claude Code Event Mapping Contract

| Event | Required signal result |
| --- | --- |
| `SessionStart` | `session_start` |
| `UserPromptSubmit` | `thinking` |
| `PreToolUse` | `working` |
| `PostToolUse` | `tool_done` |
| `PostToolUseFailure` | `blocked` |
| `PreCompact` | `working` |
| `SubagentStart` | `working` |
| `SubagentStop` | `tool_done` |
| `Notification` | `attention` |
| `PermissionRequest` | `permission` |
| `Stop` | `turn_end`, except blocked stop reasons that remain `blocked` |
| `SessionEnd` | `session_end` |
| Unknown event | `attention` |

## Session Key Resolution Contract

### Codex precedence

1. `turn_id` or `request_id` from top-level payload
2. Nested `turn_id` or `request_id` in structured payload
3. `CODEX_TURN_ID` or `CODEX_REQUEST_ID` from environment
4. Explicit session-like IDs from payload
5. Nested session-like IDs from payload
6. Session-like IDs from environment
7. `cwd` or workspace path from payload
8. `global`

### Claude Code precedence

1. `session_id` from payload
2. Session identifier from environment
3. `cwd` from payload
4. `global`

## Owner Process Contract

- If a positive owner PID is provided explicitly by payload or documented environment variables, it must be attached to the session record.
- Explicit owner PID ownership must survive compatible follow-up updates for the same session key.
- Sessions associated with an explicit owner PID must be eligible for cleanup when that process no longer exists.

## Error and Exit Contract

- Invalid usage or unsupported input should return exit code `2`.
- Runtime, transport, or hardware failures should return exit code `1`.
- Successful hook handling should return exit code `0`, even when the resulting logical action is a control event such as `turn_end`.
