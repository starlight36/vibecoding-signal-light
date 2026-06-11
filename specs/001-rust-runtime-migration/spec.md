# Feature Specification: Native Runtime Migration

**Feature Branch**: `codex/server-display-refactor`

**Created**: 2026-06-11

**Status**: Draft

**Input**: User description: "Based on the earlier migration plan, clarify the requirements for refactoring the unstable Python-based signal light runtime and record the overall plan in an ADR before proceeding to the next step."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Reliable Agent Signal Handoff (Priority: P1)

As a Codex or Claude Code user with a physical signal light connected, I want every supported agent hook event to hand off its signal quickly and reliably, so the lamp reflects whether the agent is working, waiting, blocked, or idle without making the agent feel fragile.

**Why this priority**: The main value of the product is trustworthy ambient state. If hook delivery is slow or unreliable, the lamp becomes another thing to check instead of reducing interruptions.

**Independent Test**: Can be fully tested by sending representative hook payloads for supported events and confirming each command returns promptly while the displayed state changes to the expected signal.

**Acceptance Scenarios**:

1. **Given** the signal runtime is already available, **When** a supported hook event is received, **Then** the hook command returns within the latency budget and the display state matches the mapped signal.
2. **Given** multiple agent sessions are active, **When** one session requests permission or reports a blocking failure while another remains working, **Then** the display keeps the higher-priority attention or blocked state visible.
3. **Given** an agent turn ends normally, **When** no other active session needs attention, **Then** the display returns to the correct aggregate idle or working state without leaving a stale alert.

---

### User Story 2 - Packaged Local Runtime Experience (Priority: P2)

As a user installing or repairing Signal Light locally, I want the runtime and command wrappers to work without requiring me to manage a project-specific interpreter environment, so the hardware behaves like a dependable local utility.

**Why this priority**: The current environment-sensitive startup path is a major source of perceived instability. A packaged command path reduces setup and hook-time surprises.

**Independent Test**: Can be fully tested from a fresh checkout on a supported macOS or Linux machine by invoking the documented wrapper commands and confirming list, status, dry-run, hook, and hardware test flows do not require manual interpreter or repo-local virtual-environment setup.

**Acceptance Scenarios**:

1. **Given** a fresh checkout on a supported macOS or Linux machine, **When** the user invokes the signal-light command, **Then** the command is available through the documented wrapper path and reports clear success or actionable failure.
2. **Given** an existing installation that still has the previous script-based path, **When** the user upgrades, **Then** documented commands and installed hooks continue to work or provide a clear migration instruction.
3. **Given** the runtime cannot access the physical light, **When** the user runs a diagnostic command, **Then** the user sees a clear reason and a non-hardware validation path remains available.

---

### User Story 3 - Hardware Confidence Before Full Migration (Priority: P3)

As a project maintainer, I want hardware control to be validated as a focused milestone before the full runtime is replaced, so the migration does not lose the working reference build.

**Why this priority**: The hardware layer is the highest-risk part of the migration. Proving it early prevents rewriting surrounding behavior around an unverified device path.

**Independent Test**: Can be fully tested by running the documented hardware validation flow on the reference device and confirming each light channel responds correctly with both default and configurable active-level behavior.

**Acceptance Scenarios**:

1. **Given** the reference USB GPIO adapter and signal model are connected with default wiring, **When** the hardware validation flow runs, **Then** red, yellow, and green channels each turn on and off in the documented order.
2. **Given** active-high wiring is configured, **When** the hardware validation flow runs, **Then** the same logical red, yellow, and green behavior is preserved.
3. **Given** the hardware is missing, busy, or inaccessible, **When** the runtime attempts to initialize it, **Then** the user receives a concise diagnostic message and the process exits without leaving misleading state behind.

---

### User Story 4 - Behavior Parity for Existing Users (Priority: P4)

As an existing Signal Light user, I want current signal names, lamp meanings, command names, and hook mappings to remain stable during the migration, so my muscle memory and documentation remain valid.

**Why this priority**: The project succeeds through simple, glanceable semantics. A runtime migration should improve reliability without redefining what colors mean.

**Independent Test**: Can be fully tested by comparing current documented commands, signal names, dry-run output, hook mappings, and session aggregation results against the migrated behavior.

**Acceptance Scenarios**:

1. **Given** a documented signal name, **When** the user lists or plays that signal, **Then** the same human meaning and visible lamp behavior are preserved.
2. **Given** a supported Codex or Claude Code event, **When** the event is processed, **Then** it maps to the same signal meaning as before unless a migration note explicitly documents the change.
3. **Given** a user reads the README and lamp language documentation, **When** they compare it with command output, **Then** the same signal names and meanings are used consistently.

### Edge Cases

