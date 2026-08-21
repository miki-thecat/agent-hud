use crate::{discovery::SessionSnapshot, readiness::Readiness};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionViewModel {
    pub id: String,
    pub title: Option<String>,
    pub readiness: Readiness,
    pub recency_at_ms: i64,
}

impl From<&SessionSnapshot> for SessionViewModel {
    fn from(snapshot: &SessionSnapshot) -> Self {
        Self {
            id: snapshot.id.clone(),
            title: snapshot.title.clone(),
            readiness: snapshot.readiness,
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
}

#[cfg(test)]
mod tests {
    use super::{SessionChange, SessionViewModel};
    use crate::readiness::Readiness;

    fn session(id: &str, readiness: Readiness) -> SessionViewModel {
        SessionViewModel {
            id: id.into(),
            title: None,
            readiness,
            recency_at_ms: 0,
        }
    }

    #[test]
    fn typed_changes_preserve_row_identity_and_unknown() {
        let update = SessionChange::Updated(session("root", Readiness::Unknown));
        assert_eq!(
            update,
            SessionChange::Updated(session("root", Readiness::Unknown))
        );
    }
}
