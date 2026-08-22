//! Provider-neutral review workflow state.
//!
//! Review state is informational and deliberately separate from Codex
//! readiness, attention, verification, and process health. This module does
//! not perform review actions or contact an external review provider.

/// The current outcome of a review workflow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewState {
    Pending,
    Approved,
    ChangesRequested,
}

impl ReviewState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Approved => "APPROVED",
            Self::ChangesRequested => "CHANGES_REQUESTED",
        }
    }
}

/// Identifies the person or system that supplied a review.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reviewer {
    pub display_name: String,
    pub id: Option<String>,
}

impl Reviewer {
    pub fn named(display_name: impl Into<String>) -> Self {
        Self {
            display_name: display_name.into(),
            id: None,
        }
    }

    pub fn with_id(display_name: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            display_name: display_name.into(),
            id: Some(id.into()),
        }
    }
}

/// Identifies the source that owns or reported the review workflow.
///
/// `name` is intentionally provider-neutral: it may describe a local tool,
/// a hosted code-review service, a human process, or another future source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewSource {
    pub name: String,
    pub reference: Option<String>,
}

impl ReviewSource {
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            reference: None,
        }
    }

    pub fn with_reference(name: impl Into<String>, reference: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            reference: Some(reference.into()),
        }
    }
}

/// A complete informational review snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewSnapshot {
    pub state: ReviewState,
    pub reviewer: Option<Reviewer>,
    pub source: Option<ReviewSource>,
}

impl ReviewSnapshot {
    pub const fn pending() -> Self {
        Self {
            state: ReviewState::Pending,
            reviewer: None,
            source: None,
        }
    }
}

/// A deterministic update to the review snapshot.
///
/// Updates replace the complete snapshot. Keeping the update atomic prevents
/// stale reviewer/source metadata from being accidentally combined with a
/// newer state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewUpdate {
    pub state: ReviewState,
    pub reviewer: Option<Reviewer>,
    pub source: Option<ReviewSource>,
}

impl From<ReviewSnapshot> for ReviewUpdate {
    fn from(snapshot: ReviewSnapshot) -> Self {
        Self {
            state: snapshot.state,
            reviewer: snapshot.reviewer,
            source: snapshot.source,
        }
    }
}

/// In-memory review workflow state for one work item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewWorkflow {
    snapshot: ReviewSnapshot,
}

impl Default for ReviewWorkflow {
    fn default() -> Self {
        Self {
            snapshot: ReviewSnapshot::pending(),
        }
    }
}

impl ReviewWorkflow {
    pub fn new(snapshot: ReviewSnapshot) -> Self {
        Self { snapshot }
    }

    pub const fn snapshot(&self) -> &ReviewSnapshot {
        &self.snapshot
    }

    /// Replaces the current review snapshot and reports whether it changed.
    pub fn apply(&mut self, update: impl Into<ReviewUpdate>) -> bool {
        let update = update.into();
        let next = ReviewSnapshot {
            state: update.state,
            reviewer: update.reviewer,
            source: update.source,
        };
        if self.snapshot == next {
            false
        } else {
            self.snapshot = next;
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{readiness::Readiness, verification::VerificationOutcome};

    use super::{
        ReviewSnapshot, ReviewSource, ReviewState, ReviewUpdate, ReviewWorkflow, Reviewer,
    };

    #[test]
    fn represents_all_review_states_with_stable_labels() {
        assert_eq!(ReviewState::Pending.as_str(), "PENDING");
        assert_eq!(ReviewState::Approved.as_str(), "APPROVED");
        assert_eq!(ReviewState::ChangesRequested.as_str(), "CHANGES_REQUESTED");
    }

    #[test]
    fn preserves_provider_neutral_reviewer_and_source_metadata() {
        let snapshot = ReviewSnapshot {
            state: ReviewState::Approved,
            reviewer: Some(Reviewer::with_id("Ada", "reviewer-7")),
            source: Some(ReviewSource::with_reference(
                "local-review-tool",
                "review-42",
            )),
        };

        assert_eq!(snapshot.reviewer.as_ref().unwrap().display_name, "Ada");
        assert_eq!(
            snapshot.reviewer.as_ref().unwrap().id.as_deref(),
            Some("reviewer-7")
        );
        assert_eq!(snapshot.source.as_ref().unwrap().name, "local-review-tool");
        assert_eq!(
            snapshot.source.as_ref().unwrap().reference.as_deref(),
            Some("review-42")
        );
    }

    #[test]
    fn updates_replace_state_and_metadata_atomically() {
        let mut workflow = ReviewWorkflow::default();
        assert!(workflow.apply(ReviewUpdate {
            state: ReviewState::ChangesRequested,
            reviewer: Some(Reviewer::named("Reviewer one")),
            source: Some(ReviewSource::named("service one")),
        }));

        assert!(workflow.apply(ReviewUpdate {
            state: ReviewState::Approved,
            reviewer: Some(Reviewer::named("Reviewer two")),
            source: Some(ReviewSource::named("service two")),
        }));
        assert_eq!(workflow.snapshot().state, ReviewState::Approved);
        assert_eq!(
            workflow.snapshot().reviewer.as_ref().unwrap().display_name,
            "Reviewer two"
        );
        assert_eq!(
            workflow.snapshot().source.as_ref().unwrap().name,
            "service two"
        );
    }

    #[test]
    fn identical_update_is_a_no_op() {
        let snapshot = ReviewSnapshot::pending();
        let mut workflow = ReviewWorkflow::new(snapshot.clone());
        assert!(!workflow.apply(snapshot.clone()));
        assert_eq!(workflow.snapshot(), &snapshot);
    }

    #[test]
    fn review_snapshot_is_independent_from_readiness_and_verification() {
        let readiness = Readiness::Working;
        let verification = VerificationOutcome::Failed;
        let mut workflow = ReviewWorkflow::default();
        workflow.apply(ReviewUpdate {
            state: ReviewState::Approved,
            reviewer: None,
            source: None,
        });

        assert_eq!(workflow.snapshot().state, ReviewState::Approved);
        assert_eq!(readiness, Readiness::Working);
        assert_eq!(verification, VerificationOutcome::Failed);
    }
}
