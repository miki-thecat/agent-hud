//! Deterministic, in-memory queue state for future agent workflows.
//!
//! This module deliberately knows nothing about Codex readiness, dependency
//! resolution, priorities, scheduling, or agent execution. Dependency IDs
//! are retained only as opaque references for a later domain layer.

use std::fmt;

pub type QueueItemId = String;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueItemState {
    Queued,
    Active,
    Completed,
    Blocked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueueItem {
    pub id: QueueItemId,
    pub label: String,
    pub state: QueueItemState,
    pub dependency_ids: Vec<QueueItemId>,
}

impl QueueItem {
    pub fn new(id: impl Into<QueueItemId>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state: QueueItemState::Queued,
            dependency_ids: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Queue {
    items: Vec<QueueItem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueueError {
    DuplicateId(QueueItemId),
    NotFound(QueueItemId),
    PositionOutOfBounds {
        position: usize,
        len: usize,
    },
    IdMismatch {
        expected: QueueItemId,
        actual: QueueItemId,
    },
}

impl fmt::Display for QueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(formatter, "queue item ID already exists: {id}"),
            Self::NotFound(id) => write!(formatter, "queue item ID was not found: {id}"),
            Self::PositionOutOfBounds { position, len } => {
                write!(
                    formatter,
                    "queue position {position} is out of bounds for {len} items"
                )
            }
            Self::IdMismatch { expected, actual } => {
                write!(
                    formatter,
                    "queue update ID {actual} does not match {expected}"
                )
            }
        }
    }
}

impl std::error::Error for QueueError {}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    pub fn enqueue(&mut self, item: QueueItem) -> Result<(), QueueError> {
        if self.items.iter().any(|existing| existing.id == item.id) {
            return Err(QueueError::DuplicateId(item.id));
        }
        self.items.push(item);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&QueueItem> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn remove(&mut self, id: &str) -> Option<QueueItem> {
        let position = self.items.iter().position(|item| item.id == id)?;
        Some(self.items.remove(position))
    }

    pub fn reorder(&mut self, id: &str, position: usize) -> Result<(), QueueError> {
        if position >= self.items.len() {
            return Err(QueueError::PositionOutOfBounds {
                position,
                len: self.items.len(),
            });
        }
        let current = self
            .items
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| QueueError::NotFound(id.to_owned()))?;
        let item = self.items.remove(current);
        self.items.insert(position, item);
        Ok(())
    }

    pub fn update(&mut self, id: &str, item: QueueItem) -> Result<(), QueueError> {
        if item.id != id {
            return Err(QueueError::IdMismatch {
                expected: id.to_owned(),
                actual: item.id,
            });
        }
        let existing = self
            .items
            .iter_mut()
            .find(|existing| existing.id == id)
            .ok_or_else(|| QueueError::NotFound(id.to_owned()))?;
        *existing = item;
        Ok(())
    }

    pub fn update_state(&mut self, id: &str, state: QueueItemState) -> Result<(), QueueError> {
        let item = self
            .items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| QueueError::NotFound(id.to_owned()))?;
        item.state = state;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Queue, QueueError, QueueItem, QueueItemState};

    fn item(id: &str) -> QueueItem {
        QueueItem::new(id, format!("Task {id}"))
    }

    #[test]
    fn enqueue_preserves_explicit_order_and_lookup_is_deterministic() {
        let mut queue = Queue::new();
        queue.enqueue(item("a")).unwrap();
        queue.enqueue(item("b")).unwrap();
        queue.enqueue(item("c")).unwrap();

        assert_eq!(
            queue
                .items()
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        assert_eq!(
            queue.get("b").map(|item| item.label.as_str()),
            Some("Task b")
        );
    }

    #[test]
    fn duplicate_ids_are_rejected_without_changing_queue() {
        let mut queue = Queue::new();
        queue.enqueue(item("a")).unwrap();

        assert_eq!(
            queue.enqueue(item("a")),
            Err(QueueError::DuplicateId("a".into()))
        );
        assert_eq!(queue.items(), &[item("a")]);
    }

    #[test]
    fn reorder_moves_only_the_requested_item() {
        let mut queue = Queue::new();
        for id in ["a", "b", "c"] {
            queue.enqueue(item(id)).unwrap();
        }

        queue.reorder("c", 0).unwrap();
        assert_eq!(
            queue
                .items()
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["c", "a", "b"]
        );
    }

    #[test]
    fn remove_returns_item_and_closes_the_gap() {
        let mut queue = Queue::new();
        for id in ["a", "b", "c"] {
            queue.enqueue(item(id)).unwrap();
        }

        assert_eq!(queue.remove("b"), Some(item("b")));
        assert_eq!(
            queue
                .items()
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "c"]
        );
        assert_eq!(queue.remove("missing"), None);
    }

    #[test]
    fn update_changes_item_data_without_changing_membership_or_order() {
        let mut queue = Queue::new();
        queue.enqueue(item("a")).unwrap();
        queue.enqueue(item("b")).unwrap();
        let mut updated = item("a");
        updated.label = "Updated task".into();
        updated.dependency_ids = vec!["external-task".into()];

        queue.update("a", updated.clone()).unwrap();
        assert_eq!(queue.items(), &[updated, item("b")]);
    }

    #[test]
    fn state_updates_are_independent_from_readiness() {
        let mut queue = Queue::new();
        queue.enqueue(item("a")).unwrap();

        queue.update_state("a", QueueItemState::Blocked).unwrap();
        assert_eq!(queue.get("a").unwrap().state, QueueItemState::Blocked);
        assert_eq!(queue.get("a").unwrap().dependency_ids, Vec::<String>::new());
    }
}
