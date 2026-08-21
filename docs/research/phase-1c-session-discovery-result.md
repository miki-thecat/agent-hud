# Phase 1C result — persisted session discovery

Date: 2026-08-21
Branch: `spike/windows-session-discovery`

## Verdict

Primary verdict: `SUPPORTED_FOR_BOUNDED_PERSISTED_ROOT_SESSION_DISCOVERY`.

The current local Codex state can reconstruct a bounded set of persisted
root/user sessions without reading UI state or connecting to an app-server:

```text
state_5.sqlite threads row (thread_source = user)
    -> threads.id + threads.rollout_path
    -> rollout session_meta (id/session_id)
    -> latest root rollout lifecycle event
```

That chain is authoritative for the selected persisted root-session identity
in this installation, and it can reconstruct the narrow Phase 1B recorded
readiness result (`task_started -> WORKING`, `task_complete -> READY`) after
the observer starts or restarts.

It does **not** expose which chats are currently open in Codex App, currently
visible, or owned by a particular Desktop window. Those properties are
`UNAVAILABLE` from the inspected persisted sources. A HUD must therefore call
this a bounded **Recent local sessions** set, never an open-chat or live-session
list.

## Environment and safety

- OS API version previously recorded on this host: `Microsoft Windows NT
  10.0.26200.0`.
- Current rollout metadata reported `cli_version=0.148.0-alpha.15`.
- Read-only sources: `%USERPROFILE%\\.codex\\state_5.sqlite`,
  `%USERPROFILE%\\.codex\\session_index.jsonl`, and
  `%USERPROFILE%\\.codex\\sessions\\**\\rollout-*.jsonl`.
- The SQLite database was opened with `mode=ro`; JSONL/index files were read
  only. No Desktop UI, process memory, private IPC, or app-server connection
  was used.

The inspection deliberately retained identifiers and structural metadata, but
did not copy prompt, message, preview, or title content into this report.

## Controlled local sample

Three distinct, unarchived `thread_source=user` roots were selected from the
same local Codex task environment. They used distinguishable working
directories; the first was the task being observed, the latter two were
separate completed tasks. Their persisted identity chains were verified by a
fresh database read and JSONL parse.

| Sample | Root thread ID | DB rollout basename ends in root ID | `session_meta.id` / `session_id` | Latest lifecycle record | Recorded readiness |
|---|---|---|---|---|---|
| A — observed in-progress root | `01a02398-1612-7c71-9cb5-903a4f984754` | yes | both root ID | ordinal 2 `task_started`, turn `01a02398-1fa5-7a13-a1cf-31b1e8763562` | `WORKING` |
| B — separate completed root | `01a0238a-4e1b-75e0-b988-d66eb95b7483` | yes | both root ID | ordinal 231 `task_complete`, turn `01a0238a-556d-7841-8092-8cc75b353694` | `READY` |
| C — Phase 1B controlled root | `01a0238b-b1df-7dc1-a171-9b151ac38111` | yes | both root ID | ordinal 38 `task_complete`, turn `01a0238c-d9f8-7ef2-a640-de4758c0cd50` | `READY` |

All three DB rows had `source=vscode`, `thread_source=user`, `archived=0`, a
non-empty `cwd`, a non-empty title, and a readable `rollout_path`. Their
`updated_at_ms` and `recency_at_ms` values differed as expected across the
separate tasks. Each ID also appeared in `session_index.jsonl` at observation
time, but that index supplied only ID, thread name, and update time.

This is a controlled persisted-state comparison, not a proof of simultaneous
Desktop windows. Sample A overlapped the observation; B and C were consecutive
completed sessions. No test established a separate current-window identity.

## Root/user versus subagent filtering

`threads.thread_source` is the decisive observed persisted discriminator:

| Signal | Classification | Observed use / limit |
|---|---|---|
| `threads.id -> threads.rollout_path` | `AUTHORITATIVE_FOR_PERSISTED_IDENTITY` | Selects the persisted file for a DB thread row. |
| `threads.thread_source = user` | `AUTHORITATIVE_FOR_PERSISTED_IDENTITY` | Identifies the root/user rows used by this result. |
| `threads.thread_source = subagent` | `AUTHORITATIVE_FOR_PERSISTED_IDENTITY` | Exclude from the MVP root-session list. |
| Root-row rollout `session_meta.id` and `session_id` equal DB `threads.id` | `AUTHORITATIVE_FOR_PERSISTED_IDENTITY` | Verified for all three root samples. |
| `source`, `agent_role`, `agent_path` | `STRONG_CORRELATION` | Useful provenance/detail only; do not replace `thread_source`. |
| rollout filename UUID alone | `HEURISTIC` | Convenient cross-check, not the selection key. |

A recent subagent makes the exclusion necessary. Its DB/rollout ID was
`01a02398-80fc-71d2-b032-392ac5a1e0a0`, while its
`session_meta.session_id` was parent root
`01a02398-1612-7c71-9cb5-903a4f984754`; the subagent metadata declared
`thread_source=subagent` and source `subagent/guardian`. Therefore an observer
must not treat every rollout's `session_meta.session_id` or filename as an
independent HUD root row.

