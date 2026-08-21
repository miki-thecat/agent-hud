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

## Blocking evidence

The required final live smoke was not independently completed on this
integrated `main` build. No disposable normal Codex root/user turn could be
started in this validation environment: the installed `codex.exe` was present
but returned Windows `Access is denied` when invoked, and Codex App/CLI UI
automation is out of scope for the available validation controls. Therefore
this run does not establish `READY -> WORKING -> READY` without HUD restart or
the unchanged independent control-session criterion.

## Verdict

`BLOCKED`
