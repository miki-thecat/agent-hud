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
to admit or remove recent root/user sessions. Truncation, malformed lifecycle
records, identity mismatch, and watcher overflow are treated as reconciliation
or degraded observation conditions.

The CLI remains explicitly historical/recorded readiness. It prints session
IDs and `WORKING`, `READY`, or `UNKNOWN` only; it does not print prompts,
messages, commands, model deltas, or rollout content.

## Verification status

Deterministic tests cover lifecycle append, command/item no-op behavior, partial
JSONL lines, duplicate notifications, newer-turn supersession, truncation, and
identity mismatch. Controlled live evidence and resource measurements are
recorded in the completion report for the corresponding implementation run.

The watcher does not attach to app-server, inspect processes, scrape UI, or
write Codex state.
