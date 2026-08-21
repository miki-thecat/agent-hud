# AGENTS.md

This file is the concise coding-agent map for `agent-hud`. It is intentionally smaller than the reusable blueprint it was adapted from.

## Mission

Build the smallest reliable native Windows HUD that makes parallel Codex session state glanceable while remaining fast, lightweight, and non-invasive.

Do not optimize for feature count, architecture fashion, agent autonomy, documentation volume, or framework convenience at the expense of startup time, idle resource use, correctness, or simplicity.

## Read first

For non-trivial work, inspect only the relevant subset:

1. `README.md` — product purpose and current technical direction.
2. `docs/product-requirements.md` — accepted product behavior, non-goals, and performance requirements.
3. `docs/architecture.md` — component boundaries and critical Codex-observation constraints.
4. `docs/development-workflow.md` — Golden Path for AI-assisted implementation.
5. `docs/references.md` — current external/reference repositories and what may or may not be reused.
6. `templates/task-contract.md` — use when a durable task artifact materially reduces ambiguity.

Do not load all documents into every task merely because they exist.

## Default path

`Frame -> Inspect -> Reuse/Decide -> Design to actual risk -> Implement small -> Verify observable behavior -> Fresh review when useful -> Integrate -> Learn`

## Non-negotiable product constraints

- **Performance is functionality.** Startup latency, idle CPU, memory footprint, wakeups, and event-to-paint latency are first-class requirements.
- **Keep the MVP read-only.** Do not add session steering, approvals, user-input answers, turn creation, interruption, or orchestration unless the product requirements are explicitly changed.
- **Do not interfere with Codex App.** A monitoring connection must not steal, duplicate, acknowledge, or compete for actionable app-server requests. If passive observation cannot be proven safe, fail closed.
- **Prefer event-driven state.** Do not add tight polling loops when a safe event/state source exists. Any reconciliation polling must be bounded and justified by observed protocol behavior.
- **Demand-driven rendering.** Do not introduce a continuous 60/120 FPS render loop for a mostly static HUD.
- **No WebView stack by default.** Electron, Tauri/WebView, React, Node.js, HTML/CSS/JS, or another browser-backed UI require an explicit accepted design change.
- **No persistence by default.** Do not add SQLite or another database unless restart reconstruction from Codex sources proves insufficient for a real requirement.
- **No unrelated integrations in the MVP.** GitHub, PR/CI, Git graphs, project-management features, cloud sync, analytics, and general agent orchestration are out of scope unless explicitly promoted into the product requirements.
- **Windows first.** Do not pay portability cost before another platform becomes an accepted requirement.
- **Use official/native capabilities before custom infrastructure.** Prefer current OpenAI Codex protocol surfaces and Microsoft Rust/Windows APIs where they satisfy the requirement.

## Critical Phase-0 rule

Before substantial UI work, prove the native-Windows Codex observation path with a narrow spike.

The spike must answer at least:

- Can the target Codex App sessions be discovered reliably?
- Can coarse status be observed without taking session ownership?
- Can `waitingOnApproval` / `waitingOnUserInput` be observed safely?
- Can useful current activity / latest message be obtained without subscribing in a way that duplicates actionable server requests?
- What happens when Codex App or app-server restarts?
- Which behavior is official/stable versus inferred/experimental?

Do not hide an unresolved negative result behind UI mocks.

## Architecture boundaries

Keep these responsibilities separate even if they initially live in one crate:

- **Codex adapter** — protocol/process integration only.
- **Session reducer/state** — converts observed protocol facts into small normalized application state.
- **UI/presentation** — renders state and handles local view interaction only.
- **Platform shell** — native window, paint/invalidation, lifecycle, optional OS notification hooks.

The UI must not parse raw Codex protocol messages directly. The Codex adapter must not own presentation policy.

## Implementation discipline

- Inspect current nearby code/tests before editing.
- Prefer small coherent changes that are independently verifiable.
- Search for an existing implementation or official API before creating a custom abstraction.
- Keep dependencies few and justified; generation speed is not lifecycle-cost evidence.
- Avoid generic framework layers, DI containers, plugin systems, async runtimes, or abstractions until concrete complexity requires them.
- A single blocking I/O/background thread is preferable to a larger runtime if it satisfies the protocol and shutdown requirements cleanly.
- Preserve unknown/additive protocol fields or tolerate them at the boundary where practical; Codex app-server evolves.
- Do not manufacture fine-grained activity labels from timestamps or guesses. Display only states supported by trustworthy evidence.

## Verification

Every non-trivial task must report what was actually verified.

Once the Rust bootstrap exists, the canonical fast verification path should cover the applicable subset of:

```text
cargo fmt --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

The repository should converge on one obvious `verify` entry point shared by humans, Codex, and CI rather than duplicating validation logic.

For protocol/runtime work, static checks are insufficient. Use deterministic fixtures plus a gated live integration path when current Codex is available.

For performance-sensitive changes, measure rather than assert. Record enough environment information to make comparisons meaningful.

## Review

For important AI-generated changes, use a fresh reviewer context when practical. The reviewer should inspect the actual diff and evidence, and attempt to falsify important claims rather than restating the builder report.

## Completion report

For non-trivial work report only the relevant subset:

- observable behavior changed,
- checks/evidence actually run,
- measured performance impact when relevant,
- current Codex/Windows/tool versions when they materially affect evidence,
- anything unverified or blocked,
- durable docs/decisions updated when behavior or architecture changed.

## Blueprint provenance

Adapted from `miki-thecat/software-engineering-blueprint` branch `docs/blueprint-v1.0-rc1`, especially its Golden Path, greenfield bootstrap, coding-agent harness, architecture guidance, verification discipline, and task-contract pattern. Project-local requirements override generic blueprint defaults when they conflict.
