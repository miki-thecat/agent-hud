# Phase 1B result — `task_complete` and human readiness

Date: 2026-08-21
Branch: `spike/windows-codex-observation`

## Verdict

Primary verdict: `SUPPORTED_FOR_RECORDED_ROOT_TURN_READINESS`.

In the controlled local Codex Desktop task below, each normal turn ended with
`event_msg.task_complete` for its `turn_id`. After the first completion, a
second ordinary instruction was accepted in the same task and produced a
newer `task_started`; after that completion, a harmless command/tool turn was
accepted and produced a third `task_started`. The supported app task control
reported the latter two turns `completed` and the thread `idle` at the same
boundaries.

For the product question — *can the human send the next instruction now?* — a
persisted `task_complete` for the latest correctly identified root/user turn
is sufficient to transition that rollout-derived session to `READY`. This is
not a claim that the broader task succeeded or that its implementation is
correct.

The signal is `AUTHORITATIVE_FOR_RECORDED_TURN_LIFECYCLE`: it authoritatively
records a turn boundary in the rollout, but is not a documented general-purpose
passive Desktop live-status API. It cannot identify approval/user-input waits
or prove Desktop ownership outside the correlated task. Preserve `UNKNOWN`
when identity, ordering, or root/user-turn scope is ambiguous.

## Environment and safety

- Windows API version previously recorded for this host: `10.0.26200.0`.
- Current local rollout metadata reported `cli_version=0.148.0-alpha.15`.
- Test thread: `01a0238b-b1df-7dc1-a171-9b151ac38111` (disposable local
  Codex Desktop task).
- Rollout: `%USERPROFILE%\.codex\sessions\2026\08\21\rollout-2026-08-21T17-59-12-01a0238b-b1df-7dc1-a171-9b151ac38111.jsonl`.
- The watcher only read that JSONL file. It began before the second turn and
  sampled for appended records every 50 ms; no rollout, index, or state DB
  file was modified.
- The test used ordinary safe prompts and one `Get-Date -Format o` command.
  It did not touch repository files, use app-server attachment, acknowledge
  approval/user-input requests, inject into a process, scrape UI/memory, or
  execute a destructive command.

## Procedure and observations

The prompts are omitted apart from their harmless expected markers. IDs and
lifecycle metadata are retained; message and command output content are not.

| Experiment | Turn ID | Ordered decisive records | Desktop/task result |
| --- | --- | --- | --- |
| A — normal | `01a0238b-b3c3-70a3-8700-3e05304d53e8` | 2 `task_started` (08:59:13) → 11 `AgentMessage` (08:59:16) → 14 `task_complete` (08:59:16) | Normal final marker returned. |
| B — consecutive normal | `01a0238c-9099-7691-9a2a-ea0b43820454` | 16 `task_started` (09:00:09.934) → 20 `AgentMessage` (09:00:11.788) → 23 `task_complete` (09:00:11.864) | The second instruction was accepted after A; task control reported `completed`, thread `idle`. |
| C — command/tool | `01a0238c-d9f8-7ef2-a640-de4758c0cd50` | 25 `task_started` (09:00:28.699) → 32 `CommandExecution` completed (09:00:34.373) → 35 `AgentMessage` (09:00:36.597) → 38 `task_complete` (09:00:36.872) | The third instruction was accepted after B; task control reported `completed`, thread `idle`. |

The first task was created before its rollout was known, so its lifecycle was
read immediately after completion. The passive watcher covered B and C from
their starts through their terminal events.

### Ordering, supersession, and state sequence

The three turn IDs are distinct and source ordinals strictly increase. In
particular, ordinal 16 `task_started` for B follows A's ordinal 14
`task_complete`, and ordinal 25 `task_started` for C follows B's ordinal 23
`task_complete`. Therefore an earlier completion cannot remain `READY` once a
newer root/user-turn start is recorded.

```text
UNKNOWN -> WORKING (A start) -> READY (A complete)
        -> WORKING (B start) -> READY (B complete)
        -> WORKING (C start) -> READY (C complete)
```

The completed command item at ordinal 32 did **not** end C: an additional
agent message and `task_complete` followed about 2.499 seconds later. A
command/item completion must not transition the session to `READY`.

## Visibility timing

These are record-timestamp-to-watcher-observation delays, not a measurement of
the unobservable internal Desktop event-to-flush delay.

| Record | Persisted timestamp | First watcher observation | Observed delay |
| --- | --- | --- | ---: |
| B `task_started` | 09:00:09.934Z | 09:00:10.0183731Z | 84 ms |
| B `task_complete` | 09:00:11.864Z | 09:00:11.9129919Z | 49 ms |
| C `CommandExecution` complete | 09:00:34.373Z | 09:00:34.4600479Z | 87 ms |
| C `task_complete` | 09:00:36.872Z | 09:00:36.9713077Z | 99 ms |

The 50 ms watcher cadence bounds the next read after an append; scheduling and
record timestamp granularity account for the observed 49–99 ms range. The
upstream Desktop-to-file flush latency remains `NOT_MEASURED`.

## Edge cases and remaining uncertainty

- `task_complete` followed normal agent messages in this sample. A completion
  with `last_agent_message = null` was not exercised; it remains valid as a
  *recorded turn boundary* if it has the same root/user identity and ordering,
  but that exact variant is `NOT_VERIFIED`.
- The tool turn confirms that item/command completion is non-terminal. It does
  not establish semantic command success as session success.
- Stale completion rejection is supported by the observed newer
  `task_started` ordering; a reducer must retain turn ID and ordinal.
- Subagent records were not mixed into this test. Do not apply this mapping to
  subagent lifecycle records without an explicit root/user-thread mapping.
- The watcher was not restarted during a completed rollout; reconstruction on
  restart should choose the last lifecycle record for the latest root/user
  turn, but this is `NOT_VERIFIED` live.
- Abnormal termination is `NOT_VERIFIED`. No supported normal cancel/stop
  control was exposed in this task environment, and handoff/process stopping
  would not be a normal Desktop cancel experiment.
- Approval and user-input waiting remain `UNAVAILABLE` from rollout JSONL.
  This result must not be used to manufacture those attention states.

## Required state-model changes

Update the rollout-derived reducer for a deterministically correlated
root/user thread:

```text
latest root/user lifecycle event

task_started                    -> WORKING
task_complete                   -> READY
explicit terminal turn error    -> ERROR
turn_aborted                    -> UNKNOWN (until normal Desktop cancellation is tested)
no trustworthy lifecycle event  -> UNKNOWN
```

Only `task_started`, `task_complete`, and an explicit terminal error may
change this state. Item completion, assistant-message completion, command
completion, file changes, file mtime, process activity, and silence cannot.
Use event ordinal and `turn_id`; a newer `task_started` supersedes an older
completion immediately.

## Final assessment

The controlled evidence supports `task_complete => human can issue the next
instruction` for the latest, correlated root/user rollout turn. It does not
turn rollout tailing into a supported general live Desktop status endpoint and
does not erase the Phase 0 requirement to fail closed for unknown session
identity, approval/user-input state, or unsupported observation paths.
