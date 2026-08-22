# Internal Extension Points

Issue #70 establishes the smallest contracts needed for future variation
without turning `agent-hud` into a plugin host.

## Contract boundary

```text
provider adapter -> SessionObserver -> SessionChange -> reducer/UI
                                      SessionViewModel -> PresentationContributor -> UI
user-selected intent -> ActionRequest -> ExplicitAction
```

The observer and contributor contracts use normalized application types only.
Raw Codex JSON, process handles, filesystem records, and provider-specific
types stay inside an adapter. Presentation contributors are read-only. An
action is a separate capability and is never inferred from an observer event.

## Contracts

### Session observers

`SessionObserver::next_event` is an event boundary, not a polling API. An
implementation may block until a meaningful change is available. It returns
normalized `SessionChange` values or an explicit disconnected/terminated
event. Retry policy and provider reconnection remain outside this contract so
the reducer can fail closed when freshness is no longer defensible.

### Presentation contributors

`PresentationContributor::contribute` receives one `SessionViewModel` and may
return one `PresentationContribution` containing a key, display value, and
priority. The application remains responsible for layout, truncation, and
ordering. Contributors do not receive a drawing surface and cannot request
repaints.

### Explicit actions

`ExplicitAction` is a capability declaration, not an instruction to add
interactive behavior now. `ActionDescriptor::requires_confirmation` makes
confirmation policy visible at the boundary, while `ActionRequest` requires a
caller to name the action and, when applicable, the target session. The MVP
does not register an action implementation. Future actions must be reviewed
against the read-only/non-interference requirements before wiring them in.

## Deliberate non-goals

- dynamic DLL or shared-library loading;
- a scripting or embedded runtime;
- marketplace/discovery/registration infrastructure;
- a generic event bus or dependency-injection container;
- raw provider access from the UI;
- implicit actions, approvals, steering, or orchestration.

The contracts are intentionally ordinary Rust traits. If a future requirement
needs a runtime boundary, it must first demonstrate why compile-time wiring is
insufficient and document the lifecycle and performance cost.
