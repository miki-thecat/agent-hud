//! A local, presentation-independent review workflow model.
//!
//! Review state describes an external review decision only.  It is deliberately
//! not derived from readiness, attention, verification, or observation health.
//! This module contains no GitHub client and performs no review or merge action.

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReviewStatus {
    Pending,
    Approved,
    ChangesRequested,
}

impl ReviewStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Approved => "APPROVED",
            Self::ChangesRequested => "CHANGES_REQUESTED",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewMetadata {
    /// The person or system that supplied the review decision, when known.
    pub reviewer: Option<String>,
    /// The local/source identifier from which the review was recorded.
    pub source: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewState {
    pub status: ReviewStatus,
    pub metadata: ReviewMetadata,
}

impl ReviewState {
    pub fn pending(source: impl Into<String>) -> Self {
        Self::new(ReviewStatus::Pending, None, source)
    }

    pub fn approved(reviewer: impl Into<String>, source: impl Into<String>) -> Self {
        Self::new(ReviewStatus::Approved, Some(reviewer.into()), source)
    }

    pub fn changes_requested(reviewer: impl Into<String>, source: impl Into<String>) -> Self {
        Self::new(
            ReviewStatus::ChangesRequested,
            Some(reviewer.into()),
            source,
        )
    }

    pub fn new(status: ReviewStatus, reviewer: Option<String>, source: impl Into<String>) -> Self {
        Self {
            status,
            metadata: ReviewMetadata {
                reviewer,
                source: source.into(),
            },
        }
    }
}

/// Holds the current review decision for a local workflow.
///
/// Updating this value is intentionally an in-memory model operation.  It does
/// not submit a review, call GitHub, or merge anything automatically.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewWorkflow {
    current: ReviewState,
}

impl ReviewWorkflow {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            current: ReviewState::pending(source),
        }
    }

    pub fn current(&self) -> &ReviewState {
        &self.current
    }

    pub fn replace(&mut self, state: ReviewState) -> bool {
        if self.current == state {
            return false;
        }
        self.current = state;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{ReviewState, ReviewStatus, ReviewWorkflow};

    #[test]
    fn statuses_have_stable_labels() {
        assert_eq!(ReviewStatus::Pending.as_str(), "PENDING");
        assert_eq!(ReviewStatus::Approved.as_str(), "APPROVED");
        assert_eq!(ReviewStatus::ChangesRequested.as_str(), "CHANGES_REQUESTED");
    }

    #[test]
    fn states_preserve_reviewer_and_source_metadata() {
        assert_eq!(
            ReviewState::approved("alice", "local-fixture"),
            ReviewState {
                status: ReviewStatus::Approved,
                metadata: super::ReviewMetadata {
                    reviewer: Some("alice".into()),
                    source: "local-fixture".into(),
                },
            }
        );
        assert_eq!(
            ReviewState::pending("local-fixture").metadata.reviewer,
            None
        );
    }

    #[test]
    fn workflow_starts_pending_and_replaces_only_on_change() {
        let mut workflow = ReviewWorkflow::new("fixture");
        assert_eq!(workflow.current().status, ReviewStatus::Pending);
        assert!(!workflow.replace(ReviewState::pending("fixture")));
        assert!(workflow.replace(ReviewState::changes_requested("reviewer", "fixture")));
        assert_eq!(
            workflow.current().metadata.reviewer.as_deref(),
            Some("reviewer")
        );
        assert!(workflow.replace(ReviewState::approved("reviewer", "fixture")));
    }
}
