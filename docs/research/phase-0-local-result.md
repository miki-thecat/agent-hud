# Phase 0 local result — native Windows Codex observation

Date: 2026-08-21  
Branch: `spike/windows-codex-observation`

## Verdict

Primary verdict: `BLOCKED_SUPPORTED_LIVE_PATH`.

The tested Windows installation exposes a Desktop-owned `codex.exe app-server`, but no supported external control endpoint was found. A separate stdio app-server can initialize, but it is a new ephemeral server and is not an attachment to the Desktop-owned server. It therefore cannot establish authoritative live state for Desktop-owned threads.

## Environment

- OS: Windows 11 Home, version `10.0.26200`, x64, Japanese locale.
- Desktop package path: `C:\Program Files\WindowsApps\OpenAI.Codex_26.814.5517.0_x64__2p2nqsd0c76g0`.
- CLI/runtime: `codex-cli 0.144.3`, npm-installed Windows runtime.
- `codex doctor`: state databases passed integrity checks; background app-server reported `not running (ephemeral mode)`; control socket path was reported but absent.
- Doctor also reported one rollout/state-DB discrepancy. This is relevant to persisted-state trust and was not repaired by this task.

## Direct observation procedure

Read-only commands actually run:

- `codex --version`
- `codex app-server --help`
- `codex doctor`
- `Get-CimInstance Win32_Process` for process metadata
- `Get-NetTCPConnection -State Listen`
- named-pipe name enumeration under `\\.\pipe\`
- existence check for `%USERPROFILE%\.codex\app-server-control\app-server-control.sock`
- a separate `codex app-server --stdio` child with only `initialize`, `initialized`, and `thread/list` requests

The independent server returned an initialize result identifying itself as `Codex Desktop/0.144.3` and emitted `remoteControl/status/changed` with status `disabled`. It did not return a `thread/list` result in the bounded retries; one run logged a state-db `read_repair_rollout_path` warning. The child was terminated after the bounded probe. No Desktop-owned process was terminated or sent a protocol request.

## Observed topology

The Desktop process tree included:

```text
ChatGPT.exe PID 35692, parent 19168
└─ codex.exe PID 32272, parent 35692
   ├─ codex-code-mode-host.exe PID 24292
   ├─ many node_repl.exe children
   └─ pwsh.exe children
```

The Desktop-owned server command line was:

```text
codex.exe -c features.code_mode_host=true app-server --analytics-default-enabled
```

No listening TCP endpoint was reported. The documented-looking Unix control-socket path was absent. A named pipe named `codex-ipc` existed, but its ownership, protocol, and observer semantics were not documented or established; it was not opened or intercepted.

## Claim ceiling

- `thread/list`/`thread/loaded/list` data from a separately spawned server would describe that server's ownership context, not prove Desktop-owned live state.
- Persisted files/databases, timestamps, latest messages, process existence, and the `codex-ipc` name cannot be promoted to `WORKING`, `WAITING_FOR_APPROVAL`, `WAITING_FOR_USER_INPUT`, or `DONE`.
- Approval/user-input state and current activity/latest message were not authoritatively observed.
- No Desktop-created thread was resumed, started, steered, interrupted, or answered.

## Side effects and unverified items

No Desktop degradation, lock conflict, duplicate actionable request, or crash was observed. The independent probe produced only its own initialization/status output. A temporary probe left by an interrupted command was identified and terminated by PID; the Desktop PID remained untouched.

Not verified: a user-coordinated idle/active/waiting-for-input comparison across Desktop threads, because creating or resolving those states from the probe would violate the contract and no natural user-created state was made available during the bounded experiment.

## Architecture decision

Do not implement a live Windows Codex adapter or UI state reducer based on this installation. Keep the observation source replaceable and fail closed until Codex Desktop exposes a supported passive endpoint. A future task may separately evaluate a clearly labelled metadata-only view, but it must not infer live status from persisted state or process existence.
