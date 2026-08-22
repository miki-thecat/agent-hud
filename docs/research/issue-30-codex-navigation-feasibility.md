# Issue #30 — Codex session navigation feasibility

**Status:** research-only; no product-code or Codex-control changes

**Research date:** 2026-08-22

## Executive conclusion

The safest viable route is the documented Codex deep link:

```text
codex://threads/<thread-id>
```

The current official command reference documents this link as opening an
existing local chat, and the installed Windows package on this machine
(`OpenAI.Codex`, version `26.818.3698.0`) declares the `codex` Windows protocol
handler. The existing persisted-session discovery work also establishes a
validated root-session ID that can supply `<thread-id>`.

This is sufficient to recommend a later, user-initiated **best-effort open
thread** action. It is not sufficient to claim that agent-hud can guarantee a
specific Codex window, foreground focus, exact Desktop ownership, or successful
navigation on every installation. Those behaviors are controlled by Codex and
Windows and require a separate controlled runtime test before implementation.

UI Automation is technically available as a fallback, but should not be the
primary route. It would couple agent-hud to Codex's rendered controls, require
fragile selectors and focus handling, and cross from observation into driving
another application's UI. It is therefore **experimental/fragile**, not an MVP
dependency.

## Evidence and scope

This report covers:

- official Codex deep-link and command documentation;
- Windows protocol registration and URI launch semantics;
- mapping the HUD's persisted row identity to a Codex thread ID;
- Windows UI Automation as a separately evaluated fallback;
- a concrete recommendation and claim ceiling.

No Codex session was opened, focused, steered, interrupted, approved, or sent
input during this research pass. No UI Automation implementation, private IPC
reverse engineering, or `src/` change was performed.

## Findings

| Path | Classification | Evidence | Boundary |
| --- | --- | --- | --- |
| `codex://threads/<thread-id>` | **SUPPORTED** for navigation intent | The official Codex command reference lists it as opening a local chat and defines `<thread-id>` as the technical thread ID. | The documentation does not guarantee foreground focus, a particular window, or behavior for a missing/archived thread. |
| Windows `codex` protocol activation | **SUPPORTED** on the observed installation | `AppxManifest.xml` for package `OpenAI.Codex_26.818.3698.0` declares `<uap:Protocol Name="codex" />`. Microsoft documents protocol activation for packaged Win32 apps and URI launching. | Registration proves the OS route exists; it does not prove every invocation path or Codex-side routing outcome. |
| HUD row ID → deep-link ID | **SUPPORTED** for validated persisted root sessions | Phase 1C validates `threads.id` against rollout `session_meta.id` and `session_id` for root/user sessions. | The catalog is a bounded **Recent local sessions** set, not proof that a chat is currently open or owned by a particular Desktop window. |
| Windows URI launch API / shell launch | **SUPPORTED** as the platform mechanism | Microsoft documents `LaunchUriAsync` and custom URI schemes. Launch should be tied to an explicit user action. | Windows may prompt, route to the registered handler, or fail if registration is unavailable; the caller cannot select the target app in general. |
| Explicit window selection or guaranteed focus | **UNAVAILABLE** in the reviewed public contract | The Codex deep-link reference documents the thread target but no window-targeting parameter. | Do not promise “focus this exact Codex window” or “open in a new window.” |
| Windows UI Automation | **EXPERIMENTAL/FRAGILE** fallback | Microsoft exposes an out-of-process UI Automation tree and invoke patterns for controls. | Requires stable accessible names/structure, foreground/window coordination, and matching integrity levels; UI changes can break it. It is application driving, not passive observation. |
| Private IPC/app-server attachment | **UNAVAILABLE / out of scope** | Existing Phase-0 evidence found no supported native-Windows second-client attachment path to the Desktop-owned app-server. | Do not add a private navigation channel or infer one from reverse engineering. |

### Codex deep links

The current official reference says the desktop app keeps the `codex://` scheme
for compatibility and lists:

- `codex://threads/<thread-id>` — open an existing local chat;
- `codex://threads/new` and `codex://new?...` — create a new local chat;
- query values must be encoded before being placed in a URI.

Only the existing-thread form is relevant here. The new-thread forms are not a
fallback for opening a known session: using them would create a new chat and
could change user-visible state, which is outside this issue's navigation goal.

### Windows activation

The observed package manifest contains a protocol extension for `codex`. This
is stronger evidence than assuming a file association or guessing an executable
path. Microsoft documents two relevant facts:

