# Architecture

## Purpose

This document records the smallest architecture currently justified for `agent-hud`.

The architecture optimizes for:

- trustworthy passive observation of Codex sessions,
- minimal startup and idle overhead,
- simple local state,
- demand-driven native rendering,
- easy replacement of unstable Codex protocol details without rewriting the UI.

## Current shape

```text
Codex App / app-server
        |
        | observed protocol facts
        v
+------------------------+
| Codex adapter          |
| - connect/discover     |
| - parse protocol       |
| - reconnect            |
| - no presentation      |
+-----------+------------+
            |
            | normalized observations
            v
+------------------------+
| Session reducer/state  |
| - tiny in-memory model |
| - status normalization |
| - latest activity/msg  |
+-----------+------------+
            |
            | changed state only
            v
+------------------------+
| Native UI              |
| - Windows window       |
| - Direct2D/DirectWrite |
| - demand repaint       |
+------------------------+
```

A platform shell may remain a thin separate module around native lifecycle, paint invalidation, DPI, and optional notification integration.

## Current technology decision

Windows-first candidate stack:

- Rust,
- Microsoft `windows-window`,
- Microsoft `windows-canvas`,
- Direct2D / Direct3D 11 / DXGI,
- DirectWrite for text,
- memory-only session state,
- standard-library threading/channels unless measured complexity requires more.

The focused `windows-window` and `windows-canvas` crates are currently consumed
from the Microsoft `windows-rs` repository revision selected by Cargo because
the documented `0.100` examples are not published as usable crates.io releases
in the current installation. The API is the official Microsoft stack; the git
source can move to a released version once those crates are published.

This is deliberately thinner than a browser-backed desktop stack.

Do not add Electron, Tauri/WebView, Node.js, React, or another browser runtime merely for UI convenience without an accepted architecture change backed by evidence that the native path is the wrong lifecycle trade-off.

## Component contracts

### Codex adapter

Owns all Codex-specific protocol/process behavior.

Responsibilities:

- identify the currently supported observation path,
- perform required initialization/handshake,
- discover relevant sessions,
- receive or obtain status/activity/message facts,
- tolerate additive unknown fields where practical,
- reconnect after supported lifecycle changes,
- convert raw protocol data into internal observations,
- expose uncertainty rather than fabricate state.

Must not:

- answer approval requests,
- answer user-input requests,
- start/steer/interrupt turns in the MVP,
- take over session ownership,
- leak raw protocol concerns into rendering code.

### Session reducer/state

Owns stable product-facing state.

Conceptually:

```text
SessionState {
  id
  title
  status
  current_activity?
  latest_message?
  started_at?
  last_activity?
}
```

Exact Rust types should emerge from the verified protocol rather than being frozen prematurely.

The reducer should be deterministic and easy to test with fixtures.

### UI

Owns presentation only.

Responsibilities:

- render the compact list,
- prioritize actionable sessions,
- truncate text safely,
- render elapsed/relative time,
- handle local selection/scrolling,
- request repaint only when visible state or a necessary time label changes.

The UI must not infer protocol state from raw JSON, process names, filesystem timestamps, or other weak signals.

## Concurrency model

Default to the smallest model that remains correct:

- one UI thread,
- one background blocking I/O thread for Codex observation,
- a bounded channel or equivalent small handoff into the application state.

Do not add Tokio or a larger async runtime until actual protocol/concurrency requirements make the simpler model materially worse.

If multiple independent streams become necessary later, revisit the decision with measured complexity rather than pre-installing orchestration.

## Rendering model

`agent-hud` is mostly static. It should not behave like a game loop.

Preferred model:

```text
state changes
    -> update reducer
    -> invalidate window
    -> paint once
    -> sleep
```

Avoid:

- permanent 60/120 FPS loops,
- animation-driven redraw when no useful state changed,
- repainting for every streaming token,
- expensive visual effects,
- unnecessary retained off-screen surfaces.

The standalone canvas host applies the current per-monitor DPI to the swap
chain on DPI messages. It intentionally keeps the raw-HWND composition scale at
identity because this is not a composition surface, and recreates the focused
`GpuDevice`/swap chain when `windows-canvas` reports device loss. This keeps the
native lifecycle local to the platform shell without adding a UI framework.

For latest-message streaming, coalesce deltas or prefer meaningful completion boundaries rather than painting every token.

## Persistence

No application database in the MVP.

On startup, reconstruct the currently useful state from the verified Codex source when possible.

Persistence may be introduced only for a concrete requirement such as durable user configuration that cannot be represented more simply. Session history itself is not an MVP requirement.

### Persisted discovery boundary

