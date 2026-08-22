# Issue 34 — workspace collision and integration-overlap feasibility

Date: 2026-08-22  
Scope: research/documentation only. This report does not implement runtime
conflict detection, locking, orchestration, readiness changes, or any change
under `src/`.

## Executive conclusion

The original proposal needs two separate risk models. They answer different
questions and must not be rendered as one “conflict” signal.

### `WORKSPACE_COLLISION_RISK`

Two coding-agent tasks are using the **same physical Git worktree or working
directory** and overlap on files or other mutable workspace state. Examples
include two agents editing one checked-out file, one agent observing another's
uncommitted changes, a branch switch during another task, or shared generated
artifacts. This is immediate filesystem/workspace contamination. Worktree
isolation is the primary mitigation.

### `INTEGRATION_OVERLAP`

Two tasks use **different isolated worktrees**, but their branches change
overlapping logical repository areas that will later be integrated. This can
lead to a textual merge conflict, a semantic conflict, incompatible contract
assumptions, or an integration-order requirement. It is not a workspace
collision. Worktrees solve physical isolation, not logical integration
coordination.

Therefore, path intersection is not by itself a current collision or a
guaranteed Git merge conflict. Two branches can touch the same path and merge
cleanly; two branches can touch different paths and still be semantically
incompatible. A future HUD may expose conservative, read-only evidence for
the two layers, but current evidence does not justify claiming ownership,
locks, authorship, or that tasks “will conflict”.

## Evidence reviewed and claim ceiling

