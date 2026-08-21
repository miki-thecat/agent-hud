# Issue #12 — native HUD profile

## Scope

This profile uses the optimized release binary on Windows from the
`perf/profile-native-hud` branch. Measurements are local process snapshots and
lightweight temporary startup instrumentation; the instrumentation was removed
before commit. The watcher remains read-only and event-driven.

## Attribution

| Checkpoint | Observed elapsed time from process start |
| --- | ---: |
| Window created | 22.8 ms |
| `GpuDevice::new_or_warp()` returned | 152.4 ms |
| Swap chain created | 160.8 ms |
| First paint completed | 560.2 ms |

The first paint measurement includes the initial message-loop scheduling and
text/resource creation. It is an attribution checkpoint, not an exact user
visible-window measurement.

## Before/after resource measurements

| Mode | Settled working set | Peak observed working set | Quiet CPU, 10 s |
| --- | ---: | ---: | ---: |
| Native HUD before Issue #12 fixes | 78.72 MB | 78.72 MB | 0.00 ms |
| Native HUD after Issue #12 fixes | 78.47 MB | 78.47 MB | 0.00 ms |
| Watcher-only reference | 29.28 MB | not separately sampled | 0.00 ms |

The earlier PR #11 controlled run recorded approximately 490 ms to visible
window and 81.48 MB settled/peak working set. The local process-snapshot
method used here is not directly interchangeable with that measurement. The
post-change startup proxy reached 30 MB at 220.7 ms; an exact visible-window
measurement was not repeated with this lightweight process sampler.

## Decision

The native graphics device and swap chain account for the large working-set
step relative to watcher-only operation. No low-risk optimization with a
measurable, attributable payoff was identified without adding lifecycle
complexity or weakening device-loss/DPI behavior. No graphics-stack change,
render loop, cache, dependency, or persistence layer was added.

The committed changes are limited to the requested architecture-documentation
correction and ensuring observation termination invalidates the header even when
all rows are already `UNKNOWN`.

The previously validated controlled `READY -> WORKING -> READY` run from PR #11
remains the live behavior evidence for the unchanged watcher/reducer path. A
new controlled turn was not initiated by this task because the HUD has no
Codex-control path and the repository contract prohibits steering sessions.
