use std::cmp::Ordering;

use crate::{model::SessionViewModel, project::ProjectIdentity};

/// The result of collecting the latest available results for one project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectResultBundle {
    Available {
        project: ProjectIdentity,
        sessions: Vec<ProjectResult>,
    },
    Unavailable {
        project: ProjectIdentity,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectResult {
    pub title: Option<String>,
    pub session_id: String,
    pub readiness: crate::readiness::Readiness,
    pub latest_result: String,
}

impl ProjectResultBundle {
    /// Collects non-empty results from sessions belonging to this logical project.
    pub fn from_sessions(project: &ProjectIdentity, sessions: &[SessionViewModel]) -> Self {
        let mut results = sessions
            .iter()
            .filter(|session| {
                session
                    .project_identity
                    .as_ref()
                    .is_some_and(|candidate| same_logical_project(project, candidate))
            })
            .filter_map(|session| {
                let latest_result = session.latest_result.as_deref()?;
                (!latest_result.trim().is_empty()).then(|| ProjectResult {
                    title: session.title.clone(),
                    session_id: session.id.clone(),
                    readiness: session.readiness,
                    latest_result: latest_result.to_owned(),
                })
            })
            .collect::<Vec<_>>();

        results
            .sort_by(|left, right| session_ordering(sessions, &left.session_id, &right.session_id));

        if results.is_empty() {
            Self::Unavailable {
                project: project.clone(),
            }
        } else {
            Self::Available {
                project: project.clone(),
                sessions: results,
            }
        }
    }

    /// Formats the bundle as deterministic, copyable Markdown text.
    pub fn format(&self) -> String {
        match self {
            Self::Unavailable { project } => {
                format!(
                    "No latest results available for project: {}",
                    project.normalized_name
                )
            }
            Self::Available { project, sessions } => {
                let mut output = format!("# Project: {}\n", project.normalized_name);
                for (index, session) in sessions.iter().enumerate() {
                    output.push_str("\n## Session: ");
                    if let Some(title) = session.title.as_deref() {
                        output.push_str(title);
                        output.push_str(" (");
                    }
                    output.push_str(&session.session_id);
                    if session.title.is_some() {
                        output.push(')');
                    }
                    output.push_str("\nReadiness: ");
                    output.push_str(session.readiness.as_str());
                    output.push_str("\n\n");
                    output.push_str(&session.latest_result);
                    if index + 1 < sessions.len() {
                        output.push_str("\n\n---\n");
                    }
                }
                output
            }
        }
    }
}

fn same_logical_project(left: &ProjectIdentity, right: &ProjectIdentity) -> bool {
    match (&left.repository_identity, &right.repository_identity) {
        (Some(left), Some(right)) => left == right,
        _ => canonical_root_paths_equal(left.root_path.as_deref(), right.root_path.as_deref()),
    }
}

fn canonical_root_paths_equal(
    left: Option<&std::path::Path>,
    right: Option<&std::path::Path>,
) -> bool {
    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };

    std::fs::canonicalize(left)
        .ok()
        .zip(std::fs::canonicalize(right).ok())
        .is_some_and(|(left, right)| left == right)
}

fn session_ordering(sessions: &[SessionViewModel], left_id: &str, right_id: &str) -> Ordering {
    let left = sessions.iter().find(|session| session.id == left_id);
    let right = sessions.iter().find(|session| session.id == right_id);
    right
        .and_then(|session| left.map(|left| session.recency_at_ms.cmp(&left.recency_at_ms)))
        .unwrap_or(Ordering::Equal)
        .then_with(|| left_id.cmp(right_id))
}

#[cfg(test)]
mod tests {
    use super::ProjectResultBundle;
    use crate::{model::SessionViewModel, project::ProjectIdentity, readiness::Readiness};

    fn project(name: &str, root: &str, repository: &str) -> ProjectIdentity {
        ProjectIdentity {
            normalized_name: name.into(),
            root_path: Some(root.into()),
            repository_identity: Some(repository.into()),
        }
    }

    fn session(
        id: &str,
        project: &ProjectIdentity,
        recency_at_ms: i64,
        result: Option<&str>,
        title: Option<&str>,
    ) -> SessionViewModel {
        SessionViewModel {
            id: id.into(),
            title: title.map(str::to_owned),
            project_identity: Some(project.clone()),
            readiness: Readiness::Ready,
            latest_result: result.map(str::to_owned),
            needs_attention: false,
            recency_at_ms,
            changed_files: Vec::new(),
            verification: None,
        }
    }

    #[test]
    fn bundles_full_results_in_recency_then_id_order() {
        let project = project("agent-hud", r"C:\agent-hud", "repo:one");
        let sessions = vec![
            session("z", &project, 10, Some("older"), None),
            session("b", &project, 20, Some("second"), Some("Second")),
            session(
                "a",
                &project,
                20,
                Some("first\nwith all details"),
                Some("First"),
            ),
        ];

        let bundle = ProjectResultBundle::from_sessions(&project, &sessions);
        assert_eq!(
            bundle.format(),
            "# Project: agent-hud\n\n## Session: First (a)\nReadiness: READY\n\nfirst\nwith all details\n\n---\n\n## Session: Second (b)\nReadiness: READY\n\nsecond\n\n---\n\n## Session: z\nReadiness: READY\n\nolder"
        );
    }

    #[test]
    fn excludes_missing_and_whitespace_only_results() {
        let project = project("agent-hud", r"C:\agent-hud", "repo:one");
        let sessions = vec![
            session("missing", &project, 3, None, None),
            session("blank", &project, 2, Some(" \n\t"), None),
            session("kept", &project, 1, Some("result"), None),
        ];

        let bundle = ProjectResultBundle::from_sessions(&project, &sessions);
        assert!(bundle.format().contains("result"));
        assert!(!bundle.format().contains("missing"));
        assert!(!bundle.format().contains("blank"));
    }

    #[test]
    fn isolates_projects_by_complete_identity_not_display_name() {
        let selected = project("agent-hud", r"C:\one", "repo:one");
        let same_name_other_root = project("agent-hud", r"C:\two", "repo:two");
        let sessions = vec![
            session("selected", &selected, 2, Some("included"), None),
            session("other", &same_name_other_root, 3, Some("excluded"), None),
        ];

        let bundle = ProjectResultBundle::from_sessions(&selected, &sessions);
        assert!(bundle.format().contains("included"));
        assert!(!bundle.format().contains("excluded"));
    }

    #[test]
    fn bundles_linked_worktrees_with_the_same_repository_identity() {
        let selected = project("agent-hud", r"C:\worktrees\one", "repo:one");
        let linked_worktree = project("agent-hud", r"C:\worktrees\two", "repo:one");
        let sessions = vec![
            session("selected", &selected, 2, Some("included"), None),
            session(
                "linked",
                &linked_worktree,
                3,
                Some("linked result with full detail"),
                None,
            ),
        ];

        let bundle = ProjectResultBundle::from_sessions(&selected, &sessions);
        let formatted = bundle.format();
        assert!(formatted.contains("included"));
        assert!(formatted.contains("linked result with full detail"));
    }

    #[test]
    fn reports_unavailable_when_project_has_no_result() {
        let project = project("agent-hud", r"C:\agent-hud", "repo:one");
        let bundle =
            ProjectResultBundle::from_sessions(&project, &[session("a", &project, 1, None, None)]);

        assert_eq!(
            bundle,
            ProjectResultBundle::Unavailable {
                project: project.clone()
            }
        );
        assert_eq!(
            bundle.format(),
            "No latest results available for project: agent-hud"
        );
    }
}
