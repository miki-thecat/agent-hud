# agent-hud

A tiny, native, Windows-first HUD for monitoring parallel Codex sessions without living inside every Codex App thread.

## Product goal

`agent-hud` should answer, at a glance:

- Which Codex sessions are working?
- Which sessions need user input or approval?
- What is each session doing right now?
- What did Codex most recently report?
- How long has it been running, and when was its last activity?

Codex App remains the primary place for conversation and implementation. `agent-hud` is a read-only monitoring surface, not a replacement frontend or orchestrator.

## MVP

Each visible session should expose only the useful minimum:

- task/session title,
- normalized status,
- current activity when trustworthy,
- latest Codex message, truncated to roughly 1–2 lines,
- elapsed time,
- last-activity time.

The default presentation should prioritize sessions that need human attention, while staying visually compact.

## CLI watcher

The current core-data validation surface can run as a read-only event-driven
watcher:

```text
cargo run -- --watch
```

It prints only bounded root/user session IDs and normalized recorded readiness.
Rollout appends are parsed incrementally; persisted discovery/WAL changes cause
a bounded reconciliation. The output is still a **Recent local sessions**
view, not a claim about currently open Codex App chats.

On Windows, launching the binary without `--watch` opens the native HUD. It
uses the same typed watcher state and labels the bounded list **Recent local
sessions** so persisted discovery is not confused with exact open chats.

## Product principles

1. **Fast and lightweight first.** Startup latency, idle CPU, memory footprint, and event-to-paint latency are product requirements.
2. **Read-only by default.** Observation must not compete with Codex App for approvals, user-input requests, turns, or session ownership.
3. **Event-driven where safe.** Do not introduce high-frequency polling, continuous rendering, filesystem scans, or network work without measured need.
4. **One small surface.** Avoid becoming a Git client, project manager, chat client, IDE, or agent orchestrator.
5. **Evidence over assumptions.** Codex App/app-server behavior can change; verify protocol behavior against current sources and runtime before depending on it.
6. **No speculative infrastructure.** No database, cloud backend, telemetry stack, plugin system, or framework layer unless a demonstrated requirement justifies it.

## Current technical direction

The preferred implementation direction is deliberately thin:

```text
Codex App / app-server
        |
        | JSON-RPC state/events (only when safe to observe)
        v
background observer
        |
        v
small in-memory session state
        |
        v
native Windows UI
```

Current candidate stack:

- Rust,
- Microsoft `windows-window`,
- Microsoft `windows-canvas`,
- Direct2D / Direct3D 11 / DirectWrite through the Windows Rust stack,
- memory-only application state,
- demand-driven rendering.

This stack is a current design decision, not a license to skip measurement. If a narrow Phase-0 spike disproves a critical assumption, revise the design before building the full UI.

## Critical Phase-0 risk

Before substantial UI implementation, prove that a separate native-Windows process can observe the Codex App sessions we care about **without stealing, duplicating, or interfering with actionable server requests**.

If fine-grained passive observation is not safely available, fail closed and reduce scope rather than building an unsafe pseudo-observer.

See:

- `AGENTS.md`
- `docs/product-requirements.md`
- `docs/architecture.md`
- `docs/development-workflow.md`
- `docs/references.md`
- `templates/task-contract.md`

## Documentation

- [`docs/user-guide.md`](docs/user-guide.md) — prerequisites, build/run commands, and how to read the HUD.
- [`docs/architecture.md`](docs/architecture.md) — current data flow and component boundaries.
- [`docs/design/readiness-state-model.md`](docs/design/readiness-state-model.md) — readiness invariants and conservative state transitions.
- [`docs/contributing.md`](docs/contributing.md) — isolated worktrees, verification, and contribution workflow.

Validation evidence:

- `docs/research/issue-16-readiness-validation.md`

## Engineering source

The repository workflow is adapted from `miki-thecat/software-engineering-blueprint`, using branch `docs/blueprint-v1.0-rc1` as the current convergence baseline. Only the subset relevant to this product is promoted here; the full blueprint is intentionally not copied into every task context.
