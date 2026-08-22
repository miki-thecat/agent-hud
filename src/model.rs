use crate::verification::VerificationEvidence;
use crate::{
    attention::{AttentionCenter, AttentionItem},
    discovery::SessionSnapshot,
    project::ProjectIdentity,
    readiness::Readiness,
};

pub const WORKFLOW_EVENT_LIMIT: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkflowEventKind {
    TaskStarted,
    TaskCompleted,
    AssistantResult,
    FileChange,
    CommandExecution,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowEvent {
    pub sequence: u64,
    pub timestamp: Option<String>,
    pub kind: WorkflowEventKind,
    pub summary: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionViewModel {
    pub id: String,
    pub title: Option<String>,
    pub project_identity: Option<ProjectIdentity>,
    pub readiness: Readiness,
    pub latest_result: Option<String>,
    pub needs_attention: bool,
    pub recency_at_ms: i64,
    pub changed_files: Vec<String>,
    pub verification: Option<VerificationEvidence>,
}

impl From<&SessionSnapshot> for SessionViewModel {
    fn from(snapshot: &SessionSnapshot) -> Self {
        Self {
            id: snapshot.id.clone(),
            title: snapshot.title.clone(),
            project_identity: snapshot.project_identity.clone(),
            readiness: snapshot.readiness,
            latest_result: snapshot.latest_result.clone(),
            needs_attention: false,
            recency_at_ms: snapshot.recency_at_ms,
            changed_files: snapshot.changed_files.clone(),
            verification: snapshot.verification.clone(),
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
    attention: AttentionCenter,
}

impl ApplicationState {
    #[allow(dead_code)]
    pub fn sessions(&self) -> &[SessionViewModel] {
        &self.sessions
    }

    /// Returns the presentation subset for the selected project.
    ///
    /// The underlying session collection is never changed by filtering. A
    /// missing filter keeps every session visible; a selected identity keeps
    /// only sessions with the same complete project identity.
    pub fn sessions_for_project(
        &self,
        project: Option<&ProjectIdentity>,
    ) -> Vec<&SessionViewModel> {
        self.sessions
            .iter()
            .filter(|session| {
                project.is_none_or(|project| session.project_identity.as_ref() == Some(project))
            })
            .collect()
    }

    #[allow(dead_code)]
    pub fn attention(&self) -> &AttentionCenter {
        &self.attention
    }

    #[allow(dead_code)]
    pub fn replace_attention(&mut self, items: Vec<AttentionItem>) -> bool {
        self.attention.replace(items)
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
                    let was_attention = existing.needs_attention;
                    *existing = item;
                    existing.needs_attention =
                        if was_working && existing.readiness == Readiness::Ready {
                            true
                        } else if existing.readiness != Readiness::Ready {
                            false
                        } else {
                            was_attention
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
                self.attention.remove_session(&id) || self.sessions.len() != length
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
    use crate::{
        attention::{AttentionCategory, AttentionItem},
        discovery::SessionSnapshot,
        project::ProjectIdentity,
        readiness::Readiness,
    };
    use std::path::PathBuf;

    fn session(id: &str, readiness: Readiness, recency_at_ms: i64) -> SessionViewModel {
        SessionViewModel {
            id: id.into(),
            title: None,
            latest_result: None,
            project_identity: None,
            changed_files: Vec::new(),
            readiness,
            needs_attention: false,
            recency_at_ms,
            verification: None,
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
    fn project_filter_returns_matching_sessions_in_existing_order() {
        let selected = ProjectIdentity {
            normalized_name: "selected".into(),
            root_path: None,
            repository_identity: Some("repo:selected".into()),
        };
        let other = ProjectIdentity {
            normalized_name: "other".into(),
            root_path: None,
            repository_identity: Some("repo:other".into()),
        };
        let mut first = session("first", Readiness::Working, 10);
        first.project_identity = Some(selected.clone());
        let mut second = session("second", Readiness::Ready, 30);
        second.project_identity = Some(other);
        let mut third = session("third", Readiness::Unknown, 20);
        third.project_identity = Some(selected.clone());

        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![first, second, third]));

        assert_eq!(
            state
                .sessions_for_project(Some(&selected))
                .iter()
                .map(|session| session.id.as_str())
                .collect::<Vec<_>>(),
            vec!["third", "first"]
        );
        assert_eq!(state.sessions().len(), 3);
    }

    #[test]
    fn missing_project_filter_keeps_sessions_without_identity_visible() {
        let mut identified = session("identified", Readiness::Ready, 2);
        identified.project_identity = Some(ProjectIdentity {
            normalized_name: "project".into(),
            root_path: None,
            repository_identity: None,
        });
        let unscoped = session("unscoped", Readiness::Unknown, 1);

        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![identified, unscoped]));

        assert_eq!(state.sessions_for_project(None).len(), 2);
    }

    #[test]
    fn filtering_does_not_change_readiness_or_attention() {
        let selected = ProjectIdentity {
            normalized_name: "selected".into(),
            root_path: None,
            repository_identity: None,
        };
        let mut item = session("selected", Readiness::Working, 1);
        item.project_identity = Some(selected.clone());
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![item]));
        state.apply(SessionChange::Updated(SessionViewModel {
            readiness: Readiness::Ready,
            project_identity: Some(selected.clone()),
            ..session("selected", Readiness::Ready, 1)
        }));

        let filtered = state.sessions_for_project(Some(&selected));
        assert_eq!(filtered[0].readiness, Readiness::Ready);
        assert!(filtered[0].needs_attention);
        assert_eq!(state.sessions()[0].readiness, Readiness::Ready);
        assert!(state.sessions()[0].needs_attention);
    }

    #[test]
    fn project_context_propagates_without_changing_readiness_or_attention() {
        let snapshot = SessionSnapshot {
            id: "a".into(),
            title: Some("A task".into()),
            cwd: Some(r"C:\Users\kanat\dev\agent-hud".into()),
            project_identity: Some(ProjectIdentity {
                normalized_name: "agent-hud".into(),
                root_path: None,
                repository_identity: None,
            }),
            readiness: Readiness::Ready,
            latest_result: Some("completed result".into()),
            recency_at_ms: 1,
            lifecycle_timestamp: None,
            changed_files: Vec::new(),
            rollout_path: PathBuf::from("rollout.jsonl"),
            verification: None,
            workflow_events: Vec::new(),
        };
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![(&snapshot).into()]));

        let session = &state.sessions()[0];
        assert_eq!(
            session
                .project_identity
                .as_ref()
                .map(|project| project.normalized_name.as_str()),
            Some("agent-hud")
        );
        assert_eq!(session.readiness, Readiness::Ready);
        assert_eq!(session.latest_result.as_deref(), Some("completed result"));
        assert!(!session.needs_attention);
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
    fn unacknowledged_attention_survives_ready_to_ready_update() {
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![session(
            "a",
            Readiness::Working,
            1,
        )]));
        state.apply(SessionChange::Updated(session("a", Readiness::Ready, 1)));
        assert!(state.sessions()[0].needs_attention);

        state.apply(SessionChange::Updated(session("a", Readiness::Ready, 2)));

        assert!(state.sessions()[0].needs_attention);
        assert!(state.acknowledge("a"));
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

    #[test]
    fn attention_is_independent_from_readiness_and_removed_with_its_session() {
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![session(
            "a",
            Readiness::Working,
            1,
        )]));
        assert!(state.replace_attention(vec![AttentionItem {
            id: "review-a".into(),
            session_id: "a".into(),
            category: AttentionCategory::ReviewRequired,
            title: "Review required".into(),
            detail: None,
        }]));

        state.apply(SessionChange::Updated(session("a", Readiness::Unknown, 1)));
        assert_eq!(state.sessions()[0].readiness, Readiness::Unknown);
        assert_eq!(state.attention().items().len(), 1);

        assert!(state.apply(SessionChange::Removed("a".into())));
        assert!(state.attention().items().is_empty());
    }
}
