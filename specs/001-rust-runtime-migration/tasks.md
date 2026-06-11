# Tasks: Native Runtime Migration

**Input**: Design documents from `/specs/001-rust-runtime-migration/`

**Prerequisites**: [plan.md](./plan.md) (required), [spec.md](./spec.md) (required for user stories), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: Tests are REQUIRED for this feature because the constitution requires automated coverage for every behavior change and the migration must prove native parity before retiring the previous implementation.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each migration slice.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g. US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- Native runtime surface: `native/src/`, `native/tests/`
- Wrapper surface: `scripts/`
- Feature docs and contracts: `specs/001-rust-runtime-migration/`, `docs/`

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create the native runtime workspace and basic module structure without changing behavior yet.

- [X] T001 Create the native crate manifest and binary entrypoint scaffold in `native/Cargo.toml` and `native/src/main.rs`
- [X] T002 [P] Create the native module skeleton in `native/src/cli.rs`, `native/src/config.rs`, `native/src/error.rs`, `native/src/model.rs`, `native/src/hooks/mod.rs`, `native/src/runtime/mod.rs`, and `native/src/drivers/mod.rs`
- [X] T003 [P] Create the native test harness scaffold in `native/tests/common/mod.rs`, `native/tests/signal_contracts.rs`, and `native/tests/hook_contracts.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared building blocks that all user stories depend on.

**⚠️ CRITICAL**: No user story work should begin until this phase is complete.

- [X] T004 Implement environment and state-directory configuration helpers in `native/src/config.rs`
- [X] T005 [P] Implement shared domain structs for frames, signals, sessions, runtime commands, and snapshots in `native/src/model.rs`
- [X] T006 [P] Implement structured native error types and result helpers in `native/src/error.rs`
- [X] T007 [P] Implement the driver abstraction and dry-run driver in `native/src/drivers/mod.rs` and `native/src/drivers/dry_run.rs`
- [X] T008 Implement reusable test fixtures for hook payloads, signal snapshots, and session state in `native/tests/common/mod.rs`

**Checkpoint**: Native crate, shared models, config, errors, dry-run driver, and test fixtures are ready for story work.

---

## Phase 3: User Story 1 - Reliable Agent Signal Handoff (Priority: P1) 🎯 MVP

**Goal**: Deliver a native runtime that can accept hook events, maintain session aggregation, expose status, and render signal behavior in dry-run mode.

**Independent Test**: Send representative Codex and Claude Code hook payloads through the native command path, confirm warm calls return within budget, and verify the reported status and aggregate signal match the expected mapping without requiring physical hardware.

### Tests for User Story 1 (REQUIRED) ⚠️

> **NOTE: Write these tests first, ensure they fail before implementation.**

- [X] T009 [P] [US1] Add signal-definition and aggregate-priority tests in `native/tests/signal_contracts.rs`
- [X] T010 [P] [US1] Add hook mapping, session-key precedence, and control-event tests in `native/tests/hook_contracts.rs`
- [X] T011 [P] [US1] Add runtime request and status snapshot tests in `native/tests/runtime_server.rs`
- [X] T012 [P] [US1] Add invalid-request, timeout, and stale-alert recovery tests in `native/tests/runtime_recovery.rs`

### Implementation for User Story 1

- [X] T013 [P] [US1] Implement signal definitions and frame timing helpers in `native/src/signals.rs`
- [X] T014 [P] [US1] Implement Codex and Claude Code hook parsing in `native/src/hooks/codex.rs` and `native/src/hooks/claude_code.rs`
- [X] T015 [P] [US1] Implement session-store lifecycle, aggregate priority, and owner cleanup rules in `native/src/runtime/session_store.rs`
- [X] T016 [US1] Implement runtime commands and status snapshot shaping in `native/src/runtime/commands.rs`
- [X] T017 [US1] Implement direct local IPC request handling in `native/src/runtime/ipc.rs` and `native/src/runtime/server.rs`
- [X] T018 [US1] Implement native CLI subcommands for `list`, `status`, `play`, `codex-hook`, `claude-code-hook`, and `server` in `native/src/cli.rs` and `native/src/main.rs`

**Checkpoint**: User Story 1 is complete when the native runtime can drive dry-run signal behavior, accept hook events, aggregate sessions, and report status without touching physical hardware.

---

## Phase 4: User Story 2 - Packaged Local Runtime Experience (Priority: P2)

**Goal**: Preserve the existing operator-facing wrapper and hook installation experience while requiring the native runtime.

**Independent Test**: Invoke the documented `./scripts` entry points from a fresh checkout on a supported macOS or Linux machine and confirm they use the native binary, fail clearly when the binary is missing, keep exit codes stable, and keep hook installation and repair flows understandable.

### Tests for User Story 2 (REQUIRED) ⚠️

- [X] T019 [P] [US2] Add native wrapper and missing-binary exit-code regression tests in `native/tests/cli_runtime_integration.rs`
- [X] T020 [P] [US2] Add native startup timing and status-shape integration tests in `native/tests/cli_runtime_integration.rs`
- [X] T021 [US2] Add hook installer coexistence, repair, and failure-path regression tests in `native/src/install_hooks.rs` and `native/tests/cli_runtime_integration.rs`

### Implementation for User Story 2

- [X] T022 [P] [US2] Teach the wrapper entry points to require the native binary with clear repair messaging in `scripts/signal-light`, `scripts/codex-signal-hook`, and `scripts/claude-code-signal-hook`
- [X] T023 [US2] Move hook installer compatibility and repair behavior into `native/src/install_hooks.rs` and `native/src/cli.rs`
- [X] T024 [US2] Document native build, missing-binary repair, hook repair timing, and native-only expectations in `README.md` and `docs/LAMP_LANGUAGE.md`

**Checkpoint**: User Story 2 is complete when the documented wrapper commands and installer flows keep working for users without requiring them to manage a Python interpreter path.

---

## Phase 5: User Story 3 - Hardware Confidence Before Full Migration (Priority: P3)

**Goal**: Validate MCP2221A hardware control behind the native driver boundary while keeping a clear dry-run path and actionable failure messages.

**Independent Test**: Run the native hardware validation flow on the reference MCP2221A setup, confirm red/yellow/green/all-off behavior for default and inverted active levels, and verify missing-hardware cases fail clearly without leaving misleading state.

### Tests for User Story 3 (REQUIRED) ⚠️

- [X] T025 [P] [US3] Add active-low and active-high driver contract tests in `native/tests/driver_contracts.rs`
- [X] T026 [US3] Add the reference hardware validation checklist and expected outcomes to `specs/001-rust-runtime-migration/quickstart.md` and `README.md`

### Implementation for User Story 3

- [X] T027 [P] [US3] Implement MCP2221A driver initialization and logical pin writes in `native/src/drivers/mcp2221.rs`
- [X] T028 [US3] Implement hardware mapping parsing and runtime driver selection in `native/src/config.rs` and `native/src/drivers/mod.rs`
- [X] T029 [US3] Implement the hardware test command and actionable hardware diagnostics in `native/src/cli.rs`, `native/src/drivers/mcp2221.rs`, and `native/src/error.rs`

**Checkpoint**: User Story 3 is complete when the native runtime can validate reference hardware behavior and fail safely when the device is unavailable or misconfigured.

---

## Phase 6: User Story 4 - Behavior Parity for Existing Users (Priority: P4)

**Goal**: Lock down compatibility so current signal meanings, command names, event mappings, and rollout rules remain stable throughout migration.

**Independent Test**: Compare the wrapper-level list/play/status/hook flows and the native runtime parity tests against the current documented semantics, then confirm the compatibility and rollout notes in docs match the observed behavior.

### Tests for User Story 4 (REQUIRED) ⚠️

- [X] T030 [P] [US4] Add signal-name, event-mapping, and status-output parity regressions in `native/tests/parity_regressions.rs`
- [X] T031 [P] [US4] Add wrapper-visible compatibility smoke cases for list, play, status, hook, and installer flows in `native/tests/cli_runtime_integration.rs`

### Implementation for User Story 4

- [X] T032 [P] [US4] Align native CLI and status behavior with the compatibility contracts in `native/src/cli.rs` and `native/src/runtime/commands.rs`
- [X] T033 [US4] Update rollout gates and compatibility promises in `README.md`, `docs/LAMP_LANGUAGE.md`, and `docs/adr/0001-rust-native-runtime-migration.md`
- [X] T034 [US4] Record parity acceptance notes and migration checkpoints in `specs/001-rust-runtime-migration/plan.md` and `specs/001-rust-runtime-migration/quickstart.md`

**Checkpoint**: User Story 4 is complete when compatibility promises are enforced by tests and reflected consistently in runtime behavior and documentation.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final validation, performance verification, and documentation drift cleanup across all stories.

- [X] T035 [P] Run the full validation command matrix and capture expected outcomes in `specs/001-rust-runtime-migration/quickstart.md`
- [X] T036 [P] Run clean-environment hook install and repair timing verification and record elapsed time in `specs/001-rust-runtime-migration/quickstart.md`
- [X] T037 [P] Verify warm-hook, first visible display transition, and startup budgets with automated or scripted measurements in `native/tests/cli_runtime_integration.rs` and `specs/001-rust-runtime-migration/quickstart.md`
- [X] T038 [P] Reconcile documentation and contract drift across `README.md`, `docs/LAMP_LANGUAGE.md`, `specs/001-rust-runtime-migration/contracts/cli-interface.md`, and `specs/001-rust-runtime-migration/contracts/hook-event-contract.md`
- [X] T039 Run final `cargo test` and release checks and record any hardware-only validation gaps in `specs/001-rust-runtime-migration/quickstart.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1: Setup**: No dependencies, can start immediately
- **Phase 2: Foundational**: Depends on Phase 1 completion and blocks all user stories
- **Phase 3: User Story 1 (US1)**: Depends on Phase 2 completion and establishes the native runtime MVP
- **Phase 4: User Story 2 (US2)**: Depends on US1 because wrapper execution and installer compatibility require a working native runtime
- **Phase 5: User Story 3 (US3)**: Depends on US1 because hardware validation builds on the native runtime and driver boundary
- **Phase 6: User Story 4 (US4)**: Depends on US2 and US3 so compatibility promises reflect the actual rollout path
- **Phase 7: Polish**: Depends on all desired user stories being complete

