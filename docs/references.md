# References

This file records the small set of repositories/sources worth inspecting before reinventing Codex monitoring or Windows rendering behavior. These are references, not automatic dependencies.

## Internal engineering source

### `miki-thecat/software-engineering-blueprint`

Use branch:

- `docs/blueprint-v1.0-rc1`

Relevant source areas for this project:

- `AGENTS.md`
- `docs/implementation-workflow.md`
- `docs/greenfield-bootstrap.md`
- `docs/coding-agent-harness.md`
- `docs/architecture-design.md`
- `docs/agent-execution-legibility-harness.md`
- `docs/verification-review.md`
- `templates/task-contract.md`

Do not copy the full blueprint into normal task context. Project-local docs already contain the promoted subset.

## Codex monitoring / protocol references

### `morgadoronan/codex-agents`

Repository: <https://github.com/morgadoronan/codex-agents>

Why it matters:

- Rust implementation focused specifically on a compact Codex-session dashboard.
- Demonstrates a small normalized state model such as Need input / Working / Completed.
- Explicitly distinguishes `waitingOnApproval` and `waitingOnUserInput` from generic active work.
- Contains useful architecture/protocol notes about passive observation and session ownership.
- Strong example of failing closed rather than pretending coarse timestamps reveal fine-grained activity.

Important limitation:

- Its live shared-state topology is built around a private Unix-domain-socket setup for macOS/Linux/WSL.
- Native Windows live shared status is explicitly unsupported in that design.
- It only shows manager-owned thread IDs, not arbitrary sessions discovered from other Codex processes.

Therefore: reuse its **safety principles, status normalization, protocol research, reducer/testing ideas**, but do not assume its process/socket topology solves our Codex App + native Windows requirement.

### `manuelsh/codex-monitor`

Repository: <https://github.com/manuelsh/codex-monitor>

Why it matters:

- Another independent Codex monitoring/dashboard implementation.
- Useful for understanding which app-server/session/usage information other clients have been able to expose.
- Useful as a behavior/reference source when validating our own protocol interpretation.

Why it is not our architecture:

- It is TypeScript-oriented and its product/UI scope is different.
- `agent-hud` deliberately prioritizes a much thinner native Windows executable.

### `pingdotgg/t3code`

Repository: <https://github.com/pingdotgg/t3code>

Why it matters:

- Large real-world Codex integration/control-plane codebase.
- Its Codex adapter normalizes app-server lifecycle items, request types, thread status, command/file-change/web-search activity, and other protocol events.
- Useful for checking practical mappings from evolving Codex app-server structures into stable application state.

Why it is not our product model:

- It is a broad alternate coding-agent frontend/control plane, far larger than this HUD.
- Do not copy its architecture wholesale; inspect only narrow Codex adapter/protocol patterns relevant to the current task.

## Primary Codex source

### `openai/codex`

Repository: <https://github.com/openai/codex>

Treat current OpenAI source/schema/documentation as higher authority than third-party monitor assumptions when they disagree.

Especially inspect current app-server material before protocol-sensitive work:

- app-server README/protocol docs,
- generated protocol schemas,
- notification/request registries,
- thread status / active flags,
- turn/item lifecycle notifications,
- transport/lifecycle behavior.

Codex app-server is evolving. Re-check current source for every design decision that depends on exact protocol semantics.

## Windows/Rust rendering references

### `microsoft/windows-rs`

Repository: <https://github.com/microsoft/windows-rs>

Current preferred platform source for this project.

Relevant crates/docs include:

- `windows-window` — thin native window/message-loop abstraction,
- `windows-canvas` — native 2D rendering path with Windows graphics/text stack,
- `windows` / `windows-sys` — lower-level Windows API access if a justified gap requires it.

Prefer the smallest official crate surface that satisfies the requirement before writing raw Win32 boilerplate.

### `zed-industries/zed`

Repository: <https://github.com/zed-industries/zed>

Use primarily as a performance-architecture reference, not as a dependency target.

Useful ideas:

- native Rust application rather than an Electron/WebView editor shell,
- direct Windows graphics/text integration,
- keep UI-thread work small,
- background expensive work,
- incremental/state-driven updates,
- measure performance rather than assuming framework choice is sufficient.

Do **not** copy editor-scale machinery, GPUI complexity, high-FPS rendering infrastructure, project indexing, or other systems that this tiny HUD does not need.

## Reuse rule

Before introducing a custom solution for Codex protocol handling or Windows platform behavior:

1. inspect current repository code,
2. inspect current official OpenAI/Microsoft source,
3. inspect the narrow relevant implementation in the references above,
4. choose the thinnest lifecycle-safe approach,
5. add a custom abstraction only where it creates a real stable boundary.

## Reference hygiene

- Record behavior, not folklore.
- Prefer current primary source over old README assumptions.
- Do not copy code without checking license and compatibility.
- Do not import a dependency simply because its repository is a useful reference.
- If a reference is stale or its architecture no longer matches current Codex, update this file rather than preserving it as permanent truth.
