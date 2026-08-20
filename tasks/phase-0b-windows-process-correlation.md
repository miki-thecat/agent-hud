# Phase 0B — Windows process/IPC correlation spike

## Goal / why

Determine how much of Codex Desktop's live activity can be reconstructed safely from native Windows OS telemetry when direct attachment to the Desktop-owned app-server is unavailable or unsupported.

The goal is not to invent a heuristic dashboard. The goal is to identify trustworthy, low-overhead observation signals and establish their claim ceiling.

## Current -> desired behavior

- Current: Codex Desktop and one or more local `codex.exe` processes are visible to Windows, but the relationship between OS processes and Codex Thread/session identity is not yet proven.
- Desired: produce measured evidence showing which of the following can be observed reliably and which cannot:
  - Codex Desktop process tree,
  - app-server process identity,
  - child process creation/exit,
  - child command line,
  - parent PID,
  - process cwd when obtainable without invasive techniques,
  - CPU / I/O activity where useful,
  - whether any process/IPC metadata exposes a stable thread/session identifier,
  - whether child processes can be correlated to a specific Codex Thread with strong evidence.

## Sources / constraints

Read first:

- `AGENTS.md`
- `docs/architecture.md`
- `docs/product-requirements.md`
- `docs/research/windows-codex-observation-2026-08-20.md`
- `tasks/phase-0-windows-observation.md`

External primary references to re-check if details matter:

- Microsoft ETW process events (`EVENT_TRACE_FLAG_PROCESS`)
- Microsoft Process/Process_TypeGroup1 ETW schema
- Tool Help process snapshots (`PROCESSENTRY32`)
- current OpenAI Codex app-server / Windows behavior

## In scope

Build the smallest Rust diagnostic probe needed to inspect the local process topology and process lifecycle while Codex Desktop is running.

Preferred observation order:

1. ordinary process snapshot APIs for baseline topology,
2. documented command-line/process metadata APIs,
3. ETW process start/stop events if needed for reliable temporal correlation,
4. documented local IPC endpoint discovery (handles/endpoints only when obtainable through normal OS APIs and without reading process memory),
5. correlation experiments using deliberately distinguishable commands launched from separate Codex sessions.

Example controlled experiment:

- Open two different Codex Desktop threads with distinct working directories.
- Ask Thread A to execute a harmless long-enough command with an unmistakable command line in directory A.
- Ask Thread B to execute a different harmless command in directory B.
- Record process start events, parent PID, command line, timestamps and any cwd/session metadata available.
- Repeat multiple times and with concurrent commands.
- Determine whether a deterministic mapping exists or only a probabilistic temporal heuristic.

## Out of scope / prohibited

Do NOT:

- inject code into Codex/Desktop/app-server processes,
- attach a debugger to capture internal state,
- read another process's arbitrary memory,
- hook private functions,
- scrape the Codex UI,
- intercept credentials/tokens,
- alter Desktop-owned stdin/stdout handles,
- answer approvals/user-input requests,
- resume/start/steer/interrupt Desktop-owned threads,
- claim a Thread mapping from timing proximity alone.

Do not request administrator privilege unless a specific read-only Windows API genuinely requires it and the evidence value justifies the escalation. Prefer normal-user APIs.

## Design requirements

Keep the spike disposable and narrow.

Suggested module split only if useful:

```text
src/
  main.rs
  process_snapshot.rs
  process_events.rs      # ETW only if needed
  correlation.rs
```

No GUI. Output structured human-readable diagnostics, preferably JSON Lines or a compact table.

Avoid Tokio unless the chosen ETW/library API clearly makes it the simpler path.

## Evidence classes

Classify each signal:

### AUTHORITATIVE_OS
Directly reported by Windows, e.g. PID, parent PID, image path, process create/exit event, captured command line.

### STRONG_CORRELATION
A mapping supported by a stable identifier or repeatable structural relationship, not merely timestamp proximity.

### HEURISTIC
Useful operational hint but insufficient for a correctness claim, e.g. nearest process creation time.

### UNAVAILABLE
No safe supported path found.

The production HUD must never upgrade HEURISTIC evidence into an authoritative Codex Thread state.

## Key questions

1. How many `codex.exe` / app-server processes exist for one Desktop instance?
2. Does each Desktop thread have a distinct OS process, or are multiple threads multiplexed inside one app-server process?
3. When a thread runs a shell command, what exact parent/ancestor process creates it?
4. Do concurrent commands from different threads share the same parent app-server PID?
5. Is cwd present in a documented process/event surface, or only inferable/expensive to retrieve?
6. Does any command-line/environment/handle/IPC metadata include Codex thread/turn/item identifiers?
7. Can a stable thread mapping be obtained without app-server semantic access?
8. Can process activity distinguish WORKING vs WAITING_FOR_APPROVAL vs WAITING_FOR_USER_INPUT vs IDLE? If not, record this explicitly.
9. What is the steady-state CPU/RAM/wakeup cost of the observation method?

## Acceptance criteria

- [ ] Enumerate the Codex Desktop/app-server process tree on the user's Windows machine.
- [ ] Capture process start/stop events or an equivalent reliable lifecycle signal during controlled Codex commands.
- [ ] Capture PID, parent PID, executable and command line where available.
- [ ] Run at least two distinct-thread correlation experiments and one concurrent experiment.
- [ ] State whether multiple Codex threads are multiplexed through one app-server process.
- [ ] State whether a deterministic Thread ↔ child-process mapping exists from OS-level signals alone.
- [ ] State which statuses cannot be derived from process telemetry.
- [ ] Measure approximate idle observer CPU/working-set overhead for the chosen probe.
- [ ] Produce `docs/research/phase-0b-process-correlation-result.md` with evidence and a verdict.

## Verdicts

Use exactly one primary verdict:

- `STRONG_PROCESS_CORRELATION_AVAILABLE`
  - OS-level telemetry exposes a stable, reproducible mapping useful for production.

- `PROCESS_ACTIVITY_ONLY`
  - OS telemetry can show commands/activity but cannot reliably identify the owning Codex Thread.

- `NO_USEFUL_LOW_LEVEL_PATH`
  - even process telemetry does not add enough trustworthy information to justify integration.

A secondary note may state whether combining process telemetry with another safe semantic source could close the gap.

## Stop / escalation conditions

Stop instead of escalating invasiveness when the next step would require memory scraping, injection, debugger attachment, credential interception, or changing the Codex App's ownership/communication behavior.

If the only remaining path is an undocumented/private IPC protocol, document its existence and evidence without implementing a brittle interceptor unless a separate architecture decision explicitly approves that experiment.

## Completion report

Create `docs/research/phase-0b-process-correlation-result.md` containing:

- Windows version,
- Codex Desktop/CLI version,
- probe build/tool versions,
- observed process tree,
- controlled experiment procedure,
- raw/condensed observations,
- evidence classification,
- false-positive/ambiguity cases,
- observer performance cost,
- primary verdict,
- exact claim ceiling,
- recommended next architecture step.