### User Story Dependencies

- **US1 (P1)**: Core MVP, no dependency on other user stories after Phase 2
- **US2 (P2)**: Depends on US1, but is otherwise independently testable once the native runtime exists
- **US3 (P3)**: Depends on US1, but is otherwise independently testable with reference hardware
- **US4 (P4)**: Depends on US2 and US3 because it locks final compatibility and rollout semantics

### Within Each User Story

- Tests and regression tasks must be written before implementation tasks in that story
- Shared models, config, and driver abstractions from Phase 2 must remain the only cross-story prerequisites
- Story-specific docs must be updated inside the story that changes the user-visible behavior
- A story is complete only when its independent test criteria pass without depending on incomplete later stories

### Suggested Execution Graph

```text
Setup -> Foundational -> US1 -> {US2, US3 in parallel} -> US4 -> Polish
```

## Parallel Opportunities

- `T002` and `T003` can run in parallel after `T001`
- `T005`, `T006`, and `T007` can run in parallel once `T004` has started the shared config pattern
- In **US1**, `T009`, `T010`, `T011`, and `T012` can run in parallel, followed by `T013`, `T014`, and `T015`
- In **US2**, `T019`, `T020`, and `T021` can run in parallel before `T022`
- In **US3**, `T025` and `T026` can run in parallel before `T027`
- In **US4**, `T030` and `T031` can run in parallel before `T032`
- `T035`, `T036`, `T037`, and `T038` can run in parallel during polish before the final release checks in `T039`