| Evidence | Classification | Safe claim | Boundary |
| --- | --- | --- | --- |
| Validated rollout `FileChange` items in [Phase 0C](phase-0c-rollout-observation-result.md) | `STRONG_CORRELATION`; `AUTHORITATIVE` only for the exact recorded payload | A validated session recorded a path, kind, status, sequence/turn, and possibly diff | Persisted history is not current editing, ownership, causation of current bytes, or a lease |
| [Phase 1C session discovery](phase-1c-session-discovery-result.md) and validated root/user identity chain | `STRONG_CORRELATION` for association | A session is associated with a discovered `cwd`/rollout identity | It does not prove an open Desktop chat, process ownership, or liveness |
| [Phase 1D rollout watcher](phase-1d-live-rollout-watcher-result.md) | `STRONG_CORRELATION` for observed append/reconstruction behavior | A rollout can be read incrementally and reconstructed after restart | File growth/flush is not a current activity or ownership contract |
| Git worktree/status/diff/merge semantics | `AUTHORITATIVE` for the queried repository state at an instant | A root, ref, index/worktree difference, or branch comparison was observed | Git does not attribute a dirty path to a Codex session or prove a future conflict |
| [Codex app-server source](https://github.com/openai/codex/tree/main/codex-rs/app-server) | `AUTHORITATIVE` only for the documented protocol facts, subject to version and transport | A supported endpoint may define exact item/status facts | The native-Windows Desktop passive-observation path remains `UNAVAILABLE` in this repository's evidence |
| [Git worktree documentation](https://git-scm.com/docs/git-worktree), [status](https://git-scm.com/docs/git-status), and [diff/merge documentation](https://git-scm.com/docs/git-merge) | `AUTHORITATIVE` for Git behavior | Worktrees separate checked-out files; Git compares trees/index/worktree during integration | Isolation does not remove later textual or semantic overlap |
| [Windows USN journal](https://learn.microsoft.com/en-us/windows/win32/fileio/change-journal-operations) or filesystem notifications | `HEURISTIC` | A path changed or was observed recently | No Codex identity; writers may be humans, builds, editors, sync, or antivirus |
| Timestamps, process names, silence, and `git status` authorship guesses | `HEURISTIC` or `UNAVAILABLE` | At most, weak corroboration or missing evidence | Do not infer who owns or is editing a path |

`AUTHORITATIVE` is reserved for an exact fact defined by the source.
`STRONG_CORRELATION` means useful session/worktree association, not ownership.
`HEURISTIC` means weak corroboration only. `UNAVAILABLE` means the required
identity, freshness, transport, or semantics are not established. A recent
`FileChange` is not proof that a file is currently being edited; no session
owns a file merely because it mentioned that path; and a `git status` result
cannot establish authorship.

## Two-layer model

The future reducer should preserve evidence records, then classify each layer
independently:

```text
Evidence {
  repository_identity
  worktree_root
  branch_or_ref
  base_commit?
  head_commit?
  session_or_thread_id
  normalized_repo_relative_path?
  original_path
  change_kind
  source                 // rollout_file_change, git, filesystem, protocol
  source_sequence_or_turn?
  observed_at
  freshness
  persisted_or_live
  confidence
}
```

`WORKSPACE_COLLISION_RISK` requires two validated sessions mapped to the same
canonical physical worktree, overlapping mutable path/state evidence, and a
defensible freshness window. A shared root without path/state evidence is not
enough. An ambiguous root, malformed path, or unavailable identity fails
closed.

`INTEGRATION_OVERLAP` requires distinct worktrees/repository contexts and
branch/base/head evidence or other explicit logical dependency evidence. Same
repo-relative paths are useful review inputs, not proof of a merge conflict.
Different paths do not clear the risk: shared interfaces, schemas, generated
contracts, configuration, or assumptions can be semantically incompatible.
Already-merged or cherry-picked equivalent changes should be classified as
resolved/`NO_EVIDENCE` for an outstanding integration risk, not re-alerted.

Suggested result vocabulary is deliberately explicit:

| Result | Meaning |
| --- | --- |
| `NO_EVIDENCE` | No valid current evidence for the queried layer |
| `WORKSPACE_COLLISION_RISK` | Same physical worktree plus fresh overlapping mutable-state evidence |
| `INTEGRATION_OVERLAP` | Separate worktrees with evidence of overlapping logical change/dependency |
| `STALE_EVIDENCE` | Historical evidence retained for explanation but outside the freshness boundary |
| `UNAVAILABLE` | Identity, path, repository, transport, or semantic evidence is missing/ambiguous |

These are evidence classifications, not commands to block, pause, steer,
interrupt, answer, or coordinate Codex. The feature remains read-only.

## Failure modes and safe interpretation

| Case | Correct interpretation |
| --- | --- |
| Same file, same worktree | `WORKSPACE_COLLISION_RISK` only when both session identities, physical root, path, and freshness are validated; otherwise `UNAVAILABLE` |
| Same repo-relative file, separate worktrees | No physical collision; possible `INTEGRATION_OVERLAP` for later integration |
| Different files, shared interface/contract | May be `INTEGRATION_OVERLAP` despite disjoint paths; requires dependency evidence, not string guesses |
| Sequential edits | Preserve turn/sequence and time; do not call historical overlap simultaneous |
| Stale `FileChange` history | `STALE_EVIDENCE`; rollout recency alone is not current activity |
| Generated/build/cache files | Separate or exclude by explicit policy; writes are not source-task authorship |
| Uncommitted human edits | Git may report dirty state, but attribution is `UNAVAILABLE`; never overwrite, stage, stash, or lock |
| Rename/delete | Preserve `kind`; correlate both endpoints only when the source supplies both |
| Separate repositories with identical relative paths | No comparison unless repository identity matches |
| Different base commits | Record base/head divergence; do not infer conflict from path overlap alone |
| Already merged/cherry-picked changes | Reconcile against the current integration base and suppress resolved overlap |
| Malformed/ambiguous path | Do not guess via string similarity or path escaping; return `UNAVAILABLE` |

## Evidence-preserving workflow recommendation

Keep the operational recommendation lightweight:

```text
1 issue → 1 Codex task → 1 isolated worktree → 1 branch → 1 PR
```

Before fan-out, perform dependency and hotspot/shared-contract analysis. Give
tasks clear file/area boundaries where overlap is plausible. Integrate
serially, then verify against updated `main` after each important merge. This
is dependency-aware integration hygiene, not an orchestration proposal.

For future cross-worktree analysis, retain repository identity, physical root,
branch/ref, base/head commits, session/thread identity, normalized repo-relative
paths, observed `FileChange` source/sequence/turn, freshness, and persisted
versus live classification. Never infer authorship from `git status`.

## Smallest defensible future slice

No runtime implementation is justified by the current evidence. If the
feature is later accepted, start with deterministic, offline foundations:

1. preserve identity-validated `FileChange` path/kind/status/turn/ordinal;
2. implement pure Windows path normalization and physical-worktree checks;
3. keep separate reducers for same-worktree collision and later integration
   overlap, with fixtures for every failure mode above;
4. use Git only for root/ref/base/head and dirty-state explanation, never
   attribution; and
5. gate any live feature on a supported passive Windows observation spike.

Do not add file locks, USN scanning, process injection, private IPC, a tight
polling loop, or automatic coordination. If passive observation cannot be
proven safe, remain `UNAVAILABLE`.

## Decision

Proceed with documentation and typed foundations only when separately
accepted. Defer runtime risk detection until session-to-rollout identity,
path preservation, deterministic worktree resolution, freshness/expiry
fixtures, and a safe passive Windows source are proven. Worktree isolation is
the primary mitigation for `WORKSPACE_COLLISION_RISK`; serial dependency-aware
integration is the mitigation for `INTEGRATION_OVERLAP`.

This research pass remains documentation-only and does not modify `src/` or
readiness behavior.
