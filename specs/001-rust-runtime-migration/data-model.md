# Data Model: Native Runtime Migration

## Overview

The migration keeps the current user-visible lamp language but re-homes the runtime behavior in a native process. The core data model stays small and state-oriented so it can support dry-run tests, runtime recovery, and hardware output without duplicating logic.

## Entities

### Signal Definition

Represents a named lamp-language state.

| Field | Type | Description | Validation |
| --- | --- | --- | --- |
| `name` | enum-like string | Stable public signal name such as `idle`, `working`, or `permission` | Must match a documented signal name |
| `summary` | string | Human-facing meaning of the signal | Must remain aligned with README and lamp language docs |
| `attention_level` | derived classification | Whether the signal is idle, working, attention, permission, blocked, or completion-related | Must preserve current priority semantics |
| `frames` | ordered list of Frame | Visible sequence for the signal | Must be non-empty for animated signals |
| `repeat` | boolean | Whether the signal loops until replaced | Required for working and alert states |
| `steady_state` | optional logical light tuple | Final steady output after non-repeating playback | Required for idle-like steady states |

### Frame

Represents one output step in the visible sequence.

| Field | Type | Description | Validation |
| --- | --- | --- | --- |
| `green_on` | boolean | Whether green is logically on | Defaults to `false` |
| `yellow_on` | boolean | Whether yellow is logically on | Defaults to `false` |
| `red_on` | boolean | Whether red is logically on | Defaults to `false` |
| `duration_ms` | integer | How long the frame should remain active | Must be non-negative |
| `brightness_hint` | optional normalized number | Used only by drivers that can honor brightness safely | Must remain within supported driver range |

### Session Record

Tracks one active agent activity source.

| Field | Type | Description | Validation |
| --- | --- | --- | --- |
| `session_key` | string | Stable identifier derived from turn, request, session, workspace, or global fallback | Must be non-empty |
| `signal_name` | string | Current signal for the session | Must reference a defined signal |
| `updated_at` | timestamp | Last write time for staleness checks | Must be monotonic enough for expiry logic |
| `owner_pid` | optional integer | Originating process identifier when explicitly known | Must be positive if present |
| `owner_pid_source` | optional enum-like string | Whether PID ownership was explicit or inherited | Must not downgrade explicit ownership silently |

### Direct Override

Represents a user-issued signal that temporarily overrides aggregated session display.

| Field | Type | Description | Validation |
| --- | --- | --- | --- |
| `signal_name` | string | Override display signal | Must reference a defined signal |
| `set_at` | timestamp | When the override was applied | Optional but useful for debugging |
| `clears_sessions` | boolean | Whether the override also clears tracked sessions | Required for `idle` and `off` compatibility |

### Aggregate State

Represents the prioritized state derived from all active sessions plus any explicit direct override.

| Field | Type | Description | Validation |
| --- | --- | --- | --- |
| `aggregate_signal` | string | Highest-priority signal from active sessions | Must follow priority order: blocked > permission > attention > working > idle |
| `display_signal` | string | Actual signal currently rendered after applying direct overrides or completion notices | Must always be a defined signal |
| `show_completion_notice` | boolean | Whether a short completion cue should be rendered before returning to aggregate state | Must never hide blocked or permission states |

### Runtime Snapshot

Machine-readable state emitted for status reporting and recovery.

| Field | Type | Description | Validation |
| --- | --- | --- | --- |
| `aggregate` | string | Current aggregate signal | Must be defined |
| `display_signal` | string | Signal currently shown on the lamp | Must be defined |
| `sessions` | map of Session Record | Active tracked session state | Invalid or stale entries must be pruned |
| `runtime_pid` | optional integer | Current runtime process identifier | Optional for diagnostics |
| `updated_at` | timestamp | Last snapshot refresh time | Required for stale-runtime inspection |

### Runtime Command

Represents one inbound request handled by the runtime.

| Field | Type | Description | Validation |
| --- | --- | --- | --- |
| `action` | enum-like string | Command such as status, direct signal, or session signal | Must be one of the supported actions |
| `session_key` | optional string | Target session for session updates | Required for session-signal actions |
| `signal_name` | optional string | Requested signal | Required for signal actions |
| `owner_pid` | optional integer | Explicit owner process to associate with the session | Must be positive if present |
| `speed_factor` | optional number | Requested timing multiplier for dry-run or rendering | Must stay within safe minimum and maximum bounds |

### Hook Event

Normalized representation of a Codex or Claude Code hook payload.

| Field | Type | Description | Validation |
| --- | --- | --- | --- |
| `agent_family` | enum-like string | Codex or Claude Code | Must match a supported integration |
| `event_name` | string | Hook event identifier | Must map to a supported event or default attention behavior |
| `payload` | JSON-like object | Original structured payload | Must remain readable for session key and failure extraction |
| `derived_session_key` | string | Resolved session identifier | Must use the documented precedence order |
| `derived_signal_name` | string | Signal selected from the event and payload | Must be a defined signal or supported control signal |

### Hardware Mapping

Represents logical-to-physical output mapping for the connected signal light.

| Field | Type | Description | Validation |
| --- | --- | --- | --- |
| `green_pin` | string | Physical output for green | Must be a supported pin name |
| `yellow_pin` | string | Physical output for yellow | Must be a supported pin name |
| `red_pin` | string | Physical output for red | Must be a supported pin name |
| `active_low` | boolean | Whether logical "on" is expressed as low output | Defaults to `true` for the reference build |

## Relationships

- A **Signal Definition** owns one or more **Frame** entries.
- A **Session Record** references exactly one **Signal Definition** by name.
- The **Aggregate State** is derived from the full set of **Session Record** values plus any **Direct Override**.
- The **Runtime Snapshot** materializes the current **Aggregate State** and active **Session Record** map.
- A **Runtime Command** can update one **Session Record**, mutate the **Direct Override**, or request the current **Runtime Snapshot**.
- A **Hook Event** resolves to a **Runtime Command** affecting a **Session Record**.
- A **Hardware Mapping** is consumed by the active runtime driver to turn logical channel values into physical writes.

## Validation Rules

- Unknown signal names are rejected before state mutation.
- Empty session keys are rejected for session updates.
- Stale sessions are pruned by update time and owner-process liveness rules before computing the aggregate state.
- Direct overrides must not leave the runtime showing an invalid signal after the originating request completes.
- Invalid or malformed snapshot entries are discarded rather than trusted.
- Hardware mappings must be explicit enough to drive all three logical channels.

## State Transitions

### Session Record Lifecycle

1. **Created or refreshed** when a session-signal command is accepted.
2. **Persists** while receiving updates within the allowed freshness window.
3. **Transitions to completed or cleared** when turn-end, session-end, or explicit clear semantics apply.
4. **Expires** when update time is stale or the explicitly tracked owner process is no longer alive.

### Aggregate State Lifecycle

1. **Idle** when no active session remains.
2. **Working** when at least one active session is thinking, working, or tool-done and no higher-priority state is present.
3. **Attention** when any attention-like session is present without permission or blocked.
4. **Permission** when any session explicitly needs approval and no blocked state is present.
5. **Blocked** when any session is blocked or failed.
6. **Directly overridden** when a manual command explicitly sets the display signal.
7. **Returns to aggregate** after a short completion notice or when the override is cleared.