## Parallel Example: User Story 1

```bash
# Launch US1 regression tasks together
Task: "Add signal-definition and aggregate-priority tests in native/tests/signal_contracts.rs"
Task: "Add hook mapping, session-key precedence, and control-event tests in native/tests/hook_contracts.rs"
Task: "Add runtime request and status snapshot tests in native/tests/runtime_server.rs"
Task: "Add invalid-request, timeout, and stale-alert recovery tests in native/tests/runtime_recovery.rs"

# Launch US1 core module work together after tests exist
Task: "Implement signal definitions and frame timing helpers in native/src/signals.rs"
Task: "Implement Codex and Claude Code hook parsing in native/src/hooks/codex.rs and native/src/hooks/claude_code.rs"
Task: "Implement session-store lifecycle, aggregate priority, and owner cleanup rules in native/src/runtime/session_store.rs"
```

## Parallel Example: User Story 2

```bash
# Launch US2 validation work together
Task: "Add native wrapper and missing-binary exit-code regression tests in native/tests/cli_runtime_integration.rs"
Task: "Add native startup timing and status-shape integration tests in native/tests/cli_runtime_integration.rs"
Task: "Add hook installer coexistence, repair, and failure-path regression tests in native/src/install_hooks.rs and native/tests/cli_runtime_integration.rs"

# Launch US2 compatibility work together where file ownership differs
Task: "Teach the wrapper entry points to require the native binary with clear repair messaging in scripts/signal-light, scripts/codex-signal-hook, and scripts/claude-code-signal-hook"
Task: "Document native build, missing-binary repair, hook repair timing, and native-only expectations in README.md and docs/LAMP_LANGUAGE.md"
```

