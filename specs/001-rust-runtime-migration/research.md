# Research: Native Runtime Migration

## Decision 1: Migrate through a phased hybrid model, then cut over

**Decision**: Introduce a Rust native runtime through a temporary hybrid stage, then remove the previous implementation once native dry-run, hook, session aggregation, runtime recovery, installer, and driver-contract coverage is in place.

**Rationale**: The current user-facing instability comes from runtime packaging, startup, and long-lived process ownership rather than from lamp semantics. A phased hybrid model improves reliability without forcing a one-shot rewrite of every wrapper and installer path.

**Alternatives considered**:

- Stabilize Python only: rejected because it leaves interpreter and dependency drift as a first-order failure mode.
- Rewrite everything in one step: rejected because it combines hardware risk, IPC risk, and compatibility risk into one cutover.

## Decision 2: Keep wrapper paths stable while moving implementation native

**Decision**: Preserve the existing wrapper script paths during migration, then make those wrappers require the native binary after the native CLI owns runtime, hook, installer, and hardware behavior.

**Rationale**: Users already invoke `scripts/signal-light`, `scripts/codex-signal-hook`, and `scripts/claude-code-signal-hook`. Reusing those entry points preserves installation behavior and limits the visible change to runtime reliability instead of command discovery.

**Alternatives considered**:

- Replace documented wrapper paths with native binary paths: rejected because it would force hook reinstall or path changes.
- Keep Python wrappers forever: rejected because it would dilute the packaging and reliability gains of a native runtime.

## Decision 3: Use direct local IPC with a small persisted snapshot

**Decision**: Replace file-polled request and response handoff with a direct local IPC channel owned by the native runtime, while still persisting a small JSON snapshot plus process coordination files for status reporting, stale-runtime cleanup, and debugging.

**Rationale**: Direct local IPC lowers warm-call latency and removes the fragility of request file polling. Keeping a compact snapshot file preserves observability and crash-recovery behavior that users already benefit from.

**Alternatives considered**:

- Keep the current request-file polling model: rejected because it is the least reliable and least responsive part of the current runtime.
- Use a localhost TCP port: rejected because it adds port management and firewall sensitivity to a strictly local utility.
- Keep no persisted snapshot at all: rejected because it weakens status inspection and stale-state recovery.

## Decision 4: Keep hardware access behind a driver boundary

**Decision**: Model hardware output behind a `LightDriver`-style abstraction with at least a dry-run driver and an MCP2221A driver.

**Rationale**: The migration must preserve non-hardware validation and keep the hardware-specific risk isolated. A driver boundary also makes it possible to keep active-low and active-high behavior, direct write behavior, and failure diagnostics consistent across implementations.

**Alternatives considered**:

- Let runtime logic write to MCP2221A directly: rejected because it would entangle hardware control with session and IPC logic.
- Mock hardware only at the CLI layer: rejected because it would leave too much behavior untestable at the runtime layer.

## Decision 5: Preserve current lamp semantics and documented names

**Decision**: Keep existing signal names, signal meanings, session-priority rules, and hook event mappings unchanged during the migration unless a later spec explicitly changes the lamp language.

**Rationale**: The product's value is glanceable trust. Reliability work should not retrain users or create a documentation split between Python-era and native-era behavior.

**Alternatives considered**:

- Simplify the signal vocabulary during migration: rejected because it mixes behavioral redesign with a reliability migration.
- Re-map hook events as part of the new runtime: rejected because it creates unnecessary compatibility risk.

## Decision 6: Treat test parity as the migration gate

**Decision**: Port signal, aggregation, hook, runtime, installer, wrapper-visible CLI, and driver-contract behavior checks into native tests, and require manual reference hardware validation for any driver-affecting change.

**Rationale**: The migration only succeeds if the lamp remains trustworthy. Parity tests protect the semantics, while wrapper smoke tests protect the user-facing entry points and manual hardware checks cover the one area that pure automation cannot fully prove in this repo.

**Alternatives considered**:

- Rely on manual verification only: rejected because session aggregation and hook parsing regressions are easy to miss by eye.
- Keep Python tests after native-only cutover: rejected because they would require the implementation being retired.

## Decision 7: Avoid software PWM on the reference hardware path

**Decision**: Preserve the current strategy of using stable discrete frame output rather than brightness simulation on plain GPIO hardware unless future validated hardware support proves true brightness control without visible flicker.

**Rationale**: The reference build is a desk signal, not a display panel. Visual steadiness matters more than animation complexity, and the current documentation already explains why the work state is a slow three-color cycle instead of a breathing pulse.

**Alternatives considered**:

- Implement brightness modulation immediately in the native runtime: rejected because it risks visible flicker on the current hardware path.
- Remove animations entirely: rejected because the working state would become less glanceable.
