# Phase 0C result — Codex rollout JSONL observation

Date: 2026-08-21  
Branch: `spike/windows-codex-observation`

## Verdict

Primary verdict: `BLOCKED_SUPPORTED_LIVE_PATH`.

The current Windows Codex installation writes useful session history to
`%USERPROFILE%\.codex\sessions\**\rollout-*.jsonl`. The files are readable
without UI scraping or session ownership, and their records contain useful
messages, tool calls, command/file-change item classifications, and completion
events. The same files do not expose a supported, authoritative live status
for Codex Desktop threads. Persisted timestamps, file growth, and event kinds
must not be promoted to `WORKING`, `NEEDS_INPUT`, `APPROVAL`, or `DONE`.

This reinforces the Phase 0 and Phase 0B result: do not build the production
HUD around rollout tailing as a substitute for a supported Desktop observation
endpoint.

## Experiment environment

- OS API version: `Microsoft Windows NT 10.0.26200.0`.
- Codex Desktop/runtime evidence: existing Phase 0 process inspection found
  Codex Desktop package `26.814.5517.0` and CLI `0.144.3`; the current rollout
  metadata observed during this task reports `cli_version=0.148.0-alpha.15`.
  The `codex --version` command itself was not runnable in this restricted
  probe context because Windows returned Access Denied.
- Rollout root: `%USERPROFILE%\.codex\sessions`.
- State DB: `%USERPROFILE%\.codex\state_5.sqlite`.
- Session index: `%USERPROFILE%\.codex\session_index.jsonl`.
- The state DB was opened through SQLite read-only URI mode. Rollout files,
  index, and metadata were only read.

The local state DB contained 141 thread rows at observation time, including 49
`user` and 60 `subagent` rows according to `thread_source` (the remaining rows
use other/empty source values). Ten rows had recent update timestamps. This
shows that multiple concurrent histories exist locally; it does not prove
that all are live Desktop sessions.

## Observation method

1. Enumerated `sessions\**\rollout-*.jsonl` and selected recent files by
   filesystem last-write time.
2. Parsed only JSON structure and non-content identifiers. Prompt/message
   bodies, tokens, and private text were not copied into this report.
3. Compared `session_meta.payload.session_id`, `session_meta.payload.id`,
   rollout filename UUIDs, `session_index.jsonl`, and `state_5.sqlite.threads`.
4. Counted record types and item/event subtypes in recent rollouts.
5. Queried the state DB schema and recent rows in read-only mode.
6. Tail-tested the newest rollout for 12 seconds with a 250 ms file-size
   observation interval. No append occurred during that idle window.

The requested controlled creation of multiple Desktop sessions and execution
of distinct operations was not performed. Creating, steering, answering, or
resuming Desktop sessions would violate the task's no-operation constraint.
Existing naturally-created user, subagent, and tool activity was used instead.

## Identity and persistence findings

### Rollout structure

Recent rollouts begin with a `session_meta` record whose payload includes:

- `session_id`
- `id`
- `timestamp`
- `cwd`
- `cli_version`
- source/thread-source and other session configuration

The common user-thread case is directly useful: the `session_meta` ID,
`session_id`, state DB `threads.id`, and rollout filename UUID agree. The state
DB also stores an explicit `threads.rollout_path` mapping, which is stronger
than parsing a filename.

The mapping is not universally one-to-one at the JSON payload level. A recent
subagent rollout had a filename/state-row ID ending in `01a02023`, while its
`session_meta.payload.session_id` ended in `01a02022`. Therefore the filename
alone, or the first JSON record alone, is not a universal identity contract.
The DB row's `id` plus `rollout_path` is the strongest observed persisted
mapping, but it remains persisted metadata rather than live Desktop ownership.

`session_index.jsonl` currently exposes only `id`, `thread_name`, and
`updated_at`; it does not provide the rollout path or a live state field.

### State DB schema

The `threads` table contains persisted fields including `id`, `rollout_path`,
`created_at(_ms)`, `updated_at(_ms)`, `source`, `thread_source`, `cwd`, `title`,
`cli_version`, `has_user_event`, `archived`, `preview`, and token counters.
These are useful for restart reconstruction and discovery of persisted
records. No observed column establishes that a thread is currently running,
waiting for approval, or waiting for user input.

## Activity observations and classification

Recent JSONL records included these useful categories:

