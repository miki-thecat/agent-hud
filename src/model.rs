use crate::{discovery::SessionSnapshot, readiness::Readiness};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionViewModel {
    pub id: String,
    pub title: Option<String>,
    pub readiness: Readiness,
    pub needs_attention: bool,
    pub recency_at_ms: i64,
}

impl From<&SessionSnapshot> for SessionViewModel {
    fn from(snapshot: &SessionSnapshot) -> Self {
        Self {
            id: snapshot.id.clone(),
            title: snapshot.title.clone(),
            readiness: snapshot.readiness,
            needs_attention: false,
            recency_at_ms: snapshot.recency_at_ms,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionChange {
    Snapshot(Vec<SessionViewModel>),
    Updated(SessionViewModel),
    Removed(String),
    ObservationDegraded { id: String },
    ObservationTerminated,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ApplicationState {
    sessions: Vec<SessionViewModel>,
    pub observation_degraded: bool,
}

impl ApplicationState {
    pub fn sessions(&self) -> &[SessionViewModel] {
        &self.sessions
    }

    pub fn apply(&mut self, change: SessionChange) -> bool {
        match change {
            SessionChange::Snapshot(mut items) => {
                for item in &mut items {
                    if let Some(previous) = self.sessions.iter().find(|old| old.id == item.id) {
                        item.needs_attention = if previous.readiness == Readiness::Working
                            && item.readiness == Readiness::Ready
                        {
                            true
                        } else if item.readiness != Readiness::Ready {
                            false
                        } else {
                            previous.needs_attention
                        };
                    }
                }
                items.sort_by(session_ordering);
                self.sessions = items;
                true
            }
            SessionChange::Updated(item) => {
                if let Some(existing) = self
                    .sessions
                    .iter_mut()
                    .find(|existing| existing.id == item.id)
                {
                    let was_working = existing.readiness == Readiness::Working;
                    *existing = item;
                    existing.needs_attention =
                        if was_working && existing.readiness == Readiness::Ready {
                            true
                        } else if existing.readiness != Readiness::Ready {
                            false
                        } else {
                            existing.needs_attention
                        };
                } else {
                    self.sessions.push(item);
                }
                self.sessions.sort_by(session_ordering);
                true
            }
            SessionChange::Removed(id) => {
                let length = self.sessions.len();
                self.sessions.retain(|item| item.id != id);
                self.sessions.len() != length
            }
            SessionChange::ObservationDegraded { id } => {
                if let Some(item) = self.sessions.iter_mut().find(|item| item.id == id) {
                    item.readiness = Readiness::Unknown;
                    item.needs_attention = false;
                    self.observation_degraded = true;
                    true
                } else {
                    false
                }
            }
            SessionChange::ObservationTerminated => {
                let degraded_changed = !self.observation_degraded;
                self.observation_degraded = true;
                let mut changed = false;
                for item in &mut self.sessions {
                    if item.readiness != Readiness::Unknown {
                        item.readiness = Readiness::Unknown;
                        item.needs_attention = false;
                        changed = true;
                    }
                }
                degraded_changed || changed || self.sessions.is_empty()
            }
        }
    }

    pub fn acknowledge(&mut self, id: &str) -> bool {
        if let Some(item) = self
            .sessions
            .iter_mut()
            .find(|item| item.id == id && item.needs_attention)
        {
            item.needs_attention = false;
            true
        } else {
            false
        }
    }
}

fn session_ordering(left: &SessionViewModel, right: &SessionViewModel) -> std::cmp::Ordering {
    right
        .recency_at_ms
        .cmp(&left.recency_at_ms)
        .then_with(|| left.id.cmp(&right.id))
}

#[cfg(test)]
mod tests {
    use super::{ApplicationState, SessionChange, SessionViewModel};
    use crate::readiness::Readiness;

    fn session(id: &str, readiness: Readiness, recency_at_ms: i64) -> SessionViewModel {
        SessionViewModel {
            id: id.into(),
            title: None,
            readiness,
            needs_attention: false,
            recency_at_ms,
        }
    }

    #[test]
    fn initial_snapshot_is_ordered_by_recency_then_id() {
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![
            session("b", Readiness::Ready, 10),
            session("a", Readiness::Ready, 20),
            session("c", Readiness::Ready, 20),
        ]));
        assert_eq!(
            state
                .sessions()
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "c", "b"]
        );
    }

    #[test]
    fn targeted_update_changes_only_one_row_and_preserves_order() {
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![
            session("a", Readiness::Ready, 20),
            session("b", Readiness::Working, 10),
        ]));
        state.apply(SessionChange::Updated(session("b", Readiness::Ready, 10)));
        assert_eq!(state.sessions()[0].readiness, Readiness::Ready);
        assert_eq!(state.sessions()[1].id, "b");
    }

    #[test]
    fn add_and_remove_update_the_visible_set() {
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![session(
            "a",
            Readiness::Ready,
            1,
        )]));
        state.apply(SessionChange::Updated(session("b", Readiness::Working, 2)));
        assert_eq!(
            state
                .sessions()
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["b", "a"]
        );
        assert!(state.apply(SessionChange::Removed("b".into())));
        assert_eq!(
            state
                .sessions()
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a"]
        );
    }

    #[test]
    fn observation_degradation_is_unknown_and_visible() {
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![session(
            "a",
            Readiness::Working,
            1,
        )]));
        state.apply(SessionChange::ObservationTerminated);
        assert_eq!(state.sessions()[0].readiness, Readiness::Unknown);
        assert!(state.observation_degraded);
    }

    #[test]
    fn observation_termination_repaints_when_rows_are_already_unknown() {
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![session(
            "a",
            Readiness::Unknown,
            1,
        )]));

        assert!(state.apply(SessionChange::ObservationTerminated));
        assert!(state.observation_degraded);
        assert_eq!(state.sessions()[0].readiness, Readiness::Unknown);
    }

    #[test]
    fn initial_ready_session_is_not_attention_ready() {
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![session(
            "a",
            Readiness::Ready,
            1,
        )]));
        assert!(!state.sessions()[0].needs_attention);
    }

    #[test]
    fn working_to_ready_marks_attention_and_acknowledgement_preserves_ready() {
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![session(
            "a",
            Readiness::Working,
            1,
        )]));
        state.apply(SessionChange::Updated(session("a", Readiness::Ready, 1)));
        assert!(state.sessions()[0].needs_attention);
        assert!(state.acknowledge("a"));
        assert_eq!(state.sessions()[0].readiness, Readiness::Ready);
        assert!(!state.sessions()[0].needs_attention);
    }

    #[test]
    fn repeated_completed_turn_marks_attention_again() {
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![session(
            "a",
            Readiness::Working,
            1,
        )]));
        state.apply(SessionChange::Updated(session("a", Readiness::Ready, 1)));
        state.acknowledge("a");
        state.apply(SessionChange::Updated(session("a", Readiness::Working, 1)));
        state.apply(SessionChange::Updated(session("a", Readiness::Ready, 1)));
        assert!(state.sessions()[0].needs_attention);
    }

    #[test]
    fn non_ready_transition_clears_stale_attention_and_ack_is_per_session() {
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![
            session("a", Readiness::Working, 2),
            session("b", Readiness::Working, 1),
        ]));
        state.apply(SessionChange::Updated(session("a", Readiness::Ready, 2)));
        state.apply(SessionChange::Updated(session("b", Readiness::Ready, 1)));
        assert!(state.acknowledge("a"));
        state.apply(SessionChange::Updated(session("b", Readiness::Unknown, 1)));
        assert!(
            !state
                .sessions()
                .iter()
                .find(|item| item.id == "a")
                .unwrap()
                .needs_attention
        );
        assert!(
            !state
                .sessions()
                .iter()
                .find(|item| item.id == "b")
                .unwrap()
                .needs_attention
        );
    }
}