- The local runtime is starting while multiple hook events arrive at nearly the same time.
- A previously started runtime is no longer reachable but still has stale process or state files.
- The state store contains malformed, stale, or partially written session data.
- A session owner process exits without sending a final turn or session end event.
- A direct manual signal command is issued while tracked agent sessions are active.
- The physical device is unplugged, inaccessible, or reconnected while the runtime is starting or rendering.
- Configuration values for pin mapping, active level, timing, or state directory are missing, invalid, or inherited from a hook environment.
- Multiple supported agents emit overlapping work, permission, attention, and completion events.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST preserve all currently documented signal names and their human meanings: idle, thinking, working, tool_done, attention, permission, blocked, done, session_start, session_end, session_done, and off.
- **FR-002**: System MUST preserve supported command families for listing signals, playing a signal, reporting status, processing Codex hook events, processing Claude Code hook events, running a server process, installing hooks, and testing hardware.
- **FR-003**: System MUST aggregate concurrent session states so blocked outranks permission, permission outranks other attention states, attention outranks working, working outranks idle, and idle is shown only when no active session requires a higher-priority state.
- **FR-004**: System MUST ensure hook event commands hand work off to a local runtime without requiring the caller to run long animations or polling loops.
- **FR-005**: System MUST start or reconnect to the local runtime automatically when a normal play or hook command requires it.
- **FR-006**: System MUST detect and recover from stale, unreachable, or partially started runtime state without requiring users to manually delete temporary files in normal cases.
- **FR-007**: System MUST provide dry-run behavior that exercises signal mapping and frame sequencing without requiring physical hardware.
- **FR-008**: System MUST provide a hardware validation flow that confirms red, yellow, green, and all-off behavior for the reference traffic light setup.
- **FR-009**: System MUST allow users to configure green, yellow, and red channel mapping and active-low or active-high behavior with documented settings.
- **FR-010**: System MUST report hardware initialization, hardware write, runtime startup, invalid request, and timeout failures with actionable messages.
- **FR-011**: System MUST preserve hook installation and repair behavior for supported Codex and Claude Code events, including coexistence with unrelated hooks in the same configuration files.
- **FR-012**: System MUST preserve session cleanup behavior for stale sessions, finished sessions, explicit clear commands, and exited owner processes.
- **FR-013**: System MUST write status output that exposes the current aggregate state, display state, and active session records in a machine-readable format.
- **FR-014**: System MUST provide a migration path where existing wrapper scripts and installed hooks either continue to invoke the new runtime or fail with a clear repair instruction.
- **FR-015**: System MUST retire the previous runtime path after the migrated runtime passes dry-run, hook, session aggregation, runtime recovery, installer, and native driver contract validation; wrapper scripts MUST fail with a clear native build instruction if no native binary is available.

### Experience Consistency Requirements

- **UX-001**: System MUST preserve green as idle or completion notice, yellow as attention or permission, red as blocked or failed, and the slow three-color cycle as working across CLI output, hook adapters, dry-run output, documentation, and physical display.
- **UX-002**: System MUST keep existing user-visible command names, signal names, environment setting names, and hook event names stable unless a migration note explains a deliberate change.
- **UX-003**: System MUST update README, lamp language documentation, and installation guidance in the same change as any user-visible command, signal, configuration, or migration behavior change.
- **UX-004**: System MUST make diagnostics concise enough for hook logs while still giving users a next step for common failures such as missing hardware, busy hardware, invalid configuration, or unreachable runtime.

### Performance Requirements

- **PR-001**: System MUST return hook or event commands in under 250 ms when the local runtime is already available.
- **PR-002**: System MUST make the first visible display transition within 1 second after a valid signal is accepted by the runtime.
- **PR-003**: System MUST complete normal runtime startup or report a clear startup failure within 3 seconds.
- **PR-004**: System MUST keep continuous working, attention, permission, and blocked animations persistent without requiring the hook caller to remain open.
- **PR-005**: System MUST avoid visible flicker for the reference display path by using stable frame timing and avoiding unsupported brightness simulation on plain GPIO hardware.
- **PR-006**: System MUST define degraded behavior for missing hardware, inaccessible hardware, invalid runtime state, and unavailable local runtime so users can still run dry-run and diagnostic flows.

### Key Entities *(include if feature involves data)*

- **Signal**: A named lamp-language state with a human meaning, visible frame pattern, repeat behavior, and optional final steady state.
- **Frame**: A single visible output step that specifies which light channels are active and how long the step lasts.
- **Session**: A tracked agent activity source identified by session, turn, request, workspace, or global fallback key, with current signal, update time, and optional owner process.
- **Aggregate State**: The prioritized state derived from all active sessions and direct overrides.
- **Display State**: The actual signal currently rendered by the local runtime, including short completion notices and direct manual overrides.
- **Runtime Process**: The local owner of display state, session state, animation timing, and physical hardware access.
- **Hook Event**: A Codex or Claude Code event payload that maps to a Signal and session key.
- **Hardware Mapping**: User-configurable association between logical green, yellow, and red channels and physical output pins, including active-low or active-high behavior.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a repeated local hook handoff test with the runtime already available, at least 99 out of 100 supported hook events return within 250 ms and produce the expected state.
- **SC-002**: In a concurrent-session test suite covering working, attention, permission, blocked, turn end, session end, and stale cleanup cases, 100% of expected aggregate states are produced.
- **SC-003**: In a fresh checkout on a supported macOS or Linux machine, list, status, dry-run play, hook dry-run, and diagnostic commands can be completed through the documented wrapper commands without manual interpreter or repo-local virtual-environment setup.
- **SC-004**: In the reference hardware validation flow, red, yellow, green, and all-off outputs match the documented sequence for default wiring and for the configured inverse active level.
- **SC-005**: During a missing-hardware or inaccessible-hardware test, 100% of failures produce a concise diagnostic message and leave no stale active alert that misrepresents the agent state.
- **SC-006**: Existing documented signal names, hook event mappings, and command names remain compatible across the migration, with any intentional exception documented before release.
- **SC-007**: Users following the updated install or repair instructions can restore working hooks in under 5 minutes on a supported local machine.

## Assumptions

- The primary users are local Codex and Claude Code users running a physical three-light desk signal on a supported desktop environment.
- The migration improves runtime packaging and reliability while preserving the current product behavior rather than introducing a new lamp language.
- The reference hardware remains the MCP2221A USB GPIO adapter with red, yellow, and green channels, defaulting to active-low wiring.
- The previous runtime remains available during migration until the new runtime passes behavior parity, dry-run, and reference hardware validation.
- The first migration milestone should validate signal semantics and hardware control before replacing the entire runtime path.
- Existing untracked documentation drafts in the working tree are unrelated to this feature and should not be modified by the migration spec work.