| Information | Classification | Evidence / limitation |
|---|---|---|
| JSONL record timestamp, ordinal, record type | `AUTHORITATIVE` | Authoritative for what was persisted in that rollout file. |
| `session_meta` IDs and configuration | `AUTHORITATIVE` | Authoritative session metadata for the file; not proof of Desktop live ownership. |
| State DB `threads.id` → `rollout_path` | `AUTHORITATIVE` | Strong persisted mapping in the observed installation. |
| `session_index` ID/name/update time | `AUTHORITATIVE` | Authoritative index metadata; no live state. |
| User/assistant message records | `AUTHORITATIVE` | Historical message/event records, subject to append timing and redaction policy. |
| Tool execution records | `AUTHORITATIVE` | `response_item.custom_tool_call` and `custom_tool_call_output` were observed. |
| Command/file-change activity labels | `STRONG_CORRELATION` | `event_msg.item_completed` included `CommandExecution` and `FileChange`; this describes recorded items, not a current state. |
| Completion/agent-message/user-message events | `STRONG_CORRELATION` | Useful for reconstructing history and completed turns. |
| Current assistant activity | `HEURISTIC` | A recently appended item suggests recent activity only; no safe current-state boundary was found. |
| `WORKING` | `UNAVAILABLE` | File mtime, append activity, or a live process cannot prove it. |
| `waitingOnUserInput` | `UNAVAILABLE` | No authoritative rollout field or safe live observation was found. |
| `waitingOnApproval` | `UNAVAILABLE` | No authoritative rollout field or safe live observation was found. |
| `DONE` / idle | `UNAVAILABLE` | `task_complete` is historical event evidence, not a durable current-state contract. |
| Thread ↔ Desktop process/session ownership | `UNAVAILABLE` | Rollout metadata does not prove which Desktop client currently owns the thread. |
| File change details | `STRONG_CORRELATION` | Recorded `FileChange` item types are useful historical evidence; content/path policy still applies. |

The sampled recent rollouts also contained `Reasoning`, `AgentMessage`,
`UserMessage`, `CommandExecution`, and `FileChange` item classifications. This
proves that JSONL can carry meaningful activity history, but not that every
Codex Desktop operation or actionable request is represented in a safe,
stable, externally consumable form.

## Tail and latency measurements

The newest rollout was observed with a read-only size check every 250 ms for
12 seconds. It remained unchanged for the full window. This establishes that
tailing an idle, completed file is cheap and non-invasive; it does not measure
event-to-file flush latency.

As a secondary historical measurement, consecutive JSON timestamps in recent
files showed sub-second event spacing in active sections, with large gaps when
the session was inactive or between turns. Examples from the newest sampled
files were median gaps of approximately 0.227 s, 0.066 s, 0.019 s, and 0.964 s;
maximum gaps ranged from approximately 3.6 s to 133 s. These are event
inter-arrival gaps, not flush latency.

No safe paired experiment was available that supplied an external event time
and measured the moment it became visible in a Desktop-owned rollout. Exact
flush latency is therefore `UNAVAILABLE`. A 250 ms filesystem poll would add
up to roughly 250 ms observation delay if an append were already flushed, but
the underlying flush delay remains unknown.

## Multiple-thread feasibility

If known paths are supplied, independent JSONL files can technically be tailed
concurrently with bounded file watchers or low-rate reconciliation. This is a
filesystem capability, not proof of a supported Codex observation topology.
Discovery is incomplete in `session_index.jsonl`, identity has subagent edge
cases, and there is no authoritative live state. Consequently:

- persisted history monitoring: `STRONG_CORRELATION` / potentially useful;
- authoritative per-Desktop-thread live HUD: `UNAVAILABLE`;
- production polling/tailing adapter: not justified until the upstream
  ownership and flush contract is documented and verified.

## Agent-HUD decision

Do not use rollout tailing to drive MVP status rows or actionable attention
indicators. It may be retained as a future, explicitly historical/degraded
diagnostic source for:

- persisted session discovery;
- last recorded message/activity;
- restart reconstruction;
- offline diagnostics, with content-minimal logging.

Such a source must never label a row `WORKING`, `APPROVAL`, `NEEDS_INPUT`, or
`DONE` unless a separate supported live source supplies that fact. The adapter
should fail closed when only rollout files are available.

## Side effects and unverified items

- No Codex Desktop UI was scraped.
- No process injection or memory scraping was used.
- No Desktop session was created, resumed, steered, interrupted, approved, or
  answered.
- No rollout, index, or state DB file was modified.
- No duplicate actionable request, lock conflict, crash, or Desktop degradation
  was observed.
- A controlled active/idle/approval/user-input comparison across Desktop
  sessions remains unverified by design.
- Exact rollout flush timing and the semantics of any private IPC endpoint
  remain unverified.

## Next design decision

Keep the Codex adapter replaceable and fail closed. The next useful spike must
target a documented, passive Windows Desktop/app-server observation endpoint
or an official exported status surface. Do not compensate for its absence with
filesystem timestamps, process activity, UI scraping, or inferred state.
