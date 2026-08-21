# Issue #12 — native HUD profile

## Scope

This profile uses the optimized release binary on Windows from the
`perf/profile-native-hud` branch. Measurements are local process snapshots and
lightweight temporary startup instrumentation; the instrumentation was removed
before commit. The watcher and HUD remain read-only and event-driven.

## Startup terminology and attribution

`Window::create()` calls `ShowWindow` before returning. The first checkpoint is
therefore named **window-show boundary**, not visible content. **First-painted
content** is the later checkpoint after the first successful swap-chain
present. The timings below are release-build process-start stopwatch readings;
the marker was written at each boundary and observed by a 10 ms external
sampler.

| Checkpoint | Elapsed from process start | Working set | Private bytes |
| --- | ---: | ---: | ---: |
| Window-show boundary / before GPU | 401.0 ms | 33.22 MB | 20.38 MB |
| after `GpuDevice::new_or_warp()` | 469.8 ms | 56.27 MB | 37.86 MB |
| after swap-chain creation | 533.7 ms | 57.43 MB | 38.70 MB |
| first-painted content / settled HUD | 958.4 ms | 79.11 MB | 58.28 MB |

The settled post-sampler reading was 82.41 MB. These are practical working-set
snapshots, not allocator attribution: the GPU/device step is the largest early
resident increase (+23.05 MB), while first paint/text resources add the next
large increase (+21.68 MB). This supports a graphics/UI-stack attribution, but
does not prove that one internal allocation is solely responsible.

## Before/after resource measurements

| Mode | Settled working set | Peak observed working set | Quiet CPU, 10 s |
| --- | ---: | ---: | ---: |
| Native HUD before Issue #12 fixes | 78.72 MB | 78.72 MB | 0.00 ms |
| Native HUD after Issue #12 fixes | 82.41 MB | 83.50 MB | 0.00 ms |
| Watcher-only reference | 29.28 MB | not separately sampled | 0.00 ms |

The earlier PR #11 value of approximately 490 ms was a different
startup-to-visible-window method and is retained only as historical context;
it must not be compared numerically with the 958.4 ms first-painted-content
reading above. The directly comparable metric established here is the named
process-start-to-first-painted-content measurement; a like-for-like pre-fix
run was not available on this checkout, so no before/after startup delta is
asserted.

## Decision

The native graphics/UI path accounts for the large working-set step relative
to watcher-only operation. The stage snapshots show the largest early step at
GPU-device initialization and another large step at first paint; they do not
justify attributing all of the resident cost to the swap chain alone. No
low-risk optimization with a measurable, attributable payoff was identified
without adding lifecycle complexity or weakening device-loss/DPI behavior. No
graphics-stack change, render loop, cache, dependency, or persistence layer was
added.

The committed changes are limited to the requested architecture-documentation
correction and ensuring observation termination invalidates the header even when
all rows are already `UNKNOWN`.

## Post-change controlled HUD validation

The release HUD was started once and kept running during a normal disposable
turn in tracked session
`01a023ce-711b-7000-820c-3870493dabc5`. The same process remained alive for the
whole experiment; the persisted lifecycle records were:

```text
READY -> WORKING (task_started 12:05:59.246Z)
       -> READY   (task_complete 12:06:02.131Z)
```

The HUD working set peaked at 83.50 MB during the turn and was 83.02 MB after
the turn. A separate tracked control session
`01a0238b-b1df-7dc1-a171-9b151ac38111` was `READY` before and after and emitted
no lifecycle change. A ten-second idle sample after the turn measured 0 ms of
additional HUD CPU time, confirming that the UI returned to sleep. The HUD
performed no session steering, approval, input, or other Codex-control action.
