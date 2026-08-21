# Issue #16 — integrated readiness validation

Date: 2026-08-21
Branch: `feat/glanceable-readiness-ui`
Commit validated: `b433a70`

## Verdict

The integrated v0.1 checkout is buildable and runnable on this Windows host.
The native HUD launches normally, the read-only watcher discovers a bounded
multi-session set, and the recorded readiness model remains consistent with
the documented `READY` / `WORKING` / `UNKNOWN` semantics.

No product features were added for this validation.

## Environment

- Windows: `10.0.26200.0`
- Rust: `rustc 1.98.0 (88d9e12ae 2026-08-18)`
- Cargo: `cargo 1.98.0 (797e8a9bc 2026-08-05)`
- Build: locked release build from a fresh local clone of this branch
- Codex data source: local `%USERPROFILE%\\.codex` state; the watcher only
  reads persisted state and rollout records

## Clean checkout

A disposable clone was created at
`%TEMP%\\agent-hud-issue-16-clean`; it had no build artifacts before
validation. The following commands all passed in that clone:

```text
cargo fmt --check
cargo check --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked       # 24 passed, 0 failed
cargo build --locked --release
```

## Native HUD startup

The clean-clone release binary was launched without `--watch`. The process
remained alive, created the expected native window titled
`agent-hud — Recent local sessions`, and was then closed normally by the
validation harness.

| Measurement | Result |
| --- | ---: |
| Process visible | 135 ms |
| Settled working set after 1.5 s | 83.08 MB |
| Additional CPU during the 10 s idle sample | 0.00 s |

The working-set result is consistent with the existing native HUD profile in
`docs/research/issue-12-native-hud-profile.md`; it remains above the original
20 MB target because the native graphics stack dominates the resident cost.

## Multi-session observation

The clean-clone release watcher was run against the local read-only Codex
state. It emitted 20 bounded root/user sessions, the configured maximum. The
observed initial set contained 18 `READY` sessions and 2 `WORKING` sessions,
with distinct IDs and no duplicate or cross-session state in the output.

Representative output:

```text
INITIAL 01a02387-4eea-7f71-b446-ae078cc6815 READY
INITIAL 01a023bd-8807-76b0-bd61-995b00200066 READY
INITIAL 01a02463-28ae-7940-be59-1e4991b25a15 WORKING
INITIAL 01a0246b-d9ac-7200-a11b-eeb5eb99b170 WORKING
```

The watcher process became visible in 236 ms, settled at 36.18 MB in this
run, and was stopped without changing Codex state.

The exact `READY` / `WORKING` / `UNKNOWN` three-session fixture is covered by
the reducer and watcher tests. No real Codex rollout was edited to manufacture
an `UNKNOWN` session.

## Lifecycle and recovery

The controlled live lifecycle evidence recorded in
`docs/research/phase-1d-live-rollout-watcher-result.md` demonstrates, without
restarting the watcher:

```text
READY -> WORKING -> READY -> WORKING -> READY
```

The current checkout's deterministic tests cover independent tracking,
newer-turn supersession, missed appends followed by reconciliation, truncation,
identity mismatch, and failed recovery degrading tracked readiness to
`UNKNOWN`. In particular, `failed_recovery_degrades_tracked_readiness_to_unknown`
proves that stale `READY` / `WORKING` state is not retained after recovery
cannot re-establish validated observation.

## Documentation consistency

The implementation and documentation agree on the claim ceiling:

- the view is a bounded **Recent local sessions** view, not exact open-chat
  discovery;
- readiness is recorded root/user turn lifecycle state;
- rollout observation is read-only and does not attach to app-server or own
  approvals/user input;
- missing, contradictory, or degraded evidence fails closed to `UNKNOWN`;
- `READY` means the recorded turn boundary returned control to the human, not
  that the broader task was correct or successful.

## Remaining uncertainty

This validation does not establish a supported passive live Desktop status API,
approval/user-input observation, or the internal Desktop-to-rollout flush
latency. Those limits remain documented in the Phase 0 and Phase 1 research
results and are unchanged by Issue #16.