## Parallel Example: User Story 3

```bash
# Launch US3 preparation work together
Task: "Add active-low and active-high driver contract tests in native/tests/driver_contracts.rs"
Task: "Add the reference hardware validation checklist and expected outcomes to specs/001-rust-runtime-migration/quickstart.md and README.md"

# Launch US3 implementation work together where possible
Task: "Implement MCP2221A driver initialization and logical pin writes in native/src/drivers/mcp2221.rs"
Task: "Implement hardware mapping parsing and runtime driver selection in native/src/config.rs and native/src/drivers/mod.rs"
```

## Parallel Example: User Story 4

```bash
# Launch US4 regression tasks together
Task: "Add signal-name, event-mapping, and status-output parity regressions in native/tests/parity_regressions.rs"
Task: "Add wrapper-visible compatibility smoke cases for list, play, status, hook, and installer flows in native/tests/cli_runtime_integration.rs"

# Launch US4 compatibility alignment together
Task: "Align native CLI and status behavior with the compatibility contracts in native/src/cli.rs and native/src/runtime/commands.rs"
Task: "Update rollout gates and compatibility promises in README.md, docs/LAMP_LANGUAGE.md, and docs/adr/0001-rust-native-runtime-migration.md"
```

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational
3. Complete Phase 3: User Story 1
4. Stop and validate native dry-run runtime parity before touching wrapper preference or hardware

### Incremental Delivery

1. Deliver **US1** to prove native runtime semantics and hook handoff
2. Deliver **US2** to preserve the operator-facing wrapper and installer experience on the native runtime
3. Deliver **US3** to prove reference hardware control safely
4. Deliver **US4** to lock compatibility, rollout gates, and documentation promises
5. Finish with polish and final regression checks

### Parallel Team Strategy

1. One developer completes Setup and Foundational tasks
2. Once US1 is stable:
   - Developer A: US2 wrapper and installer compatibility
   - Developer B: US3 hardware driver and validation
3. Rejoin for US4 parity lock-down and final polish

## Notes

- All tasks use the required checklist format with task ID, optional parallel marker, optional story label, and exact file paths
- Tasks marked `[P]` should still be coordinated to avoid overlapping edits in the same file
- The suggested MVP scope is **User Story 1 only**
- The previous implementation has been removed after native parity and wrapper/installer coverage landed; remaining release risk is physical hardware validation on the reference build
