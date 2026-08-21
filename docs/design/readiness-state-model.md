# Readiness State Model

## Purpose and scope

This document defines the smallest state model that the HUD may display for a
Codex session. It answers one product question:

> Can a human give this session its next instruction now?

The model intentionally does not reproduce Codex's internal lifecycle. It is a
four-value product model:

```text
WORKING | READY | ERROR | UNKNOWN
```

The model is read-only. It must not send turns, answer approval or user-input
requests, resume or interrupt threads, attach through process injection, or
scrape process memory. If the observer cannot establish a state without
interfering with Codex Desktop, the result is `UNKNOWN`.

This is a reducer contract, not a claim that the current native-Windows
installation can provide every input. Phase 1B controlled evidence establishes
one narrow rollout-derived input: for a deterministically correlated root/user
turn, the latest `task_started` / `task_complete` lifecycle record establishes
the human-readiness boundary. It does not establish a general live Desktop
status, approval/user-input status, or ownership contract; the adapter must
still fail closed outside that narrow evidence boundary.

## State definitions

### `WORKING`

The latest correctly identified root/user turn has a `task_started` lifecycle
record and no later terminal lifecycle record. A recent timestamp, an open
process, a growing rollout file, or an item completion is not sufficient.

### `READY`

The session's current work has reached a trustworthy completion boundary and no
known approval, user-input, or still-active turn is pending. The state means
that the human can reasonably issue the next instruction; it does not mean that
the task was successful, correct, or complete in the user's broader sense.

An assistant message or tool result alone does not prove `READY`. Phase 1B
established that `task_complete` does establish it when it is the latest
lifecycle record for a correctly correlated root/user turn; use turn identity
and source ordinal to reject stale completion. This remains a recorded-turn
boundary, not proof of task success or a general Desktop live-status contract.

### `ERROR`

A clear failure is reported by an authoritative source, such as an explicit
terminal error, failed turn, or unrecoverable observation failure associated
with the session. Ordinary command failure inside an otherwise continuing
turn is not automatically a session-level `ERROR`; it becomes one only when
the source says the session/turn has failed or cannot continue.

### `UNKNOWN`

The available evidence is missing, stale, contradictory, historical-only, or
too weak to distinguish the other states. `UNKNOWN` is the safe default at
discovery, after an unsafe/disconnected observation path, and whenever a
transition cannot be justified. It is preferable to a false `WORKING` or
`READY` indication.

## Input signal classification

The classification below is for this product's semantic readiness decision.
Some signals are authoritative about a recorded fact while still being
insufficient to establish the current readiness state.

| Signal | Classification | What it can establish | What it cannot establish by itself |
|---|---|---|---|
| Official live thread/turn status from the Desktop-owned server, with a proven passive observation contract | `AUTHORITATIVE` | Current active, terminal, waiting, or error state as defined by that contract | Nothing beyond the contract's documented scope |
| Explicit terminal error/failure from that live source | `AUTHORITATIVE` | `ERROR` for the associated session/turn | Whether a later turn has already superseded it, unless sequence/identity is also checked |
| Rollout JSONL root/user `task_started` / `task_complete`, turn ID, ordinal, and timestamp | `AUTHORITATIVE_FOR_RECORDED_TURN_LIFECYCLE` | The latest correctly correlated root/user turn's recorded `WORKING` / `READY` boundary | General Desktop ownership, approval/user-input state, task correctness, or an externally documented live-status contract |
| Assistant message completion | `STRONG_CORRELATION` | A response/message reached a recorded completion boundary | That no tool, approval, user-input request, or newer turn remains; therefore not `READY` alone |
| Tool execution start/output | `STRONG_CORRELATION` | Work is/was being performed; an output was recorded | Current activity after the record, terminal readiness, or session success |
| Command execution start/completion | `STRONG_CORRELATION` | A command item was recorded as active or completed | That the overall Codex turn is finished or ready for another instruction |
| File-change item | `STRONG_CORRELATION` | A file-change operation was recorded | That implementation is complete, accepted, or currently active |
| Explicit current approval/user-input request from the authoritative live source | `AUTHORITATIVE` when safely observable | The session is not ready for an ordinary next instruction; in this four-state model it prevents `READY` | It is not itself one of the four display states; do not silently add a fifth state |
| Rollout file growth, filesystem mtime, or recent event timestamp | `HEURISTIC` | Recent persisted activity may have occurred | Live processing, completion, idleness, or human readiness |
| Process existence, CPU, I/O, handles, child processes, or process creation time | `HEURISTIC` | OS-level process/resource activity | Codex session identity, current turn state, or readiness; Desktop multiplexes threads |
| Timestamp proximity between events/processes/files | `HEURISTIC` | Possible temporal correlation | Causality or Thread-to-process ownership |
| Codex Desktop UI text, screenshots, or window scraping | `UNAVAILABLE` | Nothing accepted for the MVP | A stable, supported, non-invasive protocol fact |
| Process injection, debugger/memory inspection, or memory scraping | `UNAVAILABLE` | Nothing accepted; prohibited | Any production state |
| Private/undocumented IPC endpoint or named pipe whose observer semantics are unproven | `UNAVAILABLE` | Endpoint existence at most | Safe passive status or ownership |
| Persisted state/index database liveness field | `UNAVAILABLE` in the current installation | Historical metadata and identity may be available | Authoritative current `WORKING`, `READY`, approval, or user-input state |

`AUTHORITATIVE` always means authoritative for the exact fact and scope
documented by the source. It does not allow the reducer to promote a
historical fact into a current state without freshness, session identity, and
ordering checks.

## Decision and transition rules

The reducer should process observations in source order and retain the source
sequence/identity needed to reject stale records. The following rules are
mandatory:

