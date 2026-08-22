use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use crate::readiness::{LifecycleEvent, LifecycleKind, Readiness, reduce_lifecycle};
use crate::verification::{VerificationEvidence, parse_command_execution};

pub const RECENT_SESSION_LIMIT: usize = 20;
pub const CHANGED_FILE_LIMIT: usize = 5;

#[derive(Debug, Eq, PartialEq)]
pub struct SessionSnapshot {
    pub id: String,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub project_label: Option<String>,
    pub readiness: Readiness,
    pub latest_result: Option<String>,
    pub recency_at_ms: i64,
    pub lifecycle_timestamp: Option<String>,
    pub changed_files: Vec<String>,
    pub rollout_path: PathBuf,
    pub verification: Option<VerificationEvidence>,
}

struct Candidate {
    id: String,
    title: Option<String>,
    cwd: Option<String>,
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
            "SELECT id, title, cwd, rollout_path, COALESCE(recency_at_ms, updated_at_ms, 0) \
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
                cwd: row.get(2)?,
                rollout_path: PathBuf::from(row.get::<_, String>(3)?),
                recency_at_ms: row.get(4)?,
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
    let mut verification = None;
    let mut changed_files = Vec::new();
    let mut latest_result = None;
    for line in lines {
        let line = line.map_err(DiscoveryError::Rollout)?;
        if line.trim().is_empty() {
            continue;
        }
        let record: Value = serde_json::from_str(&line).map_err(DiscoveryError::Json)?;
        if let Some((kind, turn_id, timestamp)) = lifecycle_record(&record)? {
            events.push((kind, turn_id));
            lifecycle_timestamp = timestamp;
        }
        if let Some(evidence) = parse_command_execution(&record) {
            verification = Some(evidence);
        }
        append_changed_files(&mut changed_files, file_change_paths(&record));
        if let Some(result) = assistant_result(&record) {
            latest_result = Some(result);
        }
    }

    Ok(SessionSnapshot {
        id: candidate.id,
        title: candidate.title.filter(|title| !title.trim().is_empty()),
        project_label: project_label(candidate.cwd.as_deref()),
        cwd: candidate.cwd,
        readiness: reduce_lifecycle(events.iter().map(|(kind, turn_id)| LifecycleEvent {
            kind: *kind,
            turn_id,
        })),
        latest_result,
        recency_at_ms: candidate.recency_at_ms,
        lifecycle_timestamp,
        changed_files,
        rollout_path: candidate.rollout_path,
        verification,
    })
}

/// Extracts informational assistant output without contributing to readiness.
/// Missing, partial, or non-final assistant messages are ignored.
pub(crate) fn assistant_result(record: &Value) -> Option<String> {
    let payload = record.get("payload")?;
    let content = match payload.get("type").and_then(Value::as_str) {
        // Observed current response_item shape for final assistant output.
        Some("message")
            if payload.get("role").and_then(Value::as_str) == Some("assistant")
                && payload.get("phase").and_then(Value::as_str) == Some("final_answer") =>
        {
            payload.get("content")?.as_array()?
        }
        // Observed current event_msg item_completed shape for final output.
        Some("item_completed")
            if payload
                .get("item")
                .and_then(|item| item.get("type"))
                .and_then(Value::as_str)
                == Some("AgentMessage")
                && payload
                    .get("item")
                    .and_then(|item| item.get("phase"))
                    .and_then(Value::as_str)
                    == Some("final_answer") =>
        {
            payload.get("item")?.get("content")?.as_array()?
        }
        _ => return None,
    };
    let result = content
        .iter()
        .filter_map(|item| {
            (matches!(
                item.get("type").and_then(Value::as_str),
                Some("output_text") | Some("Text")
            ))
            .then(|| item.get("text").and_then(Value::as_str))
            .flatten()
        })
        .collect::<Vec<_>>()
        .join("");
    (!result.trim().is_empty()).then_some(result)
}

pub fn project_label(cwd: Option<&str>) -> Option<String> {
    let path = cwd?.trim().trim_end_matches(['\\', '/']);
    if path.is_empty() || path.ends_with(':') {
        return None;
    }
    let component = path.rsplit(['\\', '/']).next()?.trim();
    if component.is_empty() || component == "." || component == ".." {
        return None;
    }
    Some(component.to_owned())
}

