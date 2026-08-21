# Issue #18 — final `main` smoke

Date: 2026-08-21  
Commit: `475f980065110cf9d298d2eaac06ba1b713fed3b`

## Verification

Validated from a fresh disposable clone of `main` on Windows 10.0.26200.0
with Rust/Cargo 1.98.0:

```text
cargo fmt --check                         PASS
cargo check --locked                      PASS
cargo clippy --locked --all-targets -- -D warnings  PASS
cargo test --locked                       PASS (24 passed, 0 failed)
cargo build --locked --release             PASS
```

The release executable stayed alive and responsive for inspection, with
window title `agent-hud — Recent local sessions` and no stderr output. The
release watcher emitted the bounded maximum of 20 root/user sessions, so the
multi-session catalog was present. README and the Issue #16 readiness report
were present on `main`; the implementation still states recorded readiness in
a bounded **Recent local sessions** view, not exact open-chat discovery.

## Lifecycle re-observation limitation

The final-main environment could not re-observe the lifecycle turn. No
disposable normal Codex root/user turn could be started in this validation
environment: the installed `codex.exe` was present but returned Windows
`Access is denied` when invoked, and Codex App/CLI UI automation is out of
scope for the available validation controls.

This does not block the release verdict. Prior live lifecycle/control-session
evidence remains valid because final `main` has no executable-input changes
relative to the validated product code: the net changes from the live-validated
product tip were documentation-only (`README.md` and the Issue #16 readiness
report), with no changes to `src/`, `Cargo.toml`, `Cargo.lock`, build scripts,
or other executable inputs. Final `main` independently passed the build,
test, startup, and discovery smoke above.

## Verdict

`READY_TO_TAG_V0.1.0`
