# Task Contract

Use this for non-trivial implementation work when a durable task artifact reduces ambiguity or coordination cost. Delete irrelevant sections. Do not fill it mechanically for trivial edits.

## Goal / why

What observable outcome must be achieved, and why does it matter?

## Current -> desired behavior

- **Current:**
- **Desired:**

## Relevant source of truth

List only the files/docs/current external sources that materially govern this task.

Typical project sources:

- `README.md`
- `AGENTS.md`
- `docs/product-requirements.md`
- `docs/architecture.md`
- current nearby code/tests
- current Codex/Microsoft primary source when protocol/platform facts matter

## Scope / non-goals

### In scope

-

### Out of scope

-

## Material constraints

Record only constraints capable of changing the solution.

For this repository, commonly relevant constraints include:

- passive/read-only Codex observation,
- no interference with Codex App actionable requests,
- Windows-first support,
- startup/idle memory/CPU/event-latency budgets,
- no WebView/browser stack without explicit design change,
- no database/network backend unless justified,
- current Codex protocol/version limitations.

## Reuse / existing path

What should be inspected/reused before custom implementation?

Consider in order:

1. current repository implementation,
2. official Codex capability/protocol,
3. official Microsoft Windows/Rust capability,
4. relevant maintained OSS/reference implementation,
5. thin adapter,
6. custom implementation.

If custom work remains, state why it is the better lifecycle trade-off when non-obvious.

## Design notes — only when needed

Capture the minimum needed to prevent guessing:

- responsibility / state ownership,
- protocol or internal contract,
- important failure/reconnect behavior,
- concurrency/threading behavior,
- performance implications,
- credible alternative when the choice is non-obvious.

Do not prescribe local helper/class/function details unless they affect a durable boundary.

## Acceptance criteria

Write observable/testable completion conditions.

- [ ]

## Verification / evidence plan

What evidence can actually falsify the important completion claims?

Use the applicable subset:

- deterministic unit/reducer tests,
- protocol fixture tests,
- canonical Rust verification,
- live Codex coexistence/integration check,
- native Windows runtime inspection,
- performance/resource measurement,
- fresh diff review.

Optional mapping for material criteria:

| Criterion | Required evidence | How / where |
| --- | --- | --- |
|  |  |  |

Planned evidence that was not actually produced remains unverified.

## Stop / escalation conditions

Stop and surface the issue rather than guess when any of the following materially applies:

- current Codex protocol behavior contradicts the assumed design,
- passive observation cannot be proven non-interfering,
- the task requires answering/acknowledging actionable app-server requests,
- an official/native capability required by the design is unavailable,
- scope must materially expand into an out-of-scope product area,
- a large dependency/framework/runtime is required contrary to current architecture,
- required verification cannot be produced safely,
- a performance requirement must be relaxed without measurement.

## Completion report

At completion, report only what matters:

- observable behavior changed,
- checks/evidence actually run,
- measured performance impact when relevant,
- Codex/Windows/tool version when material,
- anything still unverified/blocked,
- durable docs/decisions changed when required.

## Blueprint provenance

Adapted from `templates/task-contract.md` in `miki-thecat/software-engineering-blueprint@docs/blueprint-v1.0-rc1` and narrowed to `agent-hud` failure modes.