pub(crate) fn validate_metadata(metadata: &Value, expected_id: &str) -> Result<(), DiscoveryError> {
    if metadata.get("type").and_then(Value::as_str) != Some("session_meta") {
        return Err(DiscoveryError::InvalidMetadata);
    }
    let payload = metadata
        .get("payload")
        .ok_or(DiscoveryError::InvalidMetadata)?;
    if payload.get("id").and_then(Value::as_str) != Some(expected_id) {
        return Err(DiscoveryError::IdentityMismatch);
    }
    if payload.get("session_id").and_then(Value::as_str) != Some(expected_id) {
        return Err(DiscoveryError::IdentityMismatch);
    }
    if payload.get("thread_source").and_then(Value::as_str) != Some("user") {
        return Err(DiscoveryError::IdentityMismatch);
    }
    Ok(())
}

pub(crate) fn lifecycle_record(
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

pub(crate) fn file_change_paths(record: &Value) -> Vec<String> {
    let Some(payload) = record.get("payload") else {
        return Vec::new();
    };
    let payload_type = payload.get("type").and_then(Value::as_str);
    let is_file_change_event = match (record.get("type").and_then(Value::as_str), payload_type) {
        (Some("event_msg"), Some("item_completed")) => payload
            .get("item")
            .and_then(Value::as_object)
            .and_then(|item| item.get("type"))
            .and_then(Value::as_str)
            .is_some_and(|kind| {
                kind.eq_ignore_ascii_case("filechange") || kind.eq_ignore_ascii_case("file_change")
            }),
        _ => false,
    };
    if !is_file_change_event {
        return Vec::new();
    }
    let changes = payload.get("item").and_then(|item| item.get("changes"));
    let Some(changes) = changes else {
        return Vec::new();
    };
    match changes {
        Value::Array(items) => items
            .iter()
            .filter_map(|item| item.get("path").and_then(Value::as_str))
            .filter(|path| !path.trim().is_empty())
            .map(str::to_owned)
            .collect(),
        Value::Object(items) => items
            .keys()
            .filter(|path| !path.trim().is_empty())
            .cloned()
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn append_changed_files(
    files: &mut Vec<String>,
    paths: impl IntoIterator<Item = String>,
) {
    for path in paths {
        files.retain(|existing| existing != &path);
        files.push(path);
        if files.len() > CHANGED_FILE_LIMIT {
            files.remove(0);
        }
    }
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

    use super::{
        CHANGED_FILE_LIMIT, RECENT_SESSION_LIMIT, append_changed_files, assistant_result,
        file_change_paths, project_label, snapshot_from_paths,
    };
    use crate::readiness::Readiness;
    use serde_json::json;

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
                cwd TEXT,
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
                "INSERT INTO threads VALUES (?1, ?2, 'C:\\Users\\kanat\\dev\\agent-hud', ?3, ?4, ?4, ?5, 0)",
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
    fn project_label_uses_final_windows_component_and_handles_trailing_separator() {
        assert_eq!(
            project_label(Some(r"C:\Users\kanat\dev\agent-hud")),
            Some("agent-hud".into())
        );
        assert_eq!(
            project_label(Some(r"C:\Users\kanat\dev\agent-hud\\")),
            Some("agent-hud".into())
        );
    }

    #[test]
    fn missing_or_empty_project_context_is_optional() {
        assert_eq!(project_label(None), None);
        assert_eq!(project_label(Some("   ")), None);
        assert_eq!(project_label(Some(r"C:\")), None);
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
        assert_eq!(snapshots[0].latest_result, None);
        assert_eq!(snapshots[0].project_label.as_deref(), Some("agent-hud"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn extracts_latest_final_assistant_result_without_affecting_readiness() {
        let root = workspace("assistant-result");
        let database = root.join("state.sqlite");
        let connection = setup_database(&database);
        let path = root.join("rollout.jsonl");
        let assistant = json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "assistant",
                "phase": "final_answer",
                "content": [{"type": "output_text", "text": "First"}]
            }
        });
        let latest = json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "assistant",
                "phase": "final_answer",
                "content": [
                    {"type": "output_text", "text": "Second"},
                    {"type": "output_text", "text": " result"}
                ]
            }
        });
        fs::write(
            &path,
            format!(
                "{}{}\n{}\n",
                rollout(
                    "root",
                    &format!(
                        "{}\n{}",
                        event("task_started", "turn"),
                        event("task_complete", "turn")
                    ),
                ),
                assistant,
                latest
            ),
        )
        .unwrap();
        insert(&connection, "root", &path, 10, "user");
        drop(connection);

        let snapshots = snapshot_from_paths(&database, RECENT_SESSION_LIMIT).unwrap();
        assert_eq!(snapshots[0].readiness, Readiness::Ready);
        assert_eq!(snapshots[0].latest_result.as_deref(), Some("Second result"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_or_non_final_assistant_results_are_ignored() {
        assert_eq!(assistant_result(&json!({"type": "response_item"})), None);
        assert_eq!(
            assistant_result(&json!({
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "assistant",
                    "phase": "commentary",
                    "content": [{"type": "output_text", "text": "not final"}]
                }
            })),
            None
        );
        assert_eq!(
            assistant_result(&json!({
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text"}]
                }
            })),
            None
        );
    }

    #[test]
    fn malformed_assistant_result_does_not_change_readiness() {
        let root = workspace("malformed-assistant-result");
        let database = root.join("state.sqlite");
        let connection = setup_database(&database);
        let path = root.join("rollout.jsonl");
        let malformed_assistant = "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":null}}";
        fs::write(
            &path,
            format!(
                "{}{}\n",
                rollout(
                    "root",
                    &format!(
                        "{}\n{}",
                        event("task_started", "turn"),
                        event("task_complete", "turn")
                    ),
                ),
                malformed_assistant
            ),
        )
        .unwrap();
        insert(&connection, "root", &path, 10, "user");
        drop(connection);

        let snapshots = snapshot_from_paths(&database, RECENT_SESSION_LIMIT).unwrap();
        assert_eq!(snapshots[0].readiness, Readiness::Ready);
        assert_eq!(snapshots[0].latest_result, None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn extracts_only_structured_file_change_events() {
        let item_completed = json!({
            "type": "event_msg",
            "payload": {
                "type": "item_completed",
                "item": {
                    "type": "FileChange",
                    "changes": [
                        {"path": "src/lib.rs", "kind": "update"},
                        {"path": "README.md", "kind": "update"}
                    ]
                }
            }
        });
        let raw_tool_call = json!({
            "type": "response_item",
            "payload": {"type": "custom_tool_call", "input": "*** Update File: ignored.rs"}
        });

        assert_eq!(
            file_change_paths(&item_completed),
            vec!["src/lib.rs", "README.md"]
        );
        assert!(file_change_paths(&raw_tool_call).is_empty());
    }

    #[test]
    fn changed_file_summary_is_recent_bounded_and_deterministic() {
        let mut files = Vec::new();
        append_changed_files(
            &mut files,
            (0..=CHANGED_FILE_LIMIT).map(|index| format!("file-{index}.rs")),
        );
        append_changed_files(&mut files, ["file-3.rs".into()]);

        assert_eq!(
            files,
            vec![
                "file-1.rs",
                "file-2.rs",
                "file-4.rs",
                "file-5.rs",
                "file-3.rs"
            ]
        );
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
    fn rollout_missing_session_id_is_excluded() {
        let root = workspace("missing-session-id");
        let database = root.join("state.sqlite");
        let connection = setup_database(&database);
        let path = root.join("rollout.jsonl");
        fs::write(
            &path,
            rollout("root", &event("task_complete", "turn"))
                .replace("\"session_id\":\"root\",", ""),
        )
        .unwrap();
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
    fn rollout_missing_thread_source_is_excluded() {
        let root = workspace("missing-thread-source");
        let database = root.join("state.sqlite");
        let connection = setup_database(&database);
        let path = root.join("rollout.jsonl");
        fs::write(
            &path,
            rollout("root", &event("task_complete", "turn"))
                .replace(",\"thread_source\":\"user\"", ""),
        )
        .unwrap();
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
