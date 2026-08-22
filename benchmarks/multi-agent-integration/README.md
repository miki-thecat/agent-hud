# Multi-agent integration benchmark

Issue #100 defines a deterministic end-to-end scenario for the workflow that
Agent HUD monitors. The scenario is deliberately test-only: it does not add
agent spawning, orchestration, readiness rules, telemetry, or persistence.

The fixture contains three sessions in one project:

| Session | Expected readiness | Verification | Changed files | Integration evidence |
| --- | --- | --- | --- | --- |
| alpha | `WORKING` | `cargo test` passed | `src/alpha.rs`, `src/shared.rs` | overlaps beta on `src/shared.rs` |
| beta | `READY` | `cargo test` failed | `src/beta.rs`, `src/shared.rs` | overlaps alpha on `src/shared.rs` |
| gamma | `UNKNOWN` | none | none | absent lifecycle evidence is fail-closed |

The Rust test exercises persisted discovery, project identity/grouping,
readiness, workflow timeline normalization, verification evidence, changed-file
retention, and a deterministic same-file integration-risk assertion. It writes
only to a process-specific directory under the OS temp directory and reports
fixture discovery latency.

## Run

From the repository root:

```powershell
.\benchmarks\multi-agent-integration\run.ps1
```

The runner creates two actual Git worktrees under a unique temp directory,
sets an isolated `USERPROFILE` and `CARGO_TARGET_DIR`, runs the focused test,
and writes a small `result.json` outside the repository. Pass `-KeepArtifacts`
to retain that directory for inspection; otherwise it is removed on exit.

## Expected failure modes

- missing Rust/Cargo: the runner stops before creating benchmark state;
- fixture/parser regression: the focused test fails with the expected field;
- absent lifecycle evidence: `gamma` remains `UNKNOWN`, never inferred as ready;
- same-file overlap: the test reports evidence only; it does not attempt an
  automatic merge or conflict resolution;
- worktree setup failure: the runner stops and leaves normal Codex state alone.
