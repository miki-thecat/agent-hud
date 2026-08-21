use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use crate::readiness::{LifecycleEvent, LifecycleKind, Readiness, reduce_lifecycle};

pub const RECENT_SESSION_LIMIT: usize = 20;

#[derive(Debug, Eq, PartialEq)]
pub struct SessionSnapshot {
    pub id: String,
    pub title: Option<String>,
    pub readiness: Readiness,
    pub recency_at_ms: i64,
    pub lifecycle_timestamp: Option<String>,
}

struct Candidate {
    id: String,
    title: Option<String>,
    rollout_path: PathBuf,
    recency_at_ms: i64,
}

pub fn snapshot_from_paths(
    database_path: &Path,
    limit: usize,
) -> Result<Vec<SessionSnapshot>, DiscoveryError> {
    let connection = Connection::open_with_flags(database_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(DiscoveryError::Database)?;
    let candidates = read_candidates(&connection, limit)?;

    Ok(candidates
        .into_iter()
        .filter_map(|candidate| parse_rollout(candidate).ok())
        .collect())
}

fn read_candidates(
    connection: &Connection,
    limit: usize,
) -> Result<Vec<Candidate>, DiscoveryError> {
    let mut statement = connection
        .prepare(
            "SELECT id, title, rollout_path, COALESCE(recency_at_ms, updated_at_ms, 0) \
             FROM threads \
             WHERE thread_source = 'user' AND archived = 0 \
               AND TRIM(COALESCE(id, '')) <> '' \
               AND TRIM(COALESCE(rollout_path, '')) <> '' \
             ORDER BY COALESCE(recency_at_ms, updated_at_ms, 0) DESC, id ASC \
             LIMIT ?1",
        )
        .map_err(DiscoveryError::Database)?;
    let rows = statement
        .query_map([limit as i64], |row| {
            Ok(Candidate {
                id: row.get(0)?,
                title: row.get(1)?,
                rollout_path: PathBuf::from(row.get::<_, String>(2)?),
                recency_at_ms: row.get(3)?,
            })
        })
        .map_err(DiscoveryError::Database)?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DiscoveryError::Database)
}

fn parse_rollout(candidate: Candidate) -> Result<SessionSnapshot, DiscoveryError> {
    let file = File::open(&candidate.rollout_path).map_err(DiscoveryError::Rollout)?;
    let mut lines = BufReader::new(file).lines();
    let first = lines
        .next()
        .ok_or(DiscoveryError::InvalidMetadata)?
        .map_err(DiscoveryError::Rollout)?;
    let metadata: Value = serde_json::from_str(&first).map_err(DiscoveryError::Json)?;
    validate_metadata(&metadata, &candidate.id)?;

    let mut events = Vec::new();
    let mut lifecycle_timestamp = None;
    for line in lines {
        let line = line.map_err(DiscoveryError::Rollout)?;
        let record: Value = serde_json::from_str(&line).map_err(DiscoveryError::Json)?;
        if let Some((kind, turn_id, timestamp)) = lifecycle_record(&record)? {
            events.push((kind, turn_id));
            lifecycle_timestamp = timestamp;
        }
    }

    Ok(SessionSnapshot {
        id: candidate.id,
        title: candidate.title.filter(|title| !title.trim().is_empty()),
        readiness: reduce_lifecycle(events.iter().map(|(kind, turn_id)| LifecycleEvent {
            kind: *kind,
            turn_id,
        })),
        recency_at_ms: candidate.recency_at_ms,
        lifecycle_timestamp,
    })
}

fn validate_metadata(metadata: &Value, expected_id: &str) -> Result<(), DiscoveryError> {
    if metadata.get("type").and_then(Value::as_str) != Some("session_meta") {
        return Err(DiscoveryError::InvalidMetadata);
    }
    let payload = metadata
        .get("payload")
        .ok_or(DiscoveryError::InvalidMetadata)?;
    if payload.get("id").and_then(Value::as_str) != Some(expected_id) {
        return Err(DiscoveryError::IdentityMismatch);
    }
    if let Some(session_id) = payload.get("session_id")
        && session_id.as_str() != Some(expected_id)
    {
        return Err(DiscoveryError::IdentityMismatch);
    }
    if let Some(thread_source) = payload.get("thread_source")
        && thread_source.as_str() != Some("user")
    {
        return Err(DiscoveryError::IdentityMismatch);
    }
    Ok(())
}

fn lifecycle_record(
    record: &Value,
) -> Result<Option<(LifecycleKind, String, Option<String>)>, DiscoveryError> {
    if record.get("type").and_then(Value::as_str) != Some("event_msg") {
        return Ok(None);
    }
    let payload = record
        .get("payload")
        .ok_or(DiscoveryError::MalformedLifecycle)?;
    let kind = match payload.get("type").and_then(Value::as_str) {
        Some("task_started") => LifecycleKind::TaskStarted,
        Some("task_complete") => LifecycleKind::TaskComplete,
        _ => return Ok(None),
    };
    let turn_id = payload
        .get("turn_id")
        .and_then(Value::as_str)
        .filter(|turn_id| !turn_id.is_empty())
        .ok_or(DiscoveryError::MalformedLifecycle)?;
    Ok(Some((
        kind,
        turn_id.to_owned(),
        record
            .get("timestamp")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )))
}

