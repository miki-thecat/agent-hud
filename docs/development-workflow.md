# Development Workflow

This is the project-local Golden Path for AI-assisted implementation. It is adapted from the reusable software-engineering blueprint and intentionally kept thin.

## Default path

`Frame -> Inspect -> Reuse/Decide -> Design to actual risk -> Implement small -> Verify -> Fresh review when useful -> Integrate -> Learn`

Use `templates/task-contract.md` for a non-trivial implementation when a durable task artifact reduces ambiguity. Do not create ceremony for trivial edits.

## 1. Frame

Before implementation, establish only what prevents building the wrong thing:

- observable goal,
- current -> desired behavior,
- scope and non-goals,
- material constraints,
- acceptance criteria,
- unresolved assumptions capable of changing the solution.

For `agent-hud`, performance and non-interference with Codex App are often material constraints, not optional polish.

## 2. Inspect

Before editing, inspect the smallest relevant set:

- `README.md`,
- `AGENTS.md`,
- applicable requirements/architecture docs,
- current nearby code/tests,
- current Codex protocol/source when the task depends on version-sensitive behavior,
- current Microsoft Windows/Rust API source when a platform fact materially affects the design.

Do not rely on an old chat statement when the external protocol/API is capable of changing.

## 3. Reuse / decide

Check in this order when relevant:

1. existing repository implementation,
2. official Codex capability/API/protocol,
3. official Microsoft Windows/Rust capability,
4. mature maintained OSS/reference implementation,
5. thin adapter,
6. custom implementation.

Reference repositories are evidence and examples, not automatic dependencies. Reuse ideas/contracts deliberately; do not import a large stack merely because it already works elsewhere.

If technical feasibility is the largest uncertainty, run a narrow spike before full implementation.

## 4. Design only to actual risk

A simple local UI edit needs little design. A change to the Codex observation path deserves explicit reasoning because it can interfere with the primary Codex client.

Address only concerns that can materially change the solution:

- component responsibility,
- protocol/state ownership,
- passive-observer safety,
- connection/reconnect behavior,
- performance/resource bounds,
- Windows lifecycle/rendering behavior,
- testability and evidence.

Do not introduce generalized architecture for hypothetical future multi-provider or cross-platform support.

## 5. Implement small

Prefer coherent, independently verifiable slices.

Recommended early sequence:

### Slice 0 — protocol feasibility

Prove safe native-Windows observation before assuming the full product can work as designed.

### Slice 1 — normalized state core

Implement small protocol-independent session state/reducer with deterministic fixtures.

### Slice 2 — minimal native window

Render mock session rows through the selected Windows stack and measure startup/idle footprint.

### Slice 3 — live integration

Connect verified observations to the reducer/UI with clear degraded behavior.

### Slice 4 — polish only after core evidence

Improve compact layout, truncation, ordering, reconnect behavior, and only then optional convenience features.

Do not combine a large UI framework experiment, protocol rewrite, dependency upgrade, and product feature into one opaque change.

## 6. Verify observable claims

Planned checks are not evidence until run.

### Canonical static/build path

Once Rust bootstrap exists, converge on one repository command/script that includes the applicable subset:

```text
cargo fmt --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

Humans, Codex, and CI should call the same underlying verification contract.

### Runtime/protocol evidence

For Codex integration, directly verify coexistence with the actual target Codex version when feasible. Unit tests cannot prove that a second client does not interfere with real approval/user-input routing.

### Performance evidence

When a change can affect performance, measure the relevant requirement rather than writing "lightweight" in the completion report.

Useful evidence includes:

- startup measurement,
- Task Manager/ETW/WPR/WPA resource traces where appropriate,
- working-set comparison,
- idle CPU/wakeup behavior,
- event-to-paint latency.

Keep measurements attributable to a release build and representative Windows environment.

## 7. Review / evaluate

Use a fresh reviewer context for important AI-generated changes when practical, especially:

- protocol/session ownership changes,
- unsafe/privileged API use,
- concurrency/reconnect logic,
- performance-sensitive architecture,
- large dependency additions.

The reviewer should inspect the actual diff and evidence and try to falsify the important claims.

## 8. Integrate

Prefer short-lived branches/worktrees and small PRs.

When multiple Codex tasks run in parallel:

- start from an explicit base commit,
- give each task clear file/area ownership when overlap is plausible,
- avoid concurrent edits to shared core contracts unless coordinated,
- keep runtime/test artifacts isolated if simultaneous execution can collide,
- integrate serially and re-run canonical verification against the updated base.

Parallelism is an output of dependency/ownership design, not a target number.

## 9. Learn and simplify

After a recurring problem, ask:

1. What allowed or hid the failure?
2. What is the cheapest durable fix — type, test, boundary, script, protocol adapter, doc, or tooling change?
3. Is the problem important/recurring enough to justify ongoing complexity?
4. Can something else be removed at the same time?

Do not grow the root agent prompt for every one-off mistake.

## Definition of Done for normal non-trivial work

Use only the applicable subset:

- acceptance criteria demonstrated,
- relevant tests/checks passing,
- canonical verification passing,
- live runtime/protocol behavior inspected when required,
- performance measured when the change can affect a performance requirement,
- diff reviewed,
- no unjustified dependency/architecture expansion,
- anything unverified stated explicitly,
- durable requirements/architecture docs updated when accepted behavior or boundaries changed.

## Codex prompt guidance

A good implementation prompt should be contract-prescriptive, not implementation-micromanaging.

Include:

- goal / why,
- current -> desired behavior,
- exact scope/non-goals,
- relevant source files/docs to inspect,
- material constraints,
- required reuse/reference path,
- acceptance criteria,
- verification expectations,
- stop/escalation conditions.

Let Codex decide local helper names and internal implementation details unless they materially affect a durable boundary or performance requirement.

## Blueprint provenance

Adapted primarily from `docs/implementation-workflow.md`, `docs/coding-agent-harness.md`, `docs/greenfield-bootstrap.md`, and `docs/verification-review.md` in `miki-thecat/software-engineering-blueprint@docs/blueprint-v1.0-rc1`.
