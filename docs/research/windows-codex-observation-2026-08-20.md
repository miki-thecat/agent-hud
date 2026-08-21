# Native Windows Codex observation — feasibility findings

Date: 2026-08-20

## Purpose

Record the current evidence governing Phase 0 of `agent-hud`: whether a separate read-only Windows process can observe live sessions owned by Codex Desktop without taking ownership, duplicating actionable requests, or relying on fabricated liveness.

This is a feasibility record, not a permanent statement about future Codex versions. Re-check against the locally installed Codex version before implementation.

## Current conclusion

Do **not** assume that `agent-hud` can attach to the app-server process already owned by Codex Desktop on native Windows.

As of the evidence below, the public protocol supports the exact live status we want, but native Windows Codex Desktop does not expose a supported second-client attachment path to its existing app-server process.

Therefore Phase 0 must prove the local topology before production UI work begins.

## Official app-server facts

OpenAI's current `codex app-server` documentation describes:

- JSON-RPC-style bidirectional protocol,
- stdio as the default transport,
- WebSocket listener as experimental/unsupported,
- Unix-domain socket transport for local control-plane clients,
- `thread/list` with per-thread `ThreadStatus`,
- `thread/loaded/list`,
- thread/turn/item notifications,
- per-connection notification opt-out,
- actionable approval and user-input requests.

The status model includes the information needed by the HUD when observed from the authoritative process, including loaded/active/idle/error state and active flags such as approval/user-input waiting.

Primary source:

- `openai/codex/codex-rs/app-server/README.md`

## Windows daemon/control-plane limitation

OpenAI's current `app-server-daemon` source explicitly gates daemon lifecycle to Unix:

```rust
#[cfg(not(unix))]
fn ensure_supported_platform() -> Result<()> {
    Err(anyhow!(
        "codex app-server daemon lifecycle is only supported on Unix platforms"
    ))
}
```

Primary source:

- `openai/codex/codex-rs/app-server-daemon/src/lib.rs`

This means a Unix-style shared daemon/control-socket topology cannot simply be assumed for native Windows.

## Strong current Windows evidence

OpenAI Codex issue #37450 (opened 2026-08-07, updated 2026-08-18) reports against Codex Desktop / CLI 0.147 alpha on Windows that:

- Codex Desktop retains the active thread writer after opening a thread,
- a second process cannot safely resume that thread while Desktop owns it,
- `codex app-server daemon start` and remote-control startup reject native Windows,
- the Desktop App does not expose `~/.codex/app-server-control/app-server-control.sock`,
- no listening TCP port or named pipe was found from the Codex processes,
- therefore there is currently no supported way for an external client to attach to the Desktop-owned app-server on native Windows.

Issue:

- `openai/codex#37450`

This is the closest current evidence to `agent-hud`'s exact use case.

## Persisted state is not sufficient for authoritative liveness

OpenAI Codex issue #36571 documents that persisted thread storage contains useful metadata such as cwd, branch, origin, name, preview, and timestamps, but no authoritative liveness/status/PID field. A running thread can therefore be indistinguishable from a finished thread in the persisted store.

Issue:

- `openai/codex#36571`

This rules out the following as authoritative status sources by themselves:

- `state_5.sqlite` thread rows,
- rollout/session file mtime,
- latest persisted message timestamp,
- process existence alone.

These may be useful as metadata or explicitly labelled heuristics, but must not silently become `WORKING`, `NEEDS INPUT`, or `DONE` truth.

## Existing monitor evidence

### `morgadoronan/codex-agents`

Useful reference for:

- status grouping,
- passive-observer safety,
- fail-closed behavior,
- separating manager ownership from discovered history,
- reducer/test design.

Its current shared live-state topology is macOS/Linux/WSL only. Its README explicitly says native Windows does not have the supported Unix-socket topology and shared live status is unavailable there.

Use it as a protocol/safety reference, not a drop-in implementation.

### `pingdotgg/t3code`

Useful reference for:

- normalizing Codex app-server notifications/requests into stable internal events,
- command/file-change/message/activity classification,
- adapter boundary design.

It is much broader and heavier than `agent-hud`; copy concepts, not architecture.

## Architectural implication

The product architecture must keep Codex integration replaceable:

```text
Codex observation source
        |
        v
Codex adapter
        |
        v
stable SessionObservation
        |
        v
reducer -> native HUD
```

The UI and reducer must not depend directly on one transport.

This allows a future supported Windows control socket / named pipe / daemon API to replace the Phase-0 adapter without rewriting the product.

## Phase-0 decision tree

### A. Supported live attachment exists on the user's installed version

If local evidence shows that current Codex Desktop exposes a supported attachable endpoint:

1. initialize a read-only observer,
2. prove session discovery,
3. prove live coarse status,
4. prove approval/user-input waiting state if safely available,
5. prove observer disconnect/reconnect does not alter Desktop,
6. only then implement the production adapter.

### B. No supported live attachment exists

Do **not** bypass the boundary through process injection, memory scraping, inherited-handle theft, UI scraping, or undocumented mutation of Codex state.

Record Phase 0 as `BLOCKED_SUPPORTED_LIVE_PATH` and separate two questions:

1. Can a useful degraded metadata/activity viewer be built from safe persisted read-only state?
2. Is that degraded behavior still valuable enough to satisfy the product requirement?

Do not automatically downgrade product semantics. `UNKNOWN` is better than a fabricated `WORKING` or `NEEDS INPUT` state.

### C. An attach path exists but duplicates actionable requests

Treat this as unsafe. Do not answer, acknowledge, decline, or consume the request on behalf of Codex Desktop.

Fail closed and record the exact version/protocol behavior.

## Phase-0 evidence required from the actual machine

The spike must record:

- Windows version,
- Codex Desktop version,
- bundled/runtime Codex CLI version,
- relevant `codex doctor` app-server status,
- Codex process command lines (without secrets),
- whether a supported control socket/listener/pipe is exposed,
- behavior of a fresh independent read-only `codex app-server` process,
- whether Desktop-created threads appear as loaded/active there,
- whether persisted state exposes authoritative liveness,
- any observable side effects on Desktop,
- final verdict and claim ceiling.

## Safety rules for the spike

Allowed by default:

- read public/local version information,
- inspect process metadata,
- inspect endpoint existence/listeners,
- inspect schemas and non-secret metadata,
- start an independent app-server child and send read-only discovery requests,
- read thread metadata without resuming a thread.

Not allowed by default:

- `thread/resume` on a Desktop-owned thread,
- `turn/start`, `turn/steer`, or `turn/interrupt`,
- replying to approval/user-input requests,
- attaching through unsupported process injection/debugger tricks,
- reading or printing auth tokens/secrets,
- modifying Codex Desktop files/state merely to force observability.

## Current engineering stance

The graphics stack is not the critical risk. The critical risk is live-state authority on native Windows.

Until Phase 0 proves an observation path, keep UI implementation minimal or mocked. A polished HUD built on guessed liveness would violate the central product requirement: trustworthy, glanceable state.
