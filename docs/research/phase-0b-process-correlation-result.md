# Phase 0B local result — Windows process correlation

Date: 2026-08-21  
Branch: `spike/windows-codex-observation`

## Verdict

Primary verdict: `PROCESS_ACTIVITY_ONLY`.

Windows exposes reliable process facts and useful activity evidence, but the tested OS-level surfaces do not expose a Codex Thread/session/turn identifier. Multiple Codex threads are multiplexed through one Desktop-owned app-server PID, so no deterministic Thread ↔ child-process mapping was established.

## Observed process facts

At the observation point:

- `ChatGPT.exe` Desktop main process: PID `35692`, parent PID `19168`.
- Desktop-owned `codex.exe app-server`: PID `32272`, parent PID `35692`, started `2026-08-20 23:17:54`.
- Direct children of PID `32272`: 31 at the snapshot, including `codex-code-mode-host.exe`, many `node_repl.exe` processes, and `pwsh.exe` processes.
- There was one `codex.exe` process after cleanup: PID `32272`. The second PID observed during the experiment was the temporary independent probe and was terminated.
- Child command lines and parent PIDs were available through `Win32_Process`.
- Ordinary process metadata did not provide a child working directory. `Get-Process.Path` provided the executable path only; `Win32_Process` has no cwd field.
- The Desktop app-server exposed approximate CPU, working-set, handle, and I/O counters. Snapshot values for PID `32272` were approximately 188.5 seconds CPU, 508 MiB working set, 889 handles, 119,921 reads, and 72,662 writes. These are point-in-time counters, not thread status.

## IPC and lifecycle evidence

- A named pipe `codex-ipc` was visible by name. It was not opened, read, intercepted, or assumed to be a supported protocol.
- No TCP listener was associated with the observed Codex topology.
- The process snapshot provided creation timestamps and parent relationships. ETW process providers were discoverable (`Microsoft-Windows-Kernel-*` providers), but no ETW session was started in this task and no Desktop command lifecycle was captured through ETW.
- The separate stdio app-server probe demonstrated that a child can be started and initialized, but its PID/parent relationship is the probe's own, not a Desktop Thread identity.

## Correlation experiment result

The controlled two-thread/concurrent experiment requiring a user to issue distinct harmless commands from two Desktop threads was not performed: this task has no safe mechanism to send those commands to Desktop-owned threads, and the probe is prohibited from starting/steering/resuming them. Existing descendants were inspected, including children created at different times and with different command lines, but their process metadata contained no Codex Thread, turn, or item identifier. They all remained descendants of the same app-server PID.

Consequently, timestamp proximity, parent PID, command line, child executable, CPU/I/O deltas, and process existence are only `HEURISTIC` for Thread ownership. They cannot satisfy the requested deterministic mapping.

## Evidence classification

| Signal | Classification | Claim |
|---|---|---|
| PID, parent PID, executable path, creation time | `AUTHORITATIVE_OS` | Windows-reported process facts |
| Process start/exit when observed by a future ETW/snapshot probe | `AUTHORITATIVE_OS` | Process lifecycle, not Codex semantic state |
| Command line | `AUTHORITATIVE_OS` | Captured process metadata; no thread identity present |
| CPU/I/O counters | `AUTHORITATIVE_OS` | Resource activity only |
| `codex-ipc` pipe name | `UNAVAILABLE` | Existence is visible; safe semantic use and ownership are unproven |
| cwd | `UNAVAILABLE` | Not available from the ordinary metadata queried |
| Thread ↔ child-process mapping | `HEURISTIC` | Timing/tree proximity only; insufficient for production |
| WORKING vs approval/user-input/idle/done | `UNAVAILABLE` | Not derivable from OS process telemetry |

## Performance cost

No resident observer was added and no steady-state overhead claim is made. The bounded WMI/process snapshot and `Get-Process` queries completed in seconds, but this is diagnostic command latency rather than a production idle CPU/working-set measurement. A future ETW-only prototype should measure its own overhead before adoption.

## Architecture decision

OS telemetry can support a diagnostic activity indicator labelled as process activity, but it cannot power the product's authoritative per-thread state. Do not add process correlation to the production HUD as a substitute for a supported Codex semantic observation path. If a future supported semantic source becomes available, process telemetry may remain a secondary diagnostic signal, with the adapter and reducer preserving its weaker evidence class.