#[derive(Debug)]
pub enum DiscoveryError {
    Database(rusqlite::Error),
    Rollout(std::io::Error),
    Json(serde_json::Error),
    InvalidMetadata,
    IdentityMismatch,
    MalformedLifecycle,
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
            Self::Rollout(error) => write!(formatter, "rollout read error: {error}"),
            Self::Json(error) => write!(formatter, "rollout JSON error: {error}"),
            Self::InvalidMetadata => formatter.write_str("invalid rollout session metadata"),
            Self::IdentityMismatch => formatter.write_str("rollout identity mismatch"),
            Self::MalformedLifecycle => formatter.write_str("malformed lifecycle record"),
        }
    }
}

impl std::error::Error for DiscoveryError {}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use rusqlite::Connection;

    use super::{RECENT_SESSION_LIMIT, snapshot_from_paths};
    use crate::readiness::Readiness;

    fn workspace(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("agent-hud-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn setup_database(path: &Path) -> Connection {
        let connection = Connection::open(path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE threads (
                id TEXT,
                title TEXT,
                rollout_path TEXT,
                recency_at_ms INTEGER,
                updated_at_ms INTEGER,
                thread_source TEXT,
                archived INTEGER
            );",
            )
            .unwrap();
        connection
    }

    fn rollout(id: &str, lifecycle: &str) -> String {
        format!(
            "{{\"timestamp\":\"2026-01-01T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\",\"session_id\":\"{id}\",\"thread_source\":\"user\"}}}}\n{lifecycle}\n"
        )
    }

    fn event(kind: &str, turn_id: &str) -> String {
        format!(
            "{{\"timestamp\":\"2026-01-01T00:00:01Z\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"{kind}\",\"turn_id\":\"{turn_id}\"}}}}"
        )
    }

    fn insert(connection: &Connection, id: &str, path: &Path, recency: i64, source: &str) {
        connection
            .execute(
                "INSERT INTO threads VALUES (?1, ?2, ?3, ?4, ?4, ?5, 0)",
                (
                    id,
                    "Synthetic session",
                    path.to_string_lossy().as_ref(),
                    recency,
                    source,
                ),
            )
            .unwrap();
    }

    #[test]
    fn discovers_and_reduces_latest_lifecycle_record() {
        let root = workspace("readiness");
        let database = root.join("state.sqlite");
        let connection = setup_database(&database);
        let path = root.join("rollout.jsonl");
        fs::write(
            &path,
            rollout(
                "root",
                &format!(
                    "{}\n{}",
                    event("task_complete", "one"),
                    event("task_started", "two")
                ),
            ),
        )
        .unwrap();
        insert(&connection, "root", &path, 10, "user");
        drop(connection);

        let snapshots = snapshot_from_paths(&database, RECENT_SESSION_LIMIT).unwrap();
        assert_eq!(snapshots[0].readiness, Readiness::Working);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn command_completion_does_not_make_a_session_ready() {
        let root = workspace("command");
        let database = root.join("state.sqlite");
        let connection = setup_database(&database);
        let path = root.join("rollout.jsonl");
        let command = "{\"type\":\"response_item\",\"payload\":{\"type\":\"command_execution\",\"status\":\"completed\"}}";
        fs::write(&path, rollout("root", command)).unwrap();
        insert(&connection, "root", &path, 10, "user");
        drop(connection);

        let snapshots = snapshot_from_paths(&database, RECENT_SESSION_LIMIT).unwrap();
        assert_eq!(snapshots[0].readiness, Readiness::Unknown);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mismatched_rollout_and_subagents_are_excluded() {
        let root = workspace("filter");
        let database = root.join("state.sqlite");
        let connection = setup_database(&database);
        let mismatch = root.join("mismatch.jsonl");
        fs::write(
            &mismatch,
            rollout("different", &event("task_complete", "one")),
        )
        .unwrap();
        insert(&connection, "root", &mismatch, 20, "user");
        let subagent = root.join("subagent.jsonl");
        fs::write(
            &subagent,
            rollout("subagent", &event("task_complete", "two")),
        )
        .unwrap();
        insert(&connection, "subagent", &subagent, 10, "subagent");
        drop(connection);

        assert!(
            snapshot_from_paths(&database, RECENT_SESSION_LIMIT)
                .unwrap()
                .is_empty()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_required_metadata_is_excluded() {
        let root = workspace("malformed-metadata");
        let database = root.join("state.sqlite");
        let connection = setup_database(&database);
        let path = root.join("malformed.jsonl");
        fs::write(&path, "{\"type\":\"session_meta\",\"payload\":{}}\n").unwrap();
        insert(&connection, "root", &path, 10, "user");
        drop(connection);

        assert!(
            snapshot_from_paths(&database, RECENT_SESSION_LIMIT)
                .unwrap()
                .is_empty()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn result_set_is_recency_ordered_and_bounded() {
        let root = workspace("bound");
        let database = root.join("state.sqlite");
        let connection = setup_database(&database);
        for index in 0..21 {
            let id = format!("root-{index}");
            let path = root.join(format!("{id}.jsonl"));
            fs::write(&path, rollout(&id, &event("task_complete", "turn"))).unwrap();
            insert(&connection, &id, &path, index, "user");
        }
        drop(connection);

        let snapshots = snapshot_from_paths(&database, RECENT_SESSION_LIMIT).unwrap();
        assert_eq!(snapshots.len(), RECENT_SESSION_LIMIT);
        assert_eq!(snapshots[0].id, "root-20");
        assert_eq!(snapshots.last().unwrap().id, "root-1");
        fs::remove_dir_all(root).unwrap();
    }
}
