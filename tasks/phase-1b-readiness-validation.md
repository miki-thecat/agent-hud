# Phase 1B — Validate `task_complete` as human-readiness boundary

## Goal

Determine whether persisted Codex rollout events can reliably answer the actual MVP product question:

> Can a human give this Codex Desktop session its next instruction now?

This task does **not** attempt to prove that the implementation/task succeeded semantically. It validates whether the current Codex turn has ended and control has returned to the human.

## Why this task exists

Phase 1 defined `READY` too conservatively by requiring a supported authoritative live Desktop status source. On current native Windows that source is unavailable, which would make almost every session `UNKNOWN`.

Codex's protocol defines `TurnComplete` as successful turn completion and v1-compatible rollouts serialize that event as `task_complete`. The product requirement is human readiness, not task correctness. Therefore `task_complete` may be sufficient to establish `READY` if controlled local evidence shows that:

- it is emitted after the Desktop turn stops accepting internal work,
- it appears in the rollout with acceptable latency,
- the next user instruction produces a newer `task_started`,
- aborted/error paths can be distinguished,
- stale historical `task_complete` records can be rejected by ordering/turn identity.

## Scope

### In scope

- Read-only observation of rollout JSONL.
- Disposable / controlled Codex Desktop test turns.
- `task_started` / `task_complete` / `turn_aborted` / error-related lifecycle events.
- Turn IDs and event ordering.
- Event-to-rollout visibility latency where measurable.
- State-transition behavior across two or more consecutive turns in the same thread.
- A minimal READY/WORKING/ERROR/UNKNOWN mapping proposal backed by evidence.

### Out of scope

- Production HUD UI.
- App-server attachment.
- Approval/user-input automation.
- Process injection, debugger attachment, process memory scraping, or UI scraping.
- Proving semantic task success/correctness.
- Treating command success/failure as session completion by itself.

## Product semantics to validate

For this project:

- `WORKING` means the latest known turn has started and has not reached a later terminal lifecycle event.
- `READY` means the latest known turn has emitted a terminal `task_complete`, so the human can issue the next instruction.
- `ERROR` means the latest turn terminated through an explicit terminal failure/error path that prevents normal completion.
- `UNKNOWN` means ordering/identity/evidence is insufficient or contradictory.

A `READY` result must **not** be described as "task succeeded" or "implementation is correct".

## Controlled experiment

Use a disposable/new Codex Desktop chat or another test thread where controlled instructions are safe.

### Experiment A — normal trivial turn

1. Identify the current thread/rollout deterministically using available thread identity metadata.
2. Start a background read-only watcher before or at the beginning of the test turn.
3. Run a trivial instruction that requires no destructive action and produces a normal assistant response.
4. Record the ordered lifecycle events and the first time each becomes visible to the watcher.
5. Verify whether the terminal event is `task_complete` and capture its `turn_id`.

### Experiment B — consecutive turns

1. After Experiment A reaches the human-ready Desktop state, send one new trivial instruction in the same chat.
2. Verify that a newer `task_started` with a new turn identity supersedes the prior `task_complete`.
3. Verify that the second terminal lifecycle event restores READY.

Expected state sequence if supported by evidence:

```text
UNKNOWN -> WORKING -> READY -> WORKING -> READY
```

### Experiment C — tool/command turn

Run one safe turn containing a harmless command or file-neutral tool action.

Verify that intermediate command/tool completion does not produce READY before the turn-level terminal lifecycle event.

### Experiment D — abnormal termination when safely reproducible

If a safe, non-destructive method exists to cancel/stop a disposable turn through normal Codex Desktop user controls, observe whether the rollout records `turn_aborted`, error, or another terminal event.

Do not force an abnormal condition through unsupported APIs or process termination merely to satisfy this experiment. If safe reproduction is unavailable, mark this part `NOT_VERIFIED`.

## Evidence classification

For every candidate signal, classify it as:

- `AUTHORITATIVE_FOR_RECORDED_TURN_LIFECYCLE`
- `STRONG_CORRELATION`
- `HEURISTIC`
- `UNAVAILABLE`

The claim ceiling matters: a rollout lifecycle event may be authoritative that Codex recorded a turn boundary without being an officially supported external Desktop observer API.

## Important edge cases

Explicitly evaluate these known classes:

1. `task_complete` with `last_agent_message = null`.
2. `task_complete` following progress/commentary even when the original broader task was incomplete.
3. stale earlier `task_complete` after a newer `task_started`.
4. subagent lifecycle records versus root/user thread records.
5. rollout append delay causing a brief stale display.
6. watcher restart while the latest rollout already contains a completed turn.

For this product, cases 1–2 may still mean **human-ready** if the turn has genuinely ended. Do not confuse task incompleteness with inability to accept another user turn.

## Decision rule to test

Evaluate this candidate reducer:

```text
latest lifecycle event for latest root/user turn

 task_started   -> WORKING
 task_complete  -> READY
 terminal error -> ERROR
 turn_aborted   -> READY or ERROR only if actual Desktop behavior and product semantics justify it;
                   otherwise UNKNOWN
 no trustworthy lifecycle event -> UNKNOWN
```

Use turn identity and ordering; never use silence/timeouts alone to transition to READY.

## Acceptance criteria

- [ ] At least one controlled normal Desktop turn is observed from `task_started` through `task_complete`.
- [ ] The `turn_id`/ordering relationship is documented.
- [ ] A second user instruction demonstrates whether a newer `task_started` cleanly supersedes READY.
- [ ] Intermediate tool/command events are shown not to be terminal readiness boundaries.
- [ ] Rollout visibility latency is measured or bounded for the controlled test.
- [ ] The claim `task_complete => human can issue the next instruction` is classified as supported, unsupported, or still uncertain.
- [ ] Phase 1 state-model changes required by the evidence are identified explicitly.

## Output

Create:

`docs/research/phase-1b-readiness-validation-result.md`

Include:

- environment/version,
- experimental procedure,
- event excerpts with sensitive content removed,
- turn IDs/ordering,
- measured visibility timing,
- state sequence,
- edge-case findings,
- final verdict,
- exact proposed changes to `docs/design/readiness-state-model.md`.

If the evidence supports the candidate reducer, update `docs/design/readiness-state-model.md` in the same task so it reflects the product's actual human-readiness semantics.

## Git workflow

After completing the task:

1. inspect `git status` and `git diff`,
2. run relevant checks (`git diff --check` at minimum for docs-only changes),
3. commit the coherent changes,
4. push the current branch,
5. report commit hash, changed files, checks run, and remaining uncertainty.

Do not merge to `main`, force-push, or include unrelated changes.
