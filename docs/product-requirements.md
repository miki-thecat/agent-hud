# Product Requirements

## Product statement

`agent-hud` is a tiny native Windows HUD for developers running multiple Codex sessions in parallel. It lets the user understand session state without repeatedly opening and scanning every Codex App thread.

The application is a companion to Codex App, not a replacement for it.

## Primary user problem

When several Codex sessions are running concurrently, the user loses time repeatedly checking individual threads to answer two questions:

1. Does any session need me right now?
2. What are the other sessions currently doing?

`agent-hud` should make those answers visible in one compact surface with negligible system overhead.

## Primary outcome

A user with roughly 3–20 concurrent Codex sessions should be able to glance at one window and immediately understand:

- which sessions are actively working,
- which need user input or approval,
- which are idle/completed/error states when those states are trustworthy,
- the current activity or latest trustworthy activity description,
- the most recent Codex message in roughly one or two lines,
- elapsed time and last activity.

## MVP information per session

Display the applicable subset:

- **Title / task name**
- **Status**
- **Current activity** — only when directly supported by trustworthy protocol/runtime evidence
- **Latest Codex message** — compact, approximately 1–2 lines
- **Elapsed time**
- **Last activity**

The first version does not need a rich detail pane if the main list is sufficient.

## Normalized status model

The exact mapping depends on the currently verified Codex app-server protocol. The UI should normalize protocol facts into a small stable product vocabulary, such as:

- `WORKING`
- `NEEDS_INPUT`
- `APPROVAL`
- `IDLE` / `DONE` when distinguishable
- `ERROR`
- `UNKNOWN` when evidence is insufficient

Never infer a precise state merely to avoid showing `UNKNOWN`.

## Information hierarchy

The most important information is human intervention.

Default ordering should therefore place actionable sessions before ordinary working/idle sessions. The UI should remain compact rather than turning this into a project-management dashboard.

## Interaction

MVP interaction should be minimal:

- launch/close the HUD,
- view live session state,
- basic scrolling if necessary,
- optionally select a session.

Opening the corresponding Codex App session from a row is desirable only if a reliable supported activation path exists. It is not allowed to drive architecture or delay the core monitor.

## Explicit non-goals for MVP

Do **not** build:

- a Codex chat/composer replacement,
- session steering or orchestration,
- automatic approvals or user-input answers,
- a Git client or GitKraken replacement,
- GitHub PR/CI integration,
- an implementation DAG/project manager,
- task assignment,
- agent spawning,
- cloud sync/account system,
- analytics/telemetry service,
- database-backed session history,
- cross-device sync,
- a plugin system,
- broad multi-agent-provider support,
- macOS/Linux support merely for completeness.

These may be reconsidered only after the core monitoring experience proves useful.

## UX principles

### Glanceable

The user should not need to read a dashboard. The important state should be obvious in seconds.

### Quiet

Avoid animation, flashing, graphs, decorative metrics, and dense controls. Motion exists only when it materially improves state recognition.

### Compact

Prefer one window and one main list. Avoid sidebars, nested navigation, chat history, and permanent detail panels unless real use shows they are needed.

### Honest

Do not display invented precision. If Codex exposes only coarse `active`, show a coarse working state rather than guessing `CODING`, `TESTING`, or `THINKING` from weak signals.

## Performance requirements

Performance is part of the product, not an afterthought.

Initial engineering budgets are targets to validate on representative Windows hardware, not externally guaranteed specifications:

| Metric | Initial target |
| --- | ---: |
| Cold startup to useful window | < 150 ms target |
| Warm startup | < 75 ms target |
| Idle CPU | effectively 0% in normal Task Manager observation |
| Working set | < 20 MB target; investigate meaningful regressions |
| Event-to-visible-state update | < 50 ms target when the upstream event is available |
| Background network | none |
| Continuous render loop | none |
| High-frequency filesystem scan | none |

If a target proves unrealistic because of the selected official Windows/Codex integration, measure and record the reason before changing the budget.

## Resource policy

Prefer:

- demand-driven repaint,
- state updates only when facts change,
- a small in-memory state model,
- bounded queues,
- minimal threads,
- coalesced text/delta updates rather than painting on every token,
- no database unless a demonstrated restart/recovery requirement needs one.

## Reliability / protocol behavior

`agent-hud` must tolerate normal Codex App/app-server lifecycle changes without corrupting state or interfering with the primary client.

Expected design concerns include:

- Codex App/app-server restart,
- connection loss and reconnect,
- additive/unknown protocol fields,
- stale or coarse status,
- unavailable fine-grained activity,
- sessions appearing/disappearing,
- current protocol behavior changing between Codex versions.

When live status becomes unreliable, prefer an explicit degraded/unknown state over fabricated continuity.

## Privacy / local behavior

MVP should remain local-only.

Do not transmit prompts, messages, titles, file paths, repository names, or session metadata to a separate service for analytics, enrichment, or summarization.

Do not log message/prompt content by default. Diagnostics should be content-minimal and introduced only when needed to diagnose real failures.

## Acceptance criteria for the first useful release

- [ ] The app starts as a small native Windows window without a browser/WebView runtime.
- [ ] It discovers the intended concurrently running Codex sessions through a verified safe path.
- [ ] It shows a trustworthy normalized state for each discovered session.
- [ ] Actionable `waitingOnApproval` / `waitingOnUserInput` state is represented when safely observable.
- [ ] It displays useful current/latest activity information without inventing unsupported detail.
- [ ] State changes appear without repeatedly opening Codex App threads.
- [ ] Monitoring does not interfere with Codex App's ownership of approvals, user input, turns, or execution.
- [ ] Canonical Rust verification passes.
- [ ] Startup, idle CPU, memory, and event-update latency are measured against the performance budgets.

## Requirement-change rule

If a proposed feature does not improve the core loop — glance, identify attention, understand current work — it should default to out of scope until real use demonstrates otherwise.
