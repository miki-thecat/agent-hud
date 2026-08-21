# Phase 1D result — event-driven live rollout watcher

Date: 2026-08-21
Branch: `feat/live-rollout-watcher`

## Design choice

The watcher uses the maintained `notify` crate's Windows backend. On Windows
this is backed by `ReadDirectoryChangesW`, so the application gets native
directory-change notifications without adding Tokio or a polling loop. A raw
`ReadDirectoryChangesW` wrapper would avoid the small cross-platform adapter
surface, but would duplicate lifecycle/error handling and unsafe buffer code
for no product benefit at this phase. The dependency is isolated to the
watcher boundary and can be replaced if the native UI later needs a thinner
platform shell.

The normal rollout path is incremental: each tracked JSONL file keeps its byte
offset, incomplete trailing-line buffer, latest root/user turn, and normalized
readiness. Filesystem notifications only read newly appended complete lines.
Database/WAL changes trigger the existing bounded read-only discovery snapshot
to admit or remove recent root/user sessions. Recovery reconciliation reopens
and rescans every bounded tracked rollout, compares reconstructed readiness,
and emits a `CHANGE` only for a semantic difference. Truncation, malformed
lifecycle records, identity mismatch, and watcher overflow are treated as
reconciliation or degraded observation conditions.

Notify errors and incremental read/parse errors enter that bounded recovery
path. If recovery cannot re-establish validated state, tracked non-UNKNOWN
readiness is changed to `UNKNOWN`; the watcher never continues presenting a
stale `READY` or `WORKING` state as trustworthy.

The CLI remains explicitly historical/recorded readiness. It prints session
IDs and `WORKING`, `READY`, or `UNKNOWN` only; it does not print prompts,
messages, commands, model deltas, or rollout content.

## Verification status

Deterministic tests cover lifecycle append, command/item no-op behavior, partial
JSONL lines, duplicate notifications, newer-turn supersession, truncation,
identity mismatch, missed append followed by recovery reconciliation, and
failed recovery degrading readiness to `UNKNOWN` (19 tests total).

## Controlled live validation

The release watcher was run against the real local `%USERPROFILE%\\.codex`
state while a disposable root/user Codex task performed harmless no-op turns.
The watcher remained alive across completed turns; no app-server attachment,
UI scraping, process inspection, or Codex-state write was used.

Disposable root: `01a023ce-711b-7000-820c-3870493dabc5`.
The bounded set contained 20 sessions. An independent tracked root remained
visible throughout (`01a023bd-8807-76b0-bd61-995b00200066`, `WORKING`), while
other bounded roots remained `READY`.

Final release-watcher output for the disposable root:

```text
CHANGE 01a023ce-711b-7000-820c-3870493dabc5 WORKING persisted_at=2026-08-21T10:14:24.185Z observed_at_unix_ms=1787307264203
CHANGE 01a023ce-711b-7000-820c-3870493dabc5 READY persisted_at=2026-08-21T10:14:25.638Z observed_at_unix_ms=1787307265656
CHANGE 01a023ce-711b-7000-820c-3870493dabc5 WORKING persisted_at=2026-08-21T10:14:30.144Z observed_at_unix_ms=1787307270162
CHANGE 01a023ce-711b-7000-820c-3870493dabc5 READY persisted_at=2026-08-21T10:14:32.208Z observed_at_unix_ms=1787307272225
```

Persisted-record timestamp to watcher-emission delay was 18 ms for the first
`task_started`, 18 ms for its `task_complete`, 18 ms for the second
`task_started`, and 17 ms for the second `task_complete`. These values exclude
the internal Codex Desktop-to-rollout flush latency, which remains
unmeasured.

## Resource baseline

For the optimized release watcher: settled idle working set was 23.62 MB,
peak working set during the controlled validation was 29.42 MB, and CPU time
was 0.00 seconds over a subsequent 10-second idle window. A preceding
10-second window spanning validation-related filesystem activity measured
1.9375 CPU seconds. The watcher returned 20 bounded sessions and has no
periodic reconciliation timer; reconciliation is notification/error driven.

The watcher does not attach to app-server, inspect processes, scrape UI, or
write Codex state.