At observation, the DB contained 66 `user`, 71 `subagent`, and 32 null/other
`thread_source` rows. All observed rows had `archived=0`; archive filtering is
still required by policy because the field exists, but this sample did not
exercise an archived row.

## Discovery fields and their meaning

| Persisted field/source | Classification | Allowed use |
|---|---|---|
| `threads.id`, `rollout_path`, `thread_source` | `AUTHORITATIVE_FOR_PERSISTED_IDENTITY` | Root selection and root-to-rollout mapping. |
| rollout `session_meta.id`, `session_id`, `thread_source` | `AUTHORITATIVE_FOR_PERSISTED_IDENTITY` | Validate the selected root identity; reject mismatches/ambiguity. |
| `title` / session-index `thread_name` | `STRONG_CORRELATION` | Compact display label after content/privacy policy; neither establishes rootness or liveness. |
| `cwd` | `STRONG_CORRELATION` | Optional display/context or grouping hint; not a session-ownership key. |
| `source` | `STRONG_CORRELATION` | Provenance/diagnostics; the root samples all reported `vscode`. |
| `archived` | `AUTHORITATIVE_FOR_PERSISTED_IDENTITY` | Exclude archived rows from the default set. |
| `updated_at_ms`, `recency_at_ms`, index `updated_at` | `HEURISTIC` | Sort/bound a historical recent set only; never call it open, active, or working. |
| `session_index.jsonl` | `STRONG_CORRELATION` | Secondary name/index cross-check; it lacks rollout path, source, archive, and live-state fields. |
| currently open Desktop chat/window/session ownership | `UNAVAILABLE` | No inspected DB/index/rollout field or safe read path provides it. |

## Startup and restart reconstruction

Two independent read-only scans (`cold`, then `restart` one second later with a
new SQLite connection and fresh JSONL parse) produced the same latest lifecycle
tuple for every sample:

```text
A: (ordinal 2, task_started, 01a02398-1fa5-7a13-a1cf-31b1e8763562) -> WORKING
B: (ordinal 231, task_complete, 01a0238a-556d-7841-8092-8cc75b353694) -> READY
C: (ordinal 38, task_complete, 01a0238c-d9f8-7ef2-a640-de4758c0cd50) -> READY
```

This verifies restart reconstruction for two completed sessions (B/C) and one
observed in-progress recorded turn (A). It inherits the Phase 1B constraint:
these are recorded root-turn lifecycle states, not a general live status or
open-window claim. A reducer must scan lifecycle records in ordinal order and
use the final valid root record; a newer `task_started` supersedes an older
`task_complete`.

## Proposed MVP session-selection policy

Use the following explicitly historical/recent policy until a supported passive
Desktop ownership source exists:

1. Read `threads` read-only and keep only rows with `thread_source=user`,
   `archived=0`, a non-empty `id`, and a readable `rollout_path`.
2. Validate that the rollout's initial metadata identifies the same root. If
   `session_meta.id`/`session_id`, path, or source classification is ambiguous,
   omit the row rather than guessing.
3. Order candidates by persisted recency metadata and retain a fixed maximum
   of 20, matching the product's expected 3–20 concurrent-session range.
4. Label the view **Recent local sessions**, not “open chats”, “active
   sessions”, or “Desktop windows”. Exclude archived rows and do not paginate
   historical rows by default.
5. Reconstruct only the latest validated root lifecycle state. `task_started`
   may show recorded `WORKING`; `task_complete` may show recorded `READY` under
   the Phase 1B contract. Missing/mismatched state is `UNKNOWN`.

The cardinality bound prevents an unbounded historical list. Recency chooses
which persisted roots appear but does not establish that any one is currently
open or currently executing.

## Ambiguities and non-goals

- The persisted sources cannot distinguish a visible/open Codex App chat from
  a closed historical chat, so a recent set can include an old session and
  omit an older-but-open session.
- No archived root was present in this sample, so archive-write timing and
  restoration behavior remain untested.
- `session_index.jsonl` is incomplete as a primary discovery source because it
  omits rollout path, source, archive, and state fields.
- A subagent can reference its parent through `session_meta.session_id`; never
  collapse it into the root based on that relation alone.
- No exact concurrent multi-window experiment was run. The selected roots were
  overlapping/consecutive persisted tasks, which is sufficient for identity
  and restart reconstruction but not for window ownership.
- Approval/user-input status, live event flush latency, and a supported passive
  Desktop status API remain `UNAVAILABLE`.

## Decision

Persisted local state is sufficient for a bounded, root/user-only historical
discovery catalog and narrow lifecycle reconstruction. It is not sufficient to
ship a claim that the HUD shows all currently open Codex App chats. Keep the
adapter fail-closed outside the validated identity and recorded-turn boundary.
