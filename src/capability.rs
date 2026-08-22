//! Model for the observer's supported capabilities.
//!
//! Capability availability describes what the observer can currently do. It
//! is deliberately not a session state and must not be used to infer
//! readiness, attention, verification, review, or process health.

use std::{collections::BTreeMap, fmt};

/// Stable identity for an observer capability.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CapabilityId(String);

impl CapabilityId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CapabilityId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Source of the evidence used to classify a capability.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EvidenceSource(String);

impl EvidenceSource {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for EvidenceSource {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for EvidenceSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Current observer support level for one capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityStatus {
    Supported,
    Degraded,
    Unavailable,
}

impl CapabilityStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "SUPPORTED",
            Self::Degraded => "DEGRADED",
            Self::Unavailable => "UNAVAILABLE",
        }
    }
}

/// A capability's status and the evidence explaining that status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverCapability {
    pub id: CapabilityId,
    pub status: CapabilityStatus,
    pub evidence_source: EvidenceSource,
    pub reason: Option<String>,
}

impl ObserverCapability {
    pub fn new(
        id: impl Into<CapabilityId>,
        status: CapabilityStatus,
        evidence_source: impl Into<EvidenceSource>,
        reason: Option<impl Into<String>>,
    ) -> Self {
        Self {
            id: id.into(),
            status,
            evidence_source: evidence_source.into(),
            reason: reason.map(Into::into),
        }
    }
}

/// Deterministic in-memory registry of observer capabilities.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ObserverCapabilityRegistry {
    capabilities: BTreeMap<CapabilityId, ObserverCapability>,
}

impl ObserverCapabilityRegistry {
    pub fn register(&mut self, capability: ObserverCapability) -> Option<ObserverCapability> {
        self.capabilities.insert(capability.id.clone(), capability)
    }

    pub fn get(&self, id: &CapabilityId) -> Option<&ObserverCapability> {
        self.capabilities.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ObserverCapability> {
        self.capabilities.values()
    }

    pub fn len(&self) -> usize {
        self.capabilities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capability(id: &str, status: CapabilityStatus, reason: Option<&str>) -> ObserverCapability {
        ObserverCapability::new(id, status, "protocol fixture", reason)
    }

    #[test]
    fn represents_all_three_support_levels_and_reason() {
        let mut registry = ObserverCapabilityRegistry::default();
        registry.register(capability("sessions", CapabilityStatus::Supported, None));
        registry.register(capability(
            "activity",
            CapabilityStatus::Degraded,
            Some("activity events are coarse"),
        ));
        registry.register(capability(
            "approvals",
            CapabilityStatus::Unavailable,
            Some("passive observation is not exposed"),
        ));

        assert_eq!(registry.len(), 3);
        assert_eq!(
            registry
                .get(&CapabilityId::from("sessions"))
                .unwrap()
                .status,
            CapabilityStatus::Supported
        );
        assert_eq!(
            registry
                .get(&CapabilityId::from("activity"))
                .unwrap()
                .reason
                .as_deref(),
            Some("activity events are coarse")
        );
        assert_eq!(
            registry
                .get(&CapabilityId::from("approvals"))
                .unwrap()
                .status,
            CapabilityStatus::Unavailable
        );
    }

    #[test]
    fn replacement_is_scoped_to_the_same_capability_identity() {
        let mut registry = ObserverCapabilityRegistry::default();
        assert!(
            registry
                .register(capability(
                    "activity",
                    CapabilityStatus::Unavailable,
                    Some("offline")
                ))
                .is_none()
        );
        let previous = registry
            .register(capability("activity", CapabilityStatus::Supported, None))
            .unwrap();

        assert_eq!(previous.status, CapabilityStatus::Unavailable);
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry
                .get(&CapabilityId::from("activity"))
                .unwrap()
                .status,
            CapabilityStatus::Supported
        );
    }

    #[test]
    fn iteration_is_deterministically_sorted_by_capability_identity() {
        let mut registry = ObserverCapabilityRegistry::default();
        registry.register(capability("z-last", CapabilityStatus::Supported, None));
        registry.register(capability("a-first", CapabilityStatus::Supported, None));

        let ids: Vec<_> = registry
            .iter()
            .map(|capability| capability.id.as_str())
            .collect();
        assert_eq!(ids, ["a-first", "z-last"]);
    }

    #[test]
    fn capability_model_has_no_session_state_fields() {
        let capability = capability("readiness", CapabilityStatus::Degraded, Some("fixture"));

        assert_eq!(capability.evidence_source.as_str(), "protocol fixture");
        assert_eq!(capability.status.as_str(), "DEGRADED");
        assert_eq!(capability.reason.as_deref(), Some("fixture"));
    }
}
