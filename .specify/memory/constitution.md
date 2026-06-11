<!--
Sync Impact Report
Version change: 1.0.0 -> 2.0.0
Modified sections:
- Engineering Guardrails
Added sections:
- None
Removed sections:
- None
Templates requiring updates:
- ✅ no change required .specify/templates/plan-template.md
- ✅ no change required .specify/templates/spec-template.md
- ✅ no change required .specify/templates/tasks-template.md
Follow-up TODOs:
- None
-->
# Vibecoding Signal Light Constitution

## Core Principles

### I. Code Quality Is a Product Requirement
All production changes MUST prefer the existing `signal_light` package, script
wrappers, and runtime boundaries over new layers or duplicate flows. Modules,
functions, and public interfaces MUST stay small enough to review in one
sitting; magic values, environment variables, and hardware assumptions MUST be
named and documented at the declaration site. Any new abstraction, dependency,
or concurrency path MUST be justified in the implementation plan. Rationale:
this project coordinates local hooks and physical hardware, so unnecessary
complexity turns directly into harder debugging and less trustworthy behavior.

### II. Tests Prove Every Behavior Change
Every behavior change MUST add or update automated tests at the lowest sensible
level. Bug fixes MUST begin with a failing regression test unless the failure
can only be reproduced on physical hardware, in which case the change MUST
document the manual reproduction and verification procedure. Changes to signal
mapping, session aggregation, CLI parsing, hook installation, or runtime state
transitions MUST cover both success and failure paths. Rationale: the lamp is
only trustworthy when the agent state model is proven instead of guessed.

### III. User Experience Must Preserve Signal Semantics
Green, yellow, and red semantics MUST stay stable across the README, CLI,
hook adapters, dry-run output, and the physical lamp. Any user-visible rename,
new signal, changed flash or cycle meaning, or install flow adjustment MUST
update the signal mapping and operator guidance in the same change. New
features MUST preserve sensible defaults and clear recovery paths for both
Codex and Claude Code users. Rationale: this tool succeeds by being glanceable;
inconsistent semantics destroy that value quickly.

### IV. Performance Budgets Are Explicit
Hook entry points MUST hand work off quickly and MUST NOT hold the caller open
for long-running animation or polling. New or changed user-facing flows MUST
declare latency and cadence budgets in the spec and plan, with verification by
automated timing checks or documented manual measurement. Default expectations
for the reference implementation are: hook or event commands return in under
250 ms once the runtime server is available, display transitions complete in
under 1 second, and continuous animation avoids visible flicker on the
MCP2221A path. If first-start behavior is slower, the change MUST document the
difference and the recovery path. Rationale: the status light should reduce
interruption, not add it.

## Engineering Guardrails

- Runtime-affecting logic MUST live either in `signal_light/` or in an
  ADR-approved project-local native runtime subtree such as `native/`.
  `signal_light/` MUST remain the stable compatibility surface for wrapper
  orchestration, hook installation, and fallback behavior while a migration is
  in flight.
- Files in `scripts/` MUST stay thin entry points that delegate into tested
  compatibility code. That compatibility layer MAY invoke tested Python modules
  or an ADR-approved native runtime binary when the wrapper preserves the
  documented fallback and repair path.
- Hardware access MUST remain behind abstractions that allow dry-run behavior,
  test doubles, or equivalent non-hardware validation paths.
- Configuration MUST use documented environment variables with safe defaults.
  Undocumented behavior flags are not allowed.
- Any change to user-facing commands, environment variables, signal names, or
  installation behavior MUST update `README.md` in the same change.

## Delivery Workflow

- Every feature spec MUST describe impacted user journeys, UX consistency
  constraints, and measurable performance requirements when user-visible or
  latency-sensitive behavior changes.
- Every implementation plan MUST pass a constitution check covering code
  quality, test coverage, UX consistency, and performance verification.
- Every task list MUST include the automated test work, documentation updates,
  and validation steps needed to satisfy these principles.
- Before merge, contributors MUST run `pytest` and the relevant dry-run or CLI
  smoke checks. Hardware-affecting changes MUST also include either reference
  hardware validation or a documented reason that hardware verification was not
  available.

## Governance

This constitution overrides conflicting process notes elsewhere in the
repository. Amendments MUST update this file and any affected templates or
operator guidance in the same change. Versioning follows semantic versioning:
MAJOR for incompatible governance changes or principle removals, MINOR for new
principles or materially expanded obligations, and PATCH for clarifications
that do not change the compliance bar. Feature plans, task lists, and reviews
MUST include an explicit constitution compliance check; any exception MUST be
time-bounded, justified in writing, and recorded in the plan's Complexity
Tracking section or equivalent review notes.

**Version**: 2.0.0 | **Ratified**: 2026-06-11 | **Last Amended**: 2026-06-11