Phase 1C validates a deliberately bounded local discovery catalog for the
current installation: select unarchived `threads` rows with
`thread_source=user`, validate their `id -> rollout_path -> session_meta`
identity chain, and retain a fixed recent maximum (20). This is a **Recent
local sessions** catalog, not evidence of currently open Codex App chats or
Desktop-window ownership. Recency is permitted only to sort/bound persisted
history; it must not establish a live or working state. Exclude subagent rows
and fail closed on identity mismatch. See
`docs/research/phase-1c-session-discovery-result.md`.

## Network

No separate application network dependency.

The HUD should not send monitoring data to a cloud service or call an LLM to summarize session content.

Any communication required to observe local Codex must remain scoped to the local Codex integration path.

## Critical trust boundary: passive observation

The most important architecture risk is not graphics. It is whether a second observer can safely inspect live Codex state while Codex App remains the interactive owner.

Reference implementations show that careless subscription can be unsafe because actionable app-server requests may be duplicated to subscribed clients. Therefore:

- never assume a second subscription is passive,
- never acknowledge an actionable request merely because it arrived,
- do not call thread/session ownership APIs without proving their semantics,
- fail closed when the observer cannot prove that it will not interfere.

The native-Windows path must be verified directly because Unix-domain-socket approaches used by some existing monitors do not transfer automatically to native Windows.

## Phase-0 feasibility spike

Before building the production UI, create a narrow diagnostic executable or test harness that proves the observation contract.

Required questions:

### Discovery

- Can sessions created by Codex App be listed/discovered?
- Are all intended sessions visible or only sessions owned by the observer/manager?
- Are identifiers stable across reads/restarts?

### State

- Which coarse thread states are exposed?
- Are `waitingOnApproval` and `waitingOnUserInput` available from a safe read path?
- Can current activity/latest message be observed without unsafe subscription?

### Coexistence

- Does the observer cause duplicated approval/user-input requests?
- Does it alter Codex App behavior or ownership?
- Can it disconnect without affecting the active session?

### Lifecycle

- What happens when Codex App closes/reopens?
- What happens when app-server restarts?
- What is the authoritative reconnect/discovery behavior?

### Versioning

- Which protocol details are documented/current?
- What behavior is experimental, inferred, or version-sensitive?

The spike should produce a small written result before the rest of the application assumes a particular topology.

## Reconciliation

Pure event-driven observation is preferred when safe and reliable.

If verified Codex behavior can lose or expose stale events, a slow bounded reconciliation read may be justified. It must be treated as a correctness repair mechanism, not an excuse for high-frequency polling.

Any polling interval should be chosen from observed failure behavior and measured resource cost.

## Error/degraded behavior

The application should remain honest under partial information.

Examples:

- app-server unavailable -> show disconnected/degraded state,
- unsupported protocol version -> surface incompatibility rather than guessing,
- fine-grained activity unavailable -> show trustworthy coarse status only,
- message unavailable -> omit it rather than scraping fragile UI text,
- reconnecting -> preserve only state whose freshness is still defensible.

## Performance architecture

Performance-sensitive defaults:

- native compiled executable,
- no browser runtime,
- no application DB,
- no network backend,
- minimal dependencies,
- no continuous renderer,
- bounded state and queues,
- coalesced text updates,
- no repeated whole-machine/process/filesystem scanning,
- no background work unrelated to currently visible product value.

## Testing strategy

### Reducer/unit tests

Use deterministic protocol fixtures to verify normalization and transitions.

### Adapter tests

Separate parser/protocol tests from live Codex tests.

### Live integration tests/spike

Gate tests that require a locally installed compatible Codex App/CLI/app-server. They should be explicit rather than making ordinary unit tests depend on personal machine state.

### UI tests

Prefer focused logic/layout tests where available plus direct runtime inspection for the small number of important visual/interaction claims.

### Performance tests

Measure at least:

- cold/warm startup,
- idle working set,
- idle CPU/wakeups,
- event-to-paint latency,
- behavior with representative concurrent session counts.

Record enough Windows/hardware/build information to make comparisons meaningful.

## Change rule

Add architecture only when it controls a real failure mode or enables accepted product behavior.

A proposal for another layer, service, runtime, database, framework, cache, plugin system, abstraction, or background worker should answer:

1. What current failure/requirement does it solve?
2. Why is the simpler existing path insufficient?
3. What measurable lifecycle cost does it add?
4. How will we know the change paid for itself?

## Blueprint provenance

Adapted from `software-engineering-blueprint` branch `docs/blueprint-v1.0-rc1`, especially `architecture-design.md`, `greenfield-bootstrap.md`, `coding-agent-harness.md`, `agent-execution-legibility-harness.md`, and `verification-review.md`.
