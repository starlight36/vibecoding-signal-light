# Implementation Plan: Native Runtime Migration

**Branch**: `codex/server-display-refactor` | **Date**: 2026-06-11 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-rust-runtime-migration/spec.md`

## Summary

Migrate runtime-critical Signal Light behavior to a Rust native binary while preserving the current user-facing CLI, hook semantics, and documented lamp behavior. The migration has reached native-only cutover in this repository: wrapper scripts now require `signal-light-native`, and the previous Python implementation has been removed.

## Technical Context

**Language/Version**: Rust stable (edition 2021, target current stable toolchain) for the runtime, hook adapters, hook installer, and hardware driver.

**Primary Dependencies**: Rust crates for CLI parsing, JSON serialization, structured error handling, process coordination, and HID access to MCP2221A.

**Storage**: Local-only runtime files in a configurable state directory, including status snapshots and process coordination files; no database.

**Testing**: `cargo test` for native unit and integration coverage plus documented manual validation on reference hardware.

**Target Platform**: macOS as the primary development and validation platform, Linux as a supported secondary local desktop target.

**Project Type**: Local CLI plus background runtime process controlling a single physical signal light.

**Performance Goals**: Warm hook handoff under 250 ms, first visible display transition under 1 second, startup or clear startup failure under 3 seconds, and no visible flicker on the reference hardware path.

**Constraints**: Preserve lamp semantics, command names, and hook mappings; remain offline-capable and local-only; support one physical lamp with multiple concurrent local agent sessions; validate packaged behavior from a fresh checkout on supported macOS or Linux machines without relying on a repo-local virtual environment or Python runtime.

**Scale/Scope**: Single-repo migration affecting one native crate, one wrapper script family, two supported agent integrations, one reference hardware path, and tens of concurrently tracked local session records.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Code Quality**: PASS. One ADR-approved top-level `native/` crate owns runtime-critical code, hook parsing, hook installation, and hardware control while wrapper scripts remain thin user-facing entry points. New abstractions are limited to a driver boundary for hardware, a runtime server boundary, and parity-oriented hook/session models.
- **Testing Discipline**: PASS. The migration adds native tests for signal mapping, session aggregation, status shaping, runtime recovery, hook parsing, installer behavior, wrapper-visible CLI behavior, and driver contracts; manual validation is limited to real MCP2221A hardware.
- **UX Consistency**: PASS. Signal names, lamp colors, CLI commands, hook behavior, and environment variable semantics remain stable. Any wrapper preference change or install-flow update must ship with README and lamp-language updates in the same change.
- **Performance**: PASS. The plan keeps long-running animation in the local runtime process, measures warm hook handoff and startup timing in automated checks where feasible, and preserves the current latency budgets from the constitution and feature spec.

## Project Structure

### Documentation (this feature)

```text
specs/001-rust-runtime-migration/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── cli-interface.md
│   ├── hook-event-contract.md
│   └── status-output-schema.json
└── tasks.md
```

### Source Code (repository root)

```text
scripts/
├── signal-light
├── install-hooks
├── codex-signal-hook
└── claude-code-signal-hook

docs/
├── LAMP_LANGUAGE.md
└── adr/
    └── 0001-rust-native-runtime-migration.md

native/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── cli.rs
│   ├── config.rs
│   ├── error.rs
│   ├── install_hooks.rs
│   ├── model.rs
│   ├── signals.rs
│   ├── hooks/
│   ├── runtime/
│   └── drivers/
└── tests/
```

**Structure Decision**: Keep wrapper scripts and docs as the stable outer interface while moving implementation ownership into a single ADR-approved `native/` crate. This keeps user-facing command paths stable and confines concurrency, IPC, installation, hook parsing, and hardware code to one reviewable boundary.

## Phase 0 Research Output

Phase 0 resolves the implementation choices that most affect migration risk:

- Native-only wrapper and missing-binary repair strategy
- Local IPC and state-recovery approach
- MCP2221A hardware access abstraction
- Wrapper and installer compatibility strategy
- Test parity strategy and performance verification approach

Research findings are recorded in [research.md](./research.md) and remove any remaining implementation ambiguity before coding begins.

## Phase 1 Design Output

Phase 1 turns the spec and research decisions into implementation-ready design artifacts:

- [data-model.md](./data-model.md) captures runtime entities, validation rules, and state transitions.
- [contracts/cli-interface.md](./contracts/cli-interface.md) defines command compatibility and exit behavior.
- [contracts/hook-event-contract.md](./contracts/hook-event-contract.md) defines supported hook payload handling and session-key behavior.
- [contracts/status-output-schema.json](./contracts/status-output-schema.json) defines the machine-readable status contract.
- [quickstart.md](./quickstart.md) defines validation scenarios for parity, runtime recovery, and reference hardware checks.

## Post-Design Constitution Check

- **Code Quality**: PASS. The design keeps wrappers thin, keeps hardware behind a driver interface, and limits new code ownership to the planned native crate plus compatibility updates in existing scripts and docs.
- **Testing Discipline**: PASS. Every behavior-sensitive migration slice has an automated parity target, with manual-only validation limited to MCP2221A reference hardware.
- **UX Consistency**: PASS. The contracts explicitly preserve current signal names, hook mappings, and status semantics; quickstart includes documentation validation so drift is caught during implementation.
- **Performance**: PASS. The design commits to warm-hook and transition budgets, a direct local IPC path, and a non-PWM fallback for plain GPIO hardware to avoid flicker.

## Complexity Tracking

No constitution exceptions are currently required. The added native crate is an intentional migration boundary, not an unbounded second architecture.

## Implementation Acceptance Notes

- 2026-06-11: `cargo test --manifest-path native/Cargo.toml` passed for the native crate, including CLI integration, recovery, parity, hook, driver, and status-shape coverage.
- 2026-06-11: `cargo test --manifest-path native/Cargo.toml --release` also passed, confirming the same behavior under optimized builds.
- 2026-06-11: measured wrapper startup for `./scripts/signal-light play working` in dry-run server mode was 184.0 ms; measured warm `./scripts/codex-signal-hook` handoff was 117.2 ms.
- 2026-06-11: the local direct-IPC implementation shipped as a native request pipe plus per-request response files rather than the earlier request-directory polling loop. This keeps the runtime event-driven on the server side while remaining compatible with the current sandbox and local-only constraints.
- 2026-06-11: hook installation moved into `native/src/install_hooks.rs`, wrapper scripts were switched to native-only execution, and the Python implementation, Python tests, and Python project metadata were removed.
