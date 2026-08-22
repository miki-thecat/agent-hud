# Agent HUD user guide

Agent HUD is a small, read-only Windows companion for seeing the recorded
readiness of recent local Codex sessions. Codex App remains the place where
you chat, approve actions, and provide input.

## Prerequisites

- Windows for the native HUD window.
- Rust and Cargo from [rustup](https://rustup.rs/).
- A local Codex installation that has created `state_5.sqlite` and session
  rollout files under the normal `.codex` directory.

The repository also builds on non-Windows hosts for the CLI snapshot path, but
the native HUD is Windows-only.

## Build and run

From the repository root:

```text
cargo run --release
```

On Windows, this opens the native HUD. The window labels its data **Recent
local sessions** because the current discovery path reads persisted local
state; it does not claim to enumerate exactly which Codex App windows are
open.

For the event-driven text watcher, use:

```text
cargo run --release -- --watch
```

The watcher prints an initial snapshot and bounded changes as persisted state
or rollout files change. It is useful for validating the data path and for
diagnostics; it remains read-only.

For a faster development build, omit `--release` from either command. Cargo
places the compiled binary in `target/release` or `target/debug`.

## Optional local configuration

Agent HUD optionally reads `.codex/agent-hud.json` from the current Windows
user profile. The file is local-only and is not created automatically. The
supported settings are window dimensions; omitted settings use these defaults:

```json
{
  "window_width": 620,
  "window_height": 720
}
```

Window width must be between 320 and 3840 pixels, and height between 240 and
2160 pixels. Invalid configuration is reported and ignored; the HUD falls
back to its defaults. Unknown fields are ignored so the local format can grow
without affecting older builds.

## Reading a session row

The current readiness vocabulary is deliberately small:

- `WORKING` — a validated root/user turn has started and has not reached its
  matching completion boundary.
- `READY` — a validated root/user turn has reached its matching completion
  boundary. This means another instruction may be possible; it does not mean
  the work was successful or correct.
- `UNKNOWN` — evidence is missing, stale, contradictory, historical-only, or
  the observation path is unavailable. This is the safe fallback.

The HUD may also show the latest recorded result, changed files, project
identity, and verification evidence when those facts are available. They are
recorded metadata, not proof of a live Codex App state.

The most important rule is that timestamps, process activity, file growth,
assistant messages, individual tool completions, and silence do not establish
readiness on their own. See the complete [readiness state model](design/readiness-state-model.md)
for transition rules and false-positive cases.

## Troubleshooting

- If startup reports that the Codex state database cannot be found, start
  Codex once and confirm that its local `.codex` state exists.
- If a row becomes `UNKNOWN`, treat that as a loss of trustworthy evidence,
  not as a guess that the session is idle or finished.
- If the watcher stops after a filesystem or app restart, restart it. The
  watcher performs bounded recovery; it does not take ownership of Codex
  sessions or answer actionable requests.

There is no cloud service, telemetry backend, or application database owned by
Agent HUD. Session history is reconstructed from the local Codex sources.
