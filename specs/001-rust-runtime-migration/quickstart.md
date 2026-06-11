# Quickstart Validation Guide

## Purpose

Use this guide to validate the migration end to end as implementation slices land. The focus is on proving behavior parity, runtime recovery, and hardware confidence without changing the documented operator experience.

## Prerequisites

- A checked-out repository with the native runtime code available.
- Rust toolchain available for building and testing the native runtime.
- Reference MCP2221A traffic-light hardware for the hardware validation slice.

## Supported Clean-Environment Scenario

For packaged-runtime validation, treat a "clean" environment as:

- a fresh checkout on a supported macOS or Linux machine
- no Python runtime, virtual environment, or Python package install required
- validation performed through the documented `./scripts` entry points first
- missing-binary behavior considered successful only if the user receives a concise native build instruction

## Validation Sequence

### 1. Run automated behavior parity checks

Expected outcome: current signal semantics and native logic remain aligned.

```bash
cargo test --manifest-path native/Cargo.toml
cargo test --manifest-path native/Cargo.toml --release
```

Verify:

- Native tests cover signal mapping, session aggregation, runtime command handling, and status shaping.

Observed on 2026-06-11:

| Check | Result | Notes |
| --- | --- | --- |
| `cargo test --manifest-path native/Cargo.toml` | PASS | native tests cover CLI, hooks, installer, runtime recovery, server behavior, signals, and driver contracts |
| `cargo test --manifest-path native/Cargo.toml --release` | PASS | native release-mode tests cover the same behavior set |

### 2. Validate wrapper-level dry-run behavior

Expected outcome: documented command paths remain stable and show the same logical signal behavior without hardware.

```bash
./scripts/signal-light list
./scripts/signal-light play working --dry-run
./scripts/signal-light play permission --dry-run
./scripts/signal-light status
```

Verify:

- Signal names match the lamp-language documentation.
- Dry-run output remains understandable and consistent with the current tool.
- `status` returns JSON matching [contracts/status-output-schema.json](./contracts/status-output-schema.json).
- The wrapper path works from the supported clean-environment scenario without relying on a pre-created repo-local virtual environment.

Observed on 2026-06-11 with `SIGNAL_LIGHT_SERVER_DRY_RUN=1`:

| Command | Result | Notes |
| --- | --- | --- |
| `./scripts/signal-light list` | PASS in 9.1 ms | wrapper preferred the native binary |
| `./scripts/signal-light status` | PASS in 8.0 ms | returned `aggregate`, `display_signal`, and `sessions` |
| `./scripts/signal-light play working --dry-run` | PASS in 9.0 s | printed the expected green/yellow/red brightness cycle frames |

### 3. Validate hook install and repair timing

Expected outcome: a user can restore working hooks through the documented repair flow within the success-criteria budget.

```bash
time ./scripts/install-hooks --all -y
```

Verify:

- The documented repair flow completes successfully through the wrapper path.
- The elapsed time is under 5 minutes on a supported local machine.
- Any failure returns a concise repair message with a next step.

Observed on 2026-06-11:

| Command | Result | Notes |
| --- | --- | --- |
| `HOME=$(mktemp -d) ./scripts/install-hooks --all -y` | PASS | clean-home install succeeded through the native installer |

### 4. Validate hook handoff compatibility

Expected outcome: hook adapters still resolve the same signals and session keys while returning promptly.

```bash
printf '{"event":"UserPromptSubmit","session_id":"demo-session"}' | ./scripts/codex-signal-hook
printf '{"event":"PermissionRequest","session_id":"demo-session"}' | ./scripts/codex-signal-hook
printf '{"event":"Notification","session_id":"claude-demo"}' | ./scripts/claude-code-signal-hook
./scripts/signal-light status
```

Verify:

- Event handling matches [contracts/hook-event-contract.md](./contracts/hook-event-contract.md).
- Later status output reflects the expected aggregate state.
- Warm hook calls meet the latency budget from the plan and feature spec.

Observed on 2026-06-11 with `SIGNAL_LIGHT_SERVER_DRY_RUN=1`:

| Command | Result | Notes |
| --- | --- | --- |
| `./scripts/signal-light play working` | PASS in 184.0 ms | startup stayed under the 3 second budget and rendered the first frame before command exit |
| `printf '{"event":"PermissionRequest","session_id":"demo-session"}' \| ./scripts/codex-signal-hook` | PASS in 117.2 ms | warm hook stayed under the 250 ms budget |
| `./scripts/signal-light status` | PASS in 125.4 ms | aggregate and display both moved to `permission` |

### 5. Validate runtime recovery behavior

Expected outcome: stale or unreachable runtime state is recovered without manual cleanup in normal cases.

```bash
./scripts/signal-light play working
./scripts/signal-light status
```

Then force a stale-runtime scenario using the implementation's documented recovery test procedure and repeat:

```bash
./scripts/signal-light play attention
./scripts/signal-light status
```

Verify:

- The command path reconnects to or restarts the runtime cleanly.
- Invalid runtime requests and request timeouts fail with actionable diagnostics instead of silent hangs.
- No misleading stale blocked or permission alert remains after recovery.
- Startup succeeds or fails clearly within the documented time budget.

Observed on 2026-06-11:

- native recovery tests cover invalid request rejection, explicit request timeout, no-autostart failure, and session-end notice cleanup
- `native/tests/runtime_recovery.rs` passed in both debug and release runs

### 6. Validate reference hardware output

Expected outcome: reference wiring still matches documented red, yellow, green, and all-off behavior.

```bash
./scripts/signal-light test
./scripts/signal-light play idle
./scripts/signal-light play permission
./scripts/signal-light play off
```

Verify:

- Red, yellow, green, and all-off outputs occur in the documented order.
- Active-low default behavior is correct on the reference build.
- The configured inverse active level also produces the same logical signal meanings.
- Hardware failures produce concise diagnostics and do not leave the display in a misleading state.

Reference hardware checklist:

1. Leave the default mapping at `gp0=green`, `gp1=yellow`, `gp2=red`, `SIGNAL_LIGHT_ACTIVE_LOW=1`.
2. Run `./scripts/signal-light test` and confirm red -> yellow -> green -> all-on in that order.
3. Run `./scripts/signal-light play idle`, `./scripts/signal-light play permission`, and `./scripts/signal-light play off` to confirm steady green, flashing yellow/red attention semantics, and all-off cleanup.
4. Set `SIGNAL_LIGHT_ACTIVE_LOW=0` and repeat the same commands. The logical lamp meanings should stay the same even though the GPIO polarity flips.
5. Unplug the MCP2221A or occupy it from another process and confirm the command exits non-zero with a concise hardware diagnostic.

Hardware validation gap on 2026-06-11:

- not run in this workspace because the reference MCP2221A traffic light was not attached
- automated driver contract coverage passed, but the physical checklist above still needs to be executed on the reference build before release packaging

## Documentation Validation

Before considering the migration slice ready, confirm that:

- `README.md` installation and usage steps match the current native-only wrapper behavior.
- `docs/LAMP_LANGUAGE.md` still matches the visible signal semantics.
- Any new migration instruction is documented in the same change as the wrapper or runtime behavior it describes.
