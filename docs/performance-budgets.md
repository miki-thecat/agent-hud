# Performance budget guardrails

Issue #97 establishes a repeatable local measurement surface without adding
telemetry or changing the HUD runtime. The source of truth is
[`tools/performance-budgets.json`](../tools/performance-budgets.json).

Run it against a release binary on Windows:

```powershell
cargo build --release
powershell -NoProfile -File .\tools\measure-performance.ps1 `
  -Binary .\target\release\agent-hud.exe
```

Add `-Enforce` to fail when a measurement exceeds the documented regression
ceilings. The helper terminates only the process it started and emits one JSON
measurement suitable for attaching to CI artifacts. It does not inspect,
modify, or send data from Codex sessions.

## What is measured

The helper measures native-window-visible startup, settled and peak working set, and
additional CPU time during a quiet interval. Startup is deliberately named a
**window-visible proxy**: it is not a first-paint measurement. The native
graphics path can initialize after the process becomes visible, so first paint
must be measured with a separate instrumented run before making that claim.

`-EventToVisibleMs <number>` records an event-to-visible result from an
instrumented trace and includes it in the JSON output and optional gate. The
helper does not invent this value from timestamps or polling.

## Targets, ceilings, and variance

Targets in the JSON retain the product aspirations (<150 ms startup, <20 MiB
working set, <50 ms event-to-visible). Regression ceilings are the current
guardrails for native Windows builds: 1200 ms window-visible startup, 100 MiB
settled working set, 110 MiB peak working set, 100 ms event-to-visible, and
2000 ms additional CPU over the default ten-second idle sample. The CPU ceiling
is intentionally a coarse regression tripwire, not a claim that the app should
remain busy while idle.

These are local Windows measurements, not cross-machine guarantees. Windows
build, graphics driver, GPU/WARP selection, antivirus, desktop load, power
mode, and cold-vs-warm filesystem state can move the result. Compare runs on
the same machine and build configuration; record the JSON artifact and do not
convert one run into a population-wide claim. A ceiling change requires a
fresh measurement and an explanation in the PR.

The helper intentionally does not collect external telemetry, calculate vanity
scores, or gate on the aspirational targets that the current native graphics
profile has not yet demonstrated.
