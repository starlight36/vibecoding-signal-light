# ADR 0001: Migrate Signal Light Runtime to a Rust Native Binary

**Status**: Accepted

**Date**: 2026-06-11

**Update 2026-06-11**: The migration has reached the native-only cutover point in this repository. The Python implementation and Python project metadata have been removed, wrapper scripts now require `signal-light-native`, and hook installation has moved into the Rust CLI as `install-hooks`.

## Context

Vibecoding Signal Light currently uses Python for the CLI, Codex and Claude Code hook adapters, session aggregation, local display server, request handling, and MCP2221A GPIO control. The current implementation works, but the reliability profile is weaker than the product promise:

- Hook commands depend on the caller finding a compatible Python environment, project dependencies, and the expected wrapper behavior.
- Runtime startup spans PID files, file locks, temporary request and response files, timeouts, subprocess management, and hardware initialization.
- Physical hardware access depends on a Python-only MCP2221A library and can fail at import, initialization, device access, or write time.
- Hooks need to be quick and boring; environment-sensitive startup and file-based request polling make failures feel random to users.

The product is a local hardware utility. Users should experience it as a dependable command and ambient display, not as a Python project they have to nurse back to health.

## Decision

We will migrate the runtime toward a Rust native binary, while preserving the existing Python implementation as a fallback until the Rust path has passed behavior parity and reference hardware validation.

The migration will be phased:

1. **Behavior parity milestone**
   - Build a Rust command that supports dry-run signal behavior, signal listing, status-shaped output, hook event parsing, session key selection, and session aggregation.
   - Port the existing behavior tests from Python into Rust-level tests before replacing production paths.
   - Keep current signal names, command names, hook mappings, and lamp meanings unchanged.

2. **Hardware proof milestone**
   - Prove MCP2221A GPIO control from Rust against the reference traffic light.
   - Validate red, yellow, green, all-off, active-low, and active-high behavior.
   - Keep hardware access behind a driver abstraction so dry-run and test doubles remain first-class.

3. **Runtime server milestone**
   - Replace the environment-sensitive long-running Python display server with a Rust-owned local runtime process.
   - Prefer a direct local IPC mechanism for client/server requests instead of request-directory polling. The first implemented native transport uses a local request pipe plus per-request response files to preserve prompt handoff without reintroducing server-side file polling.
   - Preserve automatic startup, stale runtime recovery, session pruning, direct override behavior, status output, and persistent animations.

4. **Compatibility and rollout milestone**
   - Update wrapper scripts to prefer the Rust binary when present and fall back to Python during the transition.
   - Keep hook installation and repair flows compatible with existing Codex and Claude Code configurations.
   - Update README and lamp language documentation only when behavior or installation guidance changes.
   - Remove the Python fallback only after dry-run, hook, session aggregation, runtime recovery, and reference hardware checks are all green.

The repository has since completed the software cutover: runtime, hook adapters, hook installer, wrappers, tests, and project metadata are now native-only. Reference hardware validation remains a release checklist item, not a reason to keep Python code in the repository.

## Rationale

Rust is a good fit for this project because the unstable surface is mostly process lifecycle, local IPC, long-running state ownership, timing, hardware access, and packaging. A native binary reduces runtime environment drift and gives us tighter control over startup, error handling, and deployment.

The main risk is not the CLI or session logic. The main risk is MCP2221A control. The current Python implementation gets this through EasyMCP2221. The Rust migration must prove equivalent hardware behavior before the rest of the runtime is switched over.

The migration should therefore be evidence-led rather than a big-bang rewrite. We first lock behavior, then validate hardware, then replace the server path, then roll out.

## Consequences

### Positive

- Users can eventually install and run Signal Light without managing Python, virtual environments, or Python package dependencies.
- Hook commands become less sensitive to PATH, shell, and dependency differences between interactive terminals and agent hook environments.
- Runtime state, IPC, and process ownership can be made stricter and easier to reason about.
- Hardware errors can be surfaced through a single native command path with clearer diagnostics.
- Release artifacts can become closer to product packaging than source checkout setup.

### Negative

- The project gains a second implementation during migration.
- MCP2221A hardware support must be implemented or wrapped carefully and validated on real hardware.
- Contributors need a Rust toolchain for the new runtime path.
- Some logic will temporarily exist in both Python and Rust, so parity tests and documentation discipline are required.

### Neutral / Accepted Tradeoffs

- The temporary fallback was acceptable during migration; it has now been retired.
- Hook installer behavior stayed compatible while moving into the Rust CLI.
- The first Rust milestone may intentionally be dry-run only if hardware support needs deeper validation.

## Guardrails

- Do not change lamp semantics as part of the migration unless a separate spec explicitly calls for it.
- Do not ship hardware-facing release claims before reference hardware validation succeeds.
- Keep hardware access behind a driver boundary with dry-run support.
- Keep hook commands fast; long animations must remain owned by the local runtime process.
- Keep documented environment variables stable or provide explicit migration notes.
- Every migrated behavior must have parity tests or documented hardware verification.

## Alternatives Considered

### Stabilize the Existing Python Runtime Only

This would reduce immediate work and preserve the current dependency stack. It does not solve the core packaging and hook-environment fragility, and it leaves hardware access tied to a Python dependency path.

### Keep Python CLI and Add a Rust Hardware Helper

This reduces hardware risk in isolation but leaves process startup, session ownership, and IPC in the current Python runtime. It is useful as an intermediate experiment but not as the final architecture.

### Rewrite Everything in One Step

This is faster on paper but carries too much risk for hardware behavior and hook compatibility. A phased migration keeps the working reference path available while each risky boundary is proven.

## Links

- Feature specification: [specs/001-rust-runtime-migration/spec.md](../../specs/001-rust-runtime-migration/spec.md)
- Lamp language documentation: [docs/LAMP_LANGUAGE.md](../LAMP_LANGUAGE.md)
- Native runtime entry point: [native/src/runtime/server.rs](../../native/src/runtime/server.rs)
- Native hardware adapter: [native/src/drivers/mcp2221.rs](../../native/src/drivers/mcp2221.rs)
- Native hook installer: [native/src/install_hooks.rs](../../native/src/install_hooks.rs)
