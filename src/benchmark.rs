//! Deterministic, fixture-backed integration coverage for Issue #100.
//!
//! This is test-only by design. It exercises the same discovery/reducer
//! boundary as the application without adding benchmark behavior to the
//! runtime binary.

use std::{fs, path::Path, time::Instant};

use rusqlite::Connection;

use crate::{
    discovery::snapshot_from_paths, readiness::Readiness, verification::VerificationOutcome,
};

const ALPHA_ROLLOUT: &str =
    include_str!("../benchmarks/multi-agent-integration/fixtures/alpha.jsonl");
const BETA_ROLLOUT: &str =
    include_str!("../benchmarks/multi-agent-integration/fixtures/beta.jsonl");
const GAMMA_ROLLOUT: &str =
    include_str!("../benchmarks/multi-agent-integration/fixtures/gamma.jsonl");

#[test]
fn multi_agent_fixture_covers_grouping_readiness_timeline_and_risk() {
    let root = std::env::temp_dir().join(format!("agent-hud-issue-100-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("project")).unwrap();
    let database = root.join("state_5.sqlite");
    let connection = Connection::open(&database).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE threads (
                id TEXT, title TEXT, cwd TEXT, rollout_path TEXT,
                recency_at_ms INTEGER, updated_at_ms INTEGER,
                thread_source TEXT, archived INTEGER
            );",
        )
        .unwrap();

    let sessions = [
        ("alpha", "Alpha implementation", ALPHA_ROLLOUT, 300),
        ("beta", "Beta verification", BETA_ROLLOUT, 200),
        ("gamma", "Gamma finished", GAMMA_ROLLOUT, 100),
    ];
    for (id, title, rollout, recency) in sessions {
        let path = root.join(format!("{id}.jsonl"));
        fs::write(&path, rollout).unwrap();
        connection
            .execute(
                "INSERT INTO threads VALUES (?1, ?2, ?3, ?4, ?5, ?5, 'user', 0)",
                (
                    id,
                    title,
                    root.join("project").to_string_lossy().as_ref(),
                    path.to_string_lossy().as_ref(),
                    recency,
                ),
            )
            .unwrap();
    }
    drop(connection);

    let started = Instant::now();
    let snapshots = snapshot_from_paths(&database, 20).unwrap();
    let elapsed = started.elapsed();
    eprintln!(
        "issue-100 discovery fixture: {elapsed:?} for {} sessions",
        snapshots.len()
    );

    assert_eq!(snapshots.len(), 3);
    assert!(snapshots.iter().all(|session| {
        session
            .project_identity
            .as_ref()
            .is_some_and(|project| project.normalized_name == "project")
    }));
    assert_eq!(snapshots[0].id, "alpha");
    assert_eq!(snapshots[0].readiness, Readiness::Working);
    assert_eq!(snapshots[0].workflow_events.len(), 4);
    assert_eq!(
        snapshots[0].changed_files,
        ["src/alpha.rs", "src/shared.rs"]
    );
    assert_eq!(
        snapshots[0]
            .verification
            .as_ref()
            .map(|evidence| evidence.outcome),
        Some(VerificationOutcome::Passed)
    );

    assert_eq!(snapshots[1].id, "beta");
    assert_eq!(snapshots[1].readiness, Readiness::Ready);
    assert_eq!(snapshots[1].workflow_events.len(), 5);
    assert_eq!(snapshots[1].changed_files, ["src/beta.rs", "src/shared.rs"]);
    assert_eq!(
        snapshots[1]
            .verification
            .as_ref()
            .map(|evidence| evidence.outcome),
        Some(VerificationOutcome::Failed)
    );

    assert_eq!(snapshots[2].id, "gamma");
    assert_eq!(snapshots[2].readiness, Readiness::Unknown);
    assert!(snapshots[2].workflow_events.is_empty());
    assert!(snapshots[2].verification.is_none());

    let alpha_files = &snapshots[0].changed_files;
    let beta_files = &snapshots[1].changed_files;
    assert!(alpha_files.iter().any(|path| beta_files.contains(path)));

    // The fixture is intentionally small; this catches accidental whole-file
    // scans or unbounded work in the benchmark path without a flaky tight SLA.
    assert!(
        elapsed.as_secs() < 5,
        "fixture discovery took too long: {elapsed:?}"
    );
    remove_fixture_root(&root);
}

fn remove_fixture_root(path: &Path) {
    fs::remove_dir_all(path).unwrap();
}
