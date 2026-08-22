# Issue 34 — file-overlap and conflict-risk feasibility

Date: 2026-08-22  
Scope: research only; no runtime conflict detection is implemented by this
report.

## Executive conclusion

`agent-hud` can eventually report **potential path overlap** as a read-only
warning, but the current Windows evidence does not support claiming file
ownership, locks, authorship, or a guaranteed merge conflict.

The smallest defensible future feature is a derived, explicitly labelled
`potential overlap` indicator built from two independent facts:

1. a validated root/user Codex identity;
2. a recent rollout `FileChange` record whose paths overlap another session's
   recent `FileChange` paths in the same canonical worktree.

That result would be `STRONG_CORRELATION` for a recorded overlap and
`HEURISTIC` for current conflict risk. It must never be rendered as “agent A
owns file X”, “file X is locked”, or “these tasks will conflict”. A future
implementation should remain dormant until the existing session/result/file
metadata foundations are stable and a safe observation path is available.

## Evidence reviewed

This report uses the current repository's committed observations and the
current primary protocol/documentation sources available on 2026-08-22:

| Evidence | Observed fact | Boundary |
| --- | --- | --- |
| [Phase 0C rollout observation](phase-0c-rollout-observation-result.md) | Recent rollouts contain `FileChange` item classifications alongside commands, messages, and reasoning. | A recorded item describes persisted history; it does not prove that the edit is still active, accepted, or owned by the session. |
| [Phase 1C session discovery](phase-1c-session-discovery-result.md) | `threads.id -> rollout_path -> session_meta` identity was validated for root/user rows; subagent rows are distinguishable and excluded from the root catalog. | Persisted identity is not proof of an open Desktop chat, live execution, or process ownership. |
| [Phase 1D rollout watcher](phase-1d-live-rollout-watcher-result.md) | Rollouts can be read incrementally and reconstructed after restart with bounded reconciliation. | File growth/flush is not a supported live-status or ownership contract. |
| [Readiness state model](../design/readiness-state-model.md) | File-change items are `STRONG_CORRELATION`; filesystem timestamps and process activity are `HEURISTIC`; unsupported/private IPC is `UNAVAILABLE`. | The same conservative classifications apply to conflict-risk evidence. |
| [Current Codex app-server README](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md) | `fileChange` carries `changes` with `path`, `kind`, and `diff`, plus `inProgress`, `completed`, `failed`, or `declined` status. `thread/read` and item-history reads are read-oriented APIs; live status is exposed by the app-server contract. | This is authoritative only when obtained from the authoritative server through a proven passive observation path. The repository's native-Windows evidence has not established that path for Desktop-owned sessions. |
| [Git worktree documentation](https://git-scm.com/docs/git-worktree.html) and [Git status documentation](https://git-scm.com/docs/git-status.html) | Git can enumerate registered worktrees and report tracked/untracked/index/worktree differences. | Git has no general field assigning a path change to a Codex session or proving who is currently writing it. |
| [Windows USN change journal documentation](https://learn.microsoft.com/en-us/windows/win32/fileio/change-journal-operations) | NTFS can expose filesystem change records and reasons. | A filesystem event has no Codex session identity and can be caused by builds, editors, generators, sync tools, or other processes. |

The official Codex source documents the shape of a useful future source, but
the existing [native-Windows observation findings](windows-codex-observation-2026-08-20.md)
and [local Phase-0 result](phase-0-local-result.md) keep Desktop-owned live
attachment and ownership at `UNAVAILABLE`. This report therefore does not
promote the protocol's theoretical capability into a current product claim.

## Candidate evidence sources

### 1. Rollout `FileChange` items

Best available session-associated evidence in the current installation.
After validating the root/user identity chain, a `FileChange` record can
associate paths, change kinds, and a recorded status with a Codex session and
turn. Its path list is useful for overlap analysis, and a `completed` record is
stronger than an `inProgress` record for what was persisted.

Classification:

- `AUTHORITATIVE` only for the narrow fact “this validated rollout recorded
  this FileChange payload at this sequence/turn”;
- `STRONG_CORRELATION` for “this session recently proposed or recorded an edit
  to this path”;
- not authoritative for current ownership, current bytes, lock state,
  successful application, or future merge behavior.

Important limits are stale history, incomplete/truncated rollouts, duplicate
or superseded patches, generated files, and the possibility that the same
path is changed outside Codex after the record was written. A diff in a
rollout is evidence of a proposed/recorded change, not a durable lease.

### 2. Git status and worktree state

Git is the strongest source for the current repository's observable state:
worktree registration, branch/HEAD, index changes, worktree changes, and
untracked paths. It should be used to normalize a path to a worktree and to
avoid comparing paths from unrelated roots.

Classification:

- `AUTHORITATIVE` for the queried Git state at a specific instant;
- `STRONG_CORRELATION` for the workspace context associated with a session's
  validated `cwd` when the path resolves unambiguously to a registered
  worktree;
- `UNAVAILABLE` for Codex authorship or active writer identity.

`git diff` answers what differs from a chosen tree; it does not answer which
agent made the change. Uncommitted edits may predate the session, be produced
by a human, or be shared by multiple sessions. A branch diff is similarly
useful for later integration review, but it is not an online lock/ownership
signal.

### 3. Branch and commit history

Commit metadata can establish that a path changed in a commit and can help a
future integration view explain divergence. It is too late and too indirect
for current conflict risk: many Codex edits are uncommitted, commit authorship
is not session identity, and two branches can change equivalent content at
different paths or with different intent.

Classification: `STRONG_CORRELATION` for historical integration context;
`UNAVAILABLE` for current ownership.

### 4. Filesystem activity

NTFS USN records or directory notifications can provide path/timing evidence
without reading file contents. This could detect that something changed after
a rollout record, or invalidate a cached observation.

Classification: `HEURISTIC` for session conflict risk. It has no reliable
Codex session mapping and cannot distinguish a Codex write from an editor,
formatter, compiler, test, generator, cloud sync client, or antivirus scan.
Process correlation would remain heuristic as well; the existing Phase-0B
evidence found no deterministic Thread/turn/item identifier in process
metadata.

### 5. Supported Codex metadata or live app-server status

If a future supported Windows passive endpoint exposes a validated session's
current item set and `FileChange` lifecycle, it would be the preferred source.
Its authority would be limited to the exact protocol contract and freshness
guarantees proven by a coexistence spike. It still would not automatically
prove OS-level file locks or exclusive ownership.

Classification today: `UNAVAILABLE` for Desktop-owned live observation on the
validated native-Windows topology; potentially `AUTHORITATIVE` for current
session/item facts if a safe supported endpoint is later proven.

## Evidence ceiling

The following table defines the vocabulary this product should use:

| Class | Safe statement | Unsafe statement |
| --- | --- | --- |
| `AUTHORITATIVE` | “The supported source recorded/returned this exact fact for session S, path P, at sequence/time T.” | “S owns P” unless the source explicitly defines ownership/lease semantics. |
| `STRONG_CORRELATION` | “S recently recorded a file change involving P” or “S is associated with worktree W.” | “S is currently writing P” or “S caused the current uncommitted bytes.” |
| `HEURISTIC` | “P changed recently” or “these sessions may touch the same path.” | “The agents are in conflict” or “one agent is blocking the other.” |
| `UNAVAILABLE` | “No trustworthy attribution/current signal is available.” | Filling the gap with timestamps, process names, file locks, or silence. |

The report should preserve source, sequence, observed time, freshness, path
normalization, worktree identity, and whether the fact is persisted or live.
Do not collapse confidence into a numeric score: many weak signals must not
outvote a missing identity or authority boundary.

## Proposed read-only conflict-risk model

The future reducer should emit an evidence record, not an ownership record:

```text
FileEvidence {
  session_id
  turn_id?
  worktree_root?
  normalized_path
  change_kind
  source                 // rollout_file_change, git, filesystem, live_protocol
  source_sequence?
  source_event_time?
  observed_at
  freshness
  confidence
  persisted_or_live
}
```

For each pair of validated root/user sessions:

1. Resolve each session `cwd` to a canonical registered worktree. If either
   side is ambiguous, do not compare it as a same-worktree conflict.
2. Normalize paths case-insensitively on Windows, resolve separators, and
   reject paths that escape the worktree. Preserve the original path for
   explanation; never silently equate an unresolved path.
3. Intersect only recent `FileChange` path sets from the same worktree. Keep
   `source_sequence` and turn identity so stale records can be discarded.
4. Classify the result as **potential overlap**, with evidence details and an
   expiry/freshness reason. Do not call it a conflict unless a future source
   supplies a conflict definition stronger than path intersection.
5. Treat Git and filesystem observations as corroboration or invalidation,
   never as attribution. A current `git status` change can show that the
   path is dirty; it cannot assign that dirtiness to either session.

Suggested result vocabulary:

| Result | Minimum evidence | Meaning |
| --- | --- | --- |
| `NO_EVIDENCE` | No valid recent path evidence | No overlap claim. |
| `POTENTIAL_OVERLAP` | Two sessions, same worktree, intersecting recent validated `FileChange` paths | Recorded path overlap; may be benign/sequential. |
| `STALE_EVIDENCE` | Historical overlap without a defensible freshness boundary | Keep for history/diagnostics only; do not show as current. |
| `UNAVAILABLE` | Missing identity, ambiguous worktree, unsupported live source, or malformed path | Fail closed. |

The initial UI, if later accepted, should show the affected path count and
the evidence age/source on demand. It should not block, pause, steer, lock, or
coordinate either session.

## Required failure-mode handling

- **Same file touched sequentially:** retain turn/sequence and freshness; do
  not report a simultaneous conflict merely because history overlaps.
- **Generated files:** classify build outputs, caches, coverage, and other
  configured/generated paths separately or exclude them by an explicit policy.
  A generated-file collision is not evidence that the source tasks overlap.
- **Tests/build artifacts:** treat writes as filesystem/build evidence, not
  Codex authorship. They may update files after a `FileChange` record.
- **Stale FileChange history:** expire it or mark it `STALE_EVIDENCE`; never
  use rollout recency alone as proof of current activity.
- **Multiple worktrees:** compare absolute canonical roots first. Same
  relative path in different worktrees is not a current filesystem collision;
  it may be a later merge/integration concern.
- **Uncommitted edits:** report only that Git sees a dirty path. Do not claim
  which session made it, and do not overwrite, stash, stage, or lock it.
- **Renames/deletes:** preserve `kind` and pair rename endpoints only when the
  source provides both sides; otherwise report the paths independently.
- **Malformed or ambiguous paths:** omit the overlap claim and retain an
  `UNAVAILABLE` diagnostic rather than guessing through string similarity.
- **Codex restart/disconnect:** preserve historical evidence as historical;
  invalidate any current-risk claim whose freshness/ownership contract was
  lost.

## Recommendation and smallest future implementation

Feasibility is **conditional**. A useful, conservative path-overlap warning is
justified once the existing foundations are stable, but runtime conflict
detection is not justified by current evidence.

The smallest future slice should be:

1. extend the rollout adapter with a typed, identity-validated `FileChange`
   record that preserves path/kind/status/turn/ordinal and marks it persisted;
2. add pure path normalization and same-worktree intersection code with
   deterministic fixtures for the failure modes above;
3. add a read-only Git context lookup only for worktree-root resolution and
   dirty-state explanation; do not use it for attribution;
4. expose a diagnostic/result model first, with explicit `POTENTIAL_OVERLAP`,
   `STALE_EVIDENCE`, and `UNAVAILABLE` outcomes;
5. run a gated live validation only after a supported passive Windows Codex
   source is proven. Keep filesystem watchers/USN integration out of the MVP
   unless measurement shows they solve a real freshness gap.

Do not add locks, process injection, private IPC, automatic coordination, or a
background high-frequency scanner. The first implementation should be
restartable, bounded, read-only, and honest about the distinction between
recorded edit overlap and actual conflict.

## Decision

Proceed with documentation and typed foundations only when their separate
tasks are accepted. Defer runtime conflict-risk detection until:

- session-to-rollout identity remains validated;
- the adapter can parse `FileChange` paths without losing sequence/status;
- worktree resolution is deterministic;
- freshness/expiry behavior has fixture coverage; and
- a supported passive Windows observation path has been proven or the feature
  is explicitly scoped to a historical/offline view.

This issue's research pass is complete without modifying `src/` or changing
readiness behavior.
