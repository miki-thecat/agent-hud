//! Narrow internal extension contracts.
//!
//! These interfaces deliberately sit on the normalized application boundary.
//! An extension cannot receive raw Codex records through these contracts, and
//! the HUD does not need to know which provider produced a session.

use crate::model::{SessionChange, SessionViewModel};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionDescriptor {
    pub id: String,
    pub display_name: String,
}

impl ExtensionDescriptor {
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
        }
    }
}

/// A source of normalized session changes.
///
/// `next_event` may block until the provider has a meaningful event. This
/// keeps event-driven sources event-driven without imposing an async runtime
/// or a polling interval on the application.
pub trait SessionObserver {
    fn descriptor(&self) -> &ExtensionDescriptor;
    fn next_event(&mut self) -> Result<ObserverEvent, ObserverError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObserverEvent {
    Change(Box<SessionChange>),
    Disconnected,
    Terminated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverError {
    pub message: String,
    pub retryable: bool,
}

impl ObserverError {
    pub fn new(message: impl Into<String>, retryable: bool) -> Self {
        Self {
            message: message.into(),
            retryable,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationContribution {
    pub key: String,
    pub value: String,
    pub priority: u16,
}

impl PresentationContribution {
    pub fn new(key: impl Into<String>, value: impl Into<String>, priority: u16) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            priority,
        }
    }
}

/// Adds bounded, already-formatted information to a session row.
pub trait PresentationContributor {
    fn descriptor(&self) -> &ExtensionDescriptor;
    fn contribute(&self, session: &SessionViewModel) -> Option<PresentationContribution>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionDescriptor {
    pub id: String,
    pub display_name: String,
    pub requires_confirmation: bool,
}

impl ActionDescriptor {
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        requires_confirmation: bool,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            requires_confirmation,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionRequest {
    pub action_id: String,
    pub session_id: Option<String>,
}

impl ActionRequest {
    pub fn new(action_id: impl Into<String>, session_id: Option<String>) -> Self {
        Self {
            action_id: action_id.into(),
            session_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionOutcome {
    pub summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionError {
    pub message: String,
}

/// An explicitly invoked capability. Actions are not part of observation and
/// must not be called implicitly in response to an observer event.
pub trait ExplicitAction {
    fn descriptor(&self) -> &ActionDescriptor;
    fn invoke(&mut self, request: ActionRequest) -> Result<ActionOutcome, ActionError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::readiness::Readiness;

    struct FixtureObserver {
        descriptor: ExtensionDescriptor,
    }

    impl SessionObserver for FixtureObserver {
        fn descriptor(&self) -> &ExtensionDescriptor {
            &self.descriptor
        }
        fn next_event(&mut self) -> Result<ObserverEvent, ObserverError> {
            Ok(ObserverEvent::Disconnected)
        }
    }

    struct FixtureContributor {
        descriptor: ExtensionDescriptor,
    }

    impl PresentationContributor for FixtureContributor {
        fn descriptor(&self) -> &ExtensionDescriptor {
            &self.descriptor
        }
        fn contribute(&self, session: &SessionViewModel) -> Option<PresentationContribution> {
            Some(PresentationContribution::new(
                "readiness",
                session.readiness.as_str(),
                10,
            ))
        }
    }

    #[test]
    fn observer_contract_delivers_normalized_lifecycle_events() {
        let mut observer = FixtureObserver {
            descriptor: ExtensionDescriptor::new("fixture", "Fixture observer"),
        };
        assert_eq!(observer.descriptor().id, "fixture");
        assert_eq!(observer.next_event().unwrap(), ObserverEvent::Disconnected);
    }

    #[test]
    fn contributor_contract_consumes_normalized_session_state() {
        let contributor = FixtureContributor {
            descriptor: ExtensionDescriptor::new("fixture", "Fixture contributor"),
        };
        let session = SessionViewModel {
            id: "session-1".into(),
            title: None,
            project_identity: None,
            readiness: Readiness::Working,
            latest_result: None,
            needs_attention: false,
            recency_at_ms: 0,
            changed_files: Vec::new(),
            verification: None,
        };
        assert_eq!(
            contributor.contribute(&session),
            Some(PresentationContribution::new("readiness", "WORKING", 10))
        );
    }

    #[test]
    fn action_requests_are_explicit_and_session_scoped() {
        let descriptor = ActionDescriptor::new("open-session", "Open session", true);
        let request = ActionRequest::new("open-session", Some("session-1".into()));
        assert!(descriptor.requires_confirmation);
        assert_eq!(request.action_id, descriptor.id);
        assert_eq!(request.session_id.as_deref(), Some("session-1"));
    }
}
