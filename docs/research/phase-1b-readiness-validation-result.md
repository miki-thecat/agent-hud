# Phase 1B result — `task_complete` and human readiness

Date: 2026-08-21
Branch: `spike/windows-codex-observation`

## Verdict

Primary verdict: `TASK_COMPLETE_READINESS_STILL_UNCERTAIN`.

A persisted `task_complete` record is evidence that a rollout recorded a turn
completion boundary. This task did not perform the required controlled Desktop
turn experiments, so it cannot establish whether that boundary is sufficient
for the product's human-readiness claim. In particular, the available
historical evidence does not establish current Desktop ownership, that no
newer turn exists, that no turn/item remains active, or that an approval or
user-input request is not pending.

The proposed four-state reducer remains valid:

```text
WORKING | READY | ERROR | UNKNOWN
```

The candidate reducer therefore remains unvalidated. Until controlled evidence
shows that the persisted event has the required semantics and ordering, the
existing conservative model must not transition to `READY` from
`task_complete` alone.

## Question and claim boundary

The tested product question is:

> Can a human give this Codex Desktop session its next instruction now?

This is a readiness question, not a judgment about semantic task success,
correctness, or whether the implementation is complete in the user's broader
sense.

The strongest claim supported by the available evidence is narrower:

> A `task_complete` record was persisted in a particular rollout, at a
> particular position and time.

The evidence does not establish:

- that `task_complete => human can issue the next instruction`;
- current `READY` state;
- current Desktop session ownership;
- absence of a pending approval or user-input request;
- absence of a newer or still-active turn;
- task success or implementation correctness;
- safe live observation of Desktop-owned state;
- the required `task_started -> task_complete` sequence for a controlled
  Desktop turn;
- the superseding behavior of a second user turn.

## Evidence used

The result is based on the existing read-only Windows rollout observation and
the readiness-state design:

- `docs/research/phase-0c-rollout-observation-result.md` records that rollout
  JSONL contains historical completion and message/item records, but no
  authoritative live `WORKING`, `READY`, approval, or user-input field.
- `docs/design/readiness-state-model.md` classifies rollout record identity,
  ordinal, and timestamp as authoritative only for persisted history, while
  classifying completion events and assistant messages as strong correlation.
- The same design explicitly requires authoritative terminal evidence plus
  proof that no pending work exists before `READY`.

These sources establish the event's historical meaning, but do not answer the
Phase 1B question about current human readiness. The claim is therefore
`STILL_UNCERTAIN`, not supported or disproven.

## Controlled-experiment status

No experiment was performed that created, resumed, steered, interrupted,
approved, or answered a Codex Desktop session. Such operations would violate
the repository's read-only/no-interference constraints. Existing naturally
produced rollout records were used instead.

This means the following acceptance criteria remain unverified in this phase:

- a controlled normal Desktop turn observed from `task_started` through
  `task_complete`;
- a second user turn proving that a newer `task_started` supersedes READY;
- intermediate tool/command events shown not to be readiness boundaries;
- rollout visibility latency for a controlled test;
- abnormal termination semantics, where safely reproducible;

- a paired Desktop comparison of active, completed, approval-waiting, and
  user-input-waiting sessions;
- whether every actionable request is represented in rollout JSONL;
- the flush timing between a Desktop event and its persisted record;
- whether any private or undocumented live endpoint can be observed safely.

The absence of those experiments means the candidate reducer cannot be
promoted to an accepted product contract in this phase.

## Reducer implications

The adapter/reducer should apply these rules:

1. Record `task_complete` as evidence with rollout identity,
   sequence/ordinal, and event time.
2. Do not yet accept it as a standalone transition to `READY`.
3. Reject it when a newer active/pending observation supersedes it.
4. Preserve `UNKNOWN` when only persisted rollout evidence is available.
5. Permit `READY` only when a supported passive live source establishes a
   terminal boundary and no pending work or actionable request remains.

The UI may show a separately labelled historical completion/activity hint in a
future diagnostic or offline view. It must not present that hint as live
readiness.

## Safety and side effects

- No Codex Desktop UI was scraped.
- No process injection, debugger attachment, or memory inspection was used.
- No Desktop-owned request was acknowledged or answered.
- No session was created, resumed, steered, interrupted, approved, or
  answered.
- No rollout, session index, or state database file was modified.

## Recommended next step

Keep the current reducer contract and fail closed when only rollout history is
available. A future phase should target a documented, passive Windows
Desktop/app-server status source that exposes current turn state, terminal
state, and pending actionable requests with a proven non-interference
contract. Do not promote `task_complete`, file growth, timestamps, process
activity, or silence into `READY`.