1. packaged Win32 apps can register a custom URI scheme and receive protocol
   activation;
2. a caller can launch a URI through the Windows launcher, but the operating
   system/user chooses the registered handler and launch success is not the same
   as a guaranteed foreground/window result.

An earlier public Codex Windows issue reported that deep links were not handled
on an older installation. That report is useful operational evidence that
registration and runtime behavior have varied by version; it is not evidence
against the current package manifest. A later implementation must verify the
installed handler at runtime and provide a no-op/error path rather than treating
URI construction as proof of success.

### Session identity mapping

The deep-link ID must be the validated technical thread UUID, not the entire
rollout filename. The existing Phase 1C result provides the safe boundary:

1. select an unarchived `threads` row with `thread_source=user`;
2. validate its `id` against the rollout path and `session_meta.id` /
   `session_id`;
3. exclude subagent rows and ambiguous identity chains;
4. use the validated root `threads.id` in `codex://threads/<id>`.

This supports a navigation target for a persisted row. It does not establish
that the row is open, active, visible, or currently assigned to a particular
Codex Desktop window.

### UI Automation fallback

Microsoft UI Automation is a real Windows accessibility/automation API. An
out-of-process client can inspect the desktop tree and invoke controls when the
target application exposes suitable providers. That makes it technically
possible to attempt a fallback such as:

1. find the Codex window;
2. locate a sidebar/thread control by accessible properties;
3. invoke the control or search UI;
4. select the matching thread.

The approach has a poor reliability boundary for this product:

- it depends on Codex's implementation details rather than a Codex navigation
  contract;
- accessible names, tree shape, virtualization, and selection behavior can
  change without a stable API promise;
- multiple windows, minimized/background windows, focus stealing, and timing
  races make “focus this session” difficult to prove;
- elevation/integrity-level differences can block interaction with protected
  UI;
- it would turn a read-only HUD into an application driver and broaden the
  security and maintenance surface.

UI Automation could be reconsidered only as an explicitly opt-in, degraded
fallback after the supported deep-link route is implemented and measured. It
must never silently claim success when it cannot identify and invoke the exact
target.

## Recommendation

Defer implementation in this issue. For a later implementation, accept only
the following narrow design:

1. Make the row action an explicit user gesture.
2. Construct `codex://threads/<validated-root-thread-id>` from the existing
   identity chain.
3. Invoke the Windows URI handler using the native Windows mechanism available
   to the final packaged/unpackaged app model.
4. Treat the result as a request to Codex, not confirmation that the desired
   window is focused.
5. Keep the action disabled or unavailable when identity validation fails.
6. Add a controlled compatibility test across the supported Codex Windows
   package versions before calling the behavior production-ready.

Do not implement UI Automation, private IPC, app-server attachment, or new
thread creation as part of this navigation work.

## Claim ceiling

With the evidence currently available, agent-hud may eventually claim:

> “For a validated recent local session, agent-hud can ask the installed Codex
> Desktop handler to open the corresponding local chat through the documented
> `codex://threads/<thread-id>` link.”

It may not claim:

- the session is currently open or live;
- the link always succeeds on every Codex Windows version;
- a particular Codex window is selected;
- the target window is foreground/focused after launch;
- the action is passive observation; it is a user-requested navigation action;
- UI Automation is a stable or supported Codex integration.

## Sources

- OpenAI, [Codex app commands — Deep links](https://developers.openai.com/codex/app/commands#deeplinks) (current reference redirects to ChatGPT Learn; accessed 2026-08-22).
- Microsoft, [Handle URI activation with a Windows app](https://learn.microsoft.com/en-us/windows/apps/develop/launch/handle-uri-activation).
- Microsoft, [Launch the default Windows app for a URI](https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-default-app).
- Microsoft, [UI Automation overview](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-uiautomationoverview).
- Microsoft, [UI Automation security overview](https://learn.microsoft.com/en-us/dotnet/framework/ui-automation/ui-automation-security-overview).
- OpenAI Codex, [Deeplink Not Handled on Windows #14686](https://github.com/openai/codex/issues/14686) (older-version compatibility evidence; accessed 2026-08-22).
- Repository, [`docs/research/phase-1c-session-discovery-result.md`](phase-1c-session-discovery-result.md).
- Repository, [`docs/research/windows-codex-observation-2026-08-20.md`](windows-codex-observation-2026-08-20.md).
