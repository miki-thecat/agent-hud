//! Product-facing items that need human attention.
//!
//! Attention is deliberately independent from recorded session readiness. A
//! session can be `WORKING`, `READY`, or `UNKNOWN` while still having zero or
//! more attention items.

use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AttentionCategory {
    Failure,
    ReviewRequired,
    MergeReady,
    UserActionRequired,
}

impl AttentionCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Failure => "FAILURE",
            Self::ReviewRequired => "REVIEW_REQUIRED",
            Self::MergeReady => "MERGE_READY",
            Self::UserActionRequired => "USER_ACTION_REQUIRED",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttentionItem {
    pub id: String,
    pub session_id: String,
    pub category: AttentionCategory,
    pub title: String,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AttentionCenter {
    items: Vec<AttentionItem>,
}

impl AttentionCenter {
    pub fn items(&self) -> &[AttentionItem] {
        &self.items
    }

    /// Replace the observed set and keep presentation order deterministic.
    pub fn replace(&mut self, mut items: Vec<AttentionItem>) -> bool {
        items.sort_by(attention_ordering);
        if self.items == items {
            return false;
        }
        self.items = items;
        true
    }

    pub fn remove_session(&mut self, session_id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|item| item.session_id != session_id);
        before != self.items.len()
    }
}

fn attention_ordering(left: &AttentionItem, right: &AttentionItem) -> Ordering {
    left.category
        .cmp(&right.category)
        .then_with(|| left.session_id.cmp(&right.session_id))
        .then_with(|| left.id.cmp(&right.id))
}

#[cfg(test)]
mod tests {
    use super::{AttentionCategory, AttentionCenter, AttentionItem};

    fn item(id: &str, session_id: &str, category: AttentionCategory) -> AttentionItem {
        AttentionItem {
            id: id.into(),
            session_id: session_id.into(),
            category,
            title: id.into(),
            detail: None,
        }
    }

    #[test]
    fn categories_have_stable_labels() {
        assert_eq!(AttentionCategory::Failure.as_str(), "FAILURE");
        assert_eq!(
            AttentionCategory::UserActionRequired.as_str(),
            "USER_ACTION_REQUIRED"
        );
    }

    #[test]
    fn replace_is_sorted_and_reports_only_real_changes() {
        let mut center = AttentionCenter::default();
        let items = vec![
            item("merge", "b", AttentionCategory::MergeReady),
            item("failure", "a", AttentionCategory::Failure),
        ];
        assert!(center.replace(items.clone()));
        assert_eq!(center.items()[0].id, "failure");
        assert!(!center.replace(items));
    }

    #[test]
    fn removing_a_session_does_not_remove_other_attention() {
        let mut center = AttentionCenter::default();
        center.replace(vec![
            item("a", "session-a", AttentionCategory::Failure),
            item("b", "session-b", AttentionCategory::ReviewRequired),
        ]);

        assert!(center.remove_session("session-a"));
        assert_eq!(
            center
                .items()
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
        assert!(!center.remove_session("missing"));
    }
}