1. Initialize a discovered session as `UNKNOWN`.
2. Set `WORKING` when the latest deterministically correlated root/user
   lifecycle record is `task_started`. A strong-correlation activity/item
   record alone must not do this.
3. Set `READY` when the latest deterministically correlated root/user
   lifecycle record is `task_complete`. An assistant completion, command
   completion, file change, timestamp gap, or process exit alone must not do
   this. A newer `task_started` supersedes the prior completion immediately.
4. Set `ERROR` only on an authoritative session/turn failure. Do not convert a
   single failed shell command into session `ERROR` unless the source defines
   it as terminal.
5. If an authoritative approval/user-input request is observed, block any
   `READY` transition. Until the four-state contract gains an explicit
   waiting state, represent the session as `UNKNOWN` unless the source also
   provides a supported mapping to `WORKING` or `ERROR`. The UI may expose a
   separate attention reason later, but it is outside this four-state model.
6. On disconnect, restart, unsupported protocol, identity ambiguity, stale
   evidence, or contradictory observations, transition to `UNKNOWN` unless a
   still-valid authoritative terminal/error fact remains defensible.
7. A newer authoritative observation supersedes an older state. A heuristic
   signal may enrich activity text or diagnostics but may not override a state
   established by authoritative evidence.
8. Never use absence of new events as proof of `READY`. Silence is evidence of
   nothing unless the source explicitly defines an idle/terminal heartbeat or
   status.

Conceptually, the safe transition graph is:

```text
          latest root/user task_started
        +--------------------------+
        |                          v
  UNKNOWN ---------------------> WORKING
    ^  ^                           |  |
    |  | loss/ambiguity            |  | latest root/user task_complete
    |  +---------------------------+  v
    |                              READY
    |                                |
    |                                | newer active
    |                                +------> WORKING
    |
    +--------- authoritative failure <------ WORKING / READY
                                              |
                                              v
                                            ERROR
```

The graph is intentionally conservative: every state can return to `UNKNOWN`
when its evidence becomes stale or the observation contract is lost. An
`ERROR` is not cleared merely because a process remains alive; a new
authoritative turn/status must establish the replacement state.

## Confidence model

Confidence is evidence metadata for the reducer and diagnostics, not an extra
user-facing state. Use these evidence classes:

| Confidence | Meaning | Allowed state effect |
|---|---|---|
| `AUTHORITATIVE_FOR_RECORDED_TURN_LIFECYCLE` | Correctly identified latest root/user lifecycle fact in a rollout | May establish `WORKING` / `READY` only within the validated recorded-turn contract |
| `AUTHORITATIVE` | Current, correctly identified fact from a supported passive source | May establish or replace any of the four states within its contract |
| `STRONG_CORRELATION` | Closely associated recorded event, but missing current-state or ownership guarantees | May support explanation/history; may not establish `READY`, `WORKING`, or `ERROR` alone |
| `HEURISTIC` | Temporal, process, filesystem, or inferred association | Diagnostics/activity hints only; never state-changing |
| `UNAVAILABLE` | Prohibited, absent, unsafe, or unverified source | No state effect; preserve `UNKNOWN` or last defensible authoritative state |

For each candidate transition, the reducer should retain at least:

- session/thread identity and source;
- source sequence or monotonic ordering where available;
- observed-at time and source event time;
- whether the fact is live or persisted history;
- confidence/classification;
- freshness/invalidity reason.

Do not collapse these fields into a numeric score. A high number of weak
signals cannot outweigh one missing authority boundary. If required identity,
freshness, or no-pending-work evidence is absent, the result is `UNKNOWN`.

## False-positive cases

The reducer and UI must be tested against at least these cases:

- A rollout file was recently modified, but Codex is paused for approval or
  user input: do not show `WORKING` or `READY` from mtime alone.
- An assistant message was recorded, but a tool call or later turn is still
  active: do not show `READY`.
- A command completed successfully, but the assistant is still composing or
  another item is pending: do not show `READY`.
- A command failed, but Codex reports that it will continue: do not show
  session `ERROR`.
- A file-change event was recorded, but the implementation is incomplete or
  awaiting review: do not show `READY`.
- The shared Desktop `codex.exe` process has high CPU or child shells, but the
  activity belongs to another session: do not map process activity to this
  session's `WORKING` state.
- The process is idle or has exited after writing a rollout: do not show
  `READY`; the state is `UNKNOWN` unless an authoritative terminal fact exists.
- A timestamp gap is long: do not interpret silence as completion.
- A stale completion arrives after a newer active event: reject it by source
  sequence/identity and keep `WORKING` or `UNKNOWN`.
- A separate stdio app-server can list its own threads: do not treat them as
  Desktop-owned live sessions.
- A private named pipe exists: do not treat its name or successful connection
  as proof of safe observation semantics.

## Future extension points

The four-state model should remain stable at the UI boundary even if the
adapter later gains richer facts. Candidate extensions include:

- a separate attention reason (`APPROVAL`, `USER_INPUT`) while retaining a
  four-state readiness value;
- a `DISCONNECTED`/`STALE` diagnostic condition that is not confused with
  `ERROR`;
- source provenance and freshness displayed on demand rather than in the main
  glanceable row;
- explicit turn IDs, item IDs, sequence numbers, and pending-request sets from
  an officially supported Windows passive endpoint;
- a verified event-driven adapter with bounded reconciliation after restart or
  missed events;
- deterministic fixture tests for every transition and stale-event rejection;
- a separately labelled historical/offline view for rollout-derived messages
  and activity, never presented as live readiness.

Any extension must preserve the core rules: read-only observation, no Desktop
interference, no prohibited OS inspection, evidence over inference, and
`UNKNOWN` when authority is insufficient.
