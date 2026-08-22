use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::Value;

use crate::{
    discovery::{self, SessionSnapshot},
    model::SessionChange,
    readiness::{LifecycleKind, Readiness},
    verification::{VerificationEvidence, parse_command_execution},
};

#[derive(Debug, Eq, PartialEq)]
pub enum RolloutUpdate {
    Changed,
    NoChange,
    Reconcile,
}

/// Incremental reader for one validated root/user rollout. It never exposes
/// rollout content; only the normalized readiness is returned.
#[derive(Debug)]
pub struct IncrementalRollout {
    path: PathBuf,
    expected_id: String,
    offset: u64,
    partial: Vec<u8>,
    active_turn: Option<String>,
    readiness: Readiness,
    last_lifecycle_timestamp: Option<String>,
    latest_result: Option<String>,
    changed_files: Vec<String>,
    verification: Option<VerificationEvidence>,
}

impl IncrementalRollout {
    pub fn open(path: PathBuf, expected_id: String) -> io::Result<Self> {
        let mut reader = Self {
            path,
            expected_id,
            offset: 0,
            partial: Vec::new(),
            active_turn: None,
            readiness: Readiness::Unknown,
            last_lifecycle_timestamp: None,
            latest_result: None,
            changed_files: Vec::new(),
            verification: None,
        };
        reader.read_from_start()?;
        Ok(reader)
    }

    pub fn readiness(&self) -> Readiness {
        self.readiness
    }

    pub fn changed_files(&self) -> &[String] {
        &self.changed_files
    }

    fn fail_closed(&mut self) {
        self.active_turn = None;
        self.readiness = Readiness::Unknown;
        self.last_lifecycle_timestamp = None;
    }

    pub fn apply_append(&mut self) -> io::Result<RolloutUpdate> {
        let previous = self.readiness;
        let previous_result = self.latest_result.clone();
        let previous_verification = self.verification.clone();
        let previous_files = self.changed_files.clone();
        let length = std::fs::metadata(&self.path)?.len();
        if length < self.offset {
            return Ok(RolloutUpdate::Reconcile);
        }
        self.read_new_bytes()?;
        if self.readiness != previous
            || self.latest_result != previous_result
            || self.changed_files != previous_files
            || self.verification != previous_verification
        {
            // One filesystem notification can cover several rollout records.
            // Publish the complete normalized state as one update.
            Ok(RolloutUpdate::Changed)
        } else {
            Ok(RolloutUpdate::NoChange)
        }
    }

    fn read_from_start(&mut self) -> io::Result<()> {
        self.offset = 0;
        self.partial.clear();
        self.active_turn = None;
        self.readiness = Readiness::Unknown;
        self.latest_result = None;
        self.changed_files.clear();
        self.verification = None;
        self.read_new_bytes()
    }

    fn read_new_bytes(&mut self) -> io::Result<()> {
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.offset))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        self.offset += bytes.len() as u64;
        self.partial.extend(bytes);
        while let Some(newline) = self.partial.iter().position(|byte| *byte == b'\n') {
            let line = self.partial.drain(..=newline).collect::<Vec<_>>();
            let line = &line[..line.len() - 1];
            if line.is_empty() {
                continue;
            }
            let record: Value = serde_json::from_slice(line)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            self.apply_record(&record)?;
        }
        Ok(())
    }

    fn apply_record(&mut self, record: &Value) -> io::Result<()> {
        if self.offset > 0
            && self.active_turn.is_none()
            && self.readiness == Readiness::Unknown
            && record.get("type").and_then(Value::as_str) == Some("session_meta")
        {
            discovery::validate_metadata(record, &self.expected_id)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            return Ok(());
        }
        let file_paths = discovery::file_change_paths(record);
        discovery::append_changed_files(&mut self.changed_files, file_paths);
        if let Some(evidence) = parse_command_execution(record) {
            self.verification = Some(evidence);
        }
        if let Some(result) = discovery::assistant_result(record) {
            self.latest_result = Some(result);
        }
        let Some((kind, turn_id, timestamp)) = discovery::lifecycle_record(record)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        else {
            return Ok(());
        };
        self.last_lifecycle_timestamp = timestamp;
        match kind {
            LifecycleKind::TaskStarted => {
                self.active_turn = Some(turn_id);
                self.readiness = Readiness::Working;
            }
            LifecycleKind::TaskComplete
                if self.active_turn.as_deref() == Some(turn_id.as_str()) =>
            {
                self.readiness = Readiness::Ready;
            }
            LifecycleKind::TaskComplete => self.readiness = Readiness::Unknown,
        }
        Ok(())
    }
}

#[derive(Debug)]
struct TrackedSession {
    snapshot: SessionSnapshot,
    rollout: IncrementalRollout,
}

#[derive(Debug)]
pub struct LiveWatcher {
    database_path: PathBuf,
    sessions_dir: PathBuf,
    tracked: BTreeMap<String, TrackedSession>,
    watcher: Option<RecommendedWatcher>,
}

impl LiveWatcher {
    pub fn new(database_path: PathBuf, sessions_dir: PathBuf) -> io::Result<Self> {
        let snapshots =
            discovery::snapshot_from_paths(&database_path, discovery::RECENT_SESSION_LIMIT)
                .map_err(io::Error::other)?;
        let tracked = snapshots
            .into_iter()
            .filter_map(|snapshot| {
                let rollout =
                    IncrementalRollout::open(snapshot.rollout_path.clone(), snapshot.id.clone())
                        .ok()?;
                Some((snapshot.id.clone(), TrackedSession { snapshot, rollout }))
            })
            .collect();
        Ok(Self {
            database_path,
            sessions_dir,
            tracked,
            watcher: None,
        })
    }

    pub fn initial_changes(&self) -> Vec<SessionChange> {
        vec![SessionChange::Snapshot(
            self.tracked
                .values()
                .map(|session| (&session.snapshot).into())
                .collect(),
        )]
    }

    pub fn degrade(&mut self) -> Vec<SessionChange> {
        let mut changes = self.fail_closed();
        changes.push(SessionChange::ObservationTerminated);
        changes
    }

    pub fn watch(&mut self) -> io::Result<Receiver<notify::Result<Event>>> {
        let (tx, rx) = mpsc::channel();
        let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |event| {
            let _ = tx.send(event);
        })
        .map_err(io::Error::other)?;
        watcher
            .watch(&self.sessions_dir, RecursiveMode::Recursive)
            .map_err(io::Error::other)?;
        if let Some(parent) = self.database_path.parent() {
            watcher
                .watch(parent, RecursiveMode::NonRecursive)
                .map_err(io::Error::other)?;
        }
        self.watcher = Some(watcher);
        Ok(rx)
    }

    pub fn handle_event(&mut self, event: &Event) -> io::Result<Vec<SessionChange>> {
        if matches!(event.kind, EventKind::Other)
            || matches!(event.kind, EventKind::Create(_) | EventKind::Remove(_))
            || event.paths.iter().any(|path| {
                path == &self.database_path
                    || path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("state_5.sqlite"))
            })
        {
            return self.reconcile();
        }
        let mut output = Vec::new();
        for path in &event.paths {
            if let Some(session) = self
                .tracked
                .values_mut()
                .find(|session| session.snapshot.rollout_path == *path)
            {
                match session.rollout.apply_append()? {
                    RolloutUpdate::Changed => {
                        session.snapshot.readiness = session.rollout.readiness;
                        session.snapshot.latest_result = session.rollout.latest_result.clone();
                        session.snapshot.changed_files = session.rollout.changed_files().to_vec();
                        session.snapshot.verification = session.rollout.verification.clone();
                        output.push(SessionChange::Updated((&session.snapshot).into()));
                    }
                    RolloutUpdate::Reconcile => return self.reconcile(),
                    RolloutUpdate::NoChange => {}
                }
            }
        }
        Ok(output)
    }

    /// Rebuild all bounded tracked rollout readers from disk. This recovery
    /// path handles missed or overflowed notifications; ordinary append
    /// events remain incremental.
    pub fn recover(&mut self) -> Vec<SessionChange> {
        match self.reconcile() {
            Ok(lines) => lines,
            Err(error) => {
                eprintln!("agent-hud: recovery failed; failing closed: {error}");
                self.degrade()
            }
        }
    }

    fn fail_closed(&mut self) -> Vec<SessionChange> {
        let mut output = Vec::new();
        for session in self.tracked.values_mut() {
            if session.rollout.readiness() != Readiness::Unknown {
                session.rollout.fail_closed();
                session.snapshot.readiness = Readiness::Unknown;
                output.push(SessionChange::ObservationDegraded {
                    id: session.snapshot.id.clone(),
                });
            }
        }
        output
    }

    fn reconcile(&mut self) -> io::Result<Vec<SessionChange>> {
        let snapshots =
            discovery::snapshot_from_paths(&self.database_path, discovery::RECENT_SESSION_LIMIT)
                .map_err(io::Error::other)?;
        let mut next = BTreeMap::new();
        let mut output = Vec::new();
        let previous_ids = self
            .tracked
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        for snapshot in snapshots {
            let id = snapshot.id.clone();
            if let Some(mut existing) = self.tracked.remove(&id) {
                let previous = existing.rollout.readiness();
                let previous_result = existing.rollout.latest_result.clone();
                let previous_files = existing.rollout.changed_files().to_vec();
                let previous_verification = existing.rollout.verification.clone();
                let rollout = IncrementalRollout::open(snapshot.rollout_path.clone(), id.clone())
                    .map_err(io::Error::other)?;
                if previous != rollout.readiness()
                    || previous_result != rollout.latest_result
                    || previous_files != rollout.changed_files()
                    || previous_verification != rollout.verification
                {
                    output.push(SessionChange::Updated((&snapshot).into()));
                }
                existing.snapshot = snapshot;
                existing.rollout = rollout;
                next.insert(id, existing);
            } else {
                let rollout = IncrementalRollout::open(snapshot.rollout_path.clone(), id.clone())
                    .map_err(io::Error::other)?;
                output.push(SessionChange::Updated((&snapshot).into()));
                next.insert(id, TrackedSession { snapshot, rollout });
            }
        }
        for removed in previous_ids.difference(&next.keys().cloned().collect()) {
            output.push(SessionChange::Removed(removed.clone()));
        }
        self.tracked = next;
        if output.is_empty() {
            Ok(output)
        } else {
            Ok(vec![SessionChange::Snapshot(
                self.tracked
                    .values()
                    .map(|session| (&session.snapshot).into())
                    .collect(),
            )])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IncrementalRollout, LiveWatcher, RolloutUpdate};
    use crate::model::SessionChange;
    use crate::readiness::Readiness;
    use rusqlite::Connection;
    use std::{fs, io::Write};

    fn rollout(path: &std::path::Path, id: &str) {
        fs::write(path, format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\",\"session_id\":\"{id}\",\"thread_source\":\"user\"}}}}\n")).unwrap();
    }
    fn event(kind: &str, turn: &str) -> String {
        format!(
            "{{\"timestamp\":\"2026-01-01T00:00:00Z\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"{kind}\",\"turn_id\":\"{turn}\"}}}}\n"
        )
    }

    fn setup_live_watcher(name: &str, contents: &str) -> (std::path::PathBuf, LiveWatcher) {
        let root =
            std::env::temp_dir().join(format!("agent-hud-live-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let database = root.join("state_5.sqlite");
        let rollout_path = root.join("rollout.jsonl");
        fs::write(&rollout_path, contents).unwrap();
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
        connection
            .execute(
                "INSERT INTO threads VALUES ('root', 'Synthetic', NULL, ?1, 1, 1, 'user', 0)",
                [rollout_path.to_string_lossy().as_ref()],
            )
            .unwrap();
        drop(connection);
        let watcher = LiveWatcher::new(database, root.join("sessions")).unwrap();
        (root, watcher)
    }

    fn metadata() -> String {
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"root\",\"session_id\":\"root\",\"thread_source\":\"user\"}}\n".into()
    }
    fn reader(name: &str) -> (std::path::PathBuf, IncrementalRollout) {
        let path =
            std::env::temp_dir().join(format!("agent-hud-watch-{name}-{}", std::process::id()));
        rollout(&path, "root");
        let reader = IncrementalRollout::open(path.clone(), "root".into()).unwrap();
        (path, reader)
    }
    #[test]
    fn append_lifecycle_changes_only_on_complete_records() {
        let (path, mut reader) = reader("lifecycle");
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        write!(file, "{}", event("task_started", "one")).unwrap();
        assert_eq!(reader.apply_append().unwrap(), RolloutUpdate::Changed);
        writeln!(
            file,
            "{{\"type\":\"response_item\",\"payload\":{{\"type\":\"command_execution\"}}}}"
        )
        .unwrap();
        assert_eq!(reader.apply_append().unwrap(), RolloutUpdate::NoChange);
        write!(file, "{}", event("task_complete", "one")).unwrap();
        assert_eq!(reader.apply_append().unwrap(), RolloutUpdate::Changed);
        let _ = fs::remove_file(path);
    }
    #[test]
    fn partial_line_is_ignored_until_completed() {
        let (path, mut reader) = reader("partial");
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        let line = event("task_started", "one");
        let (first, second) = line.split_at(line.len() - 2);
        write!(file, "{first}").unwrap();
        assert_eq!(reader.apply_append().unwrap(), RolloutUpdate::NoChange);
        write!(file, "{second}").unwrap();
        assert_eq!(reader.apply_append().unwrap(), RolloutUpdate::Changed);
        let _ = fs::remove_file(path);
    }
    #[test]
    fn duplicate_notification_has_no_semantic_change() {
        let (path, mut reader) = reader("duplicate");
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        write!(file, "{}", event("task_started", "one")).unwrap();
        assert_eq!(reader.apply_append().unwrap(), RolloutUpdate::Changed);
        assert_eq!(reader.apply_append().unwrap(), RolloutUpdate::NoChange);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn newer_turn_supersedes_old_completion() {
        let (path, mut reader) = reader("supersede");
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        write!(
            file,
            "{}{}",
            event("task_started", "one"),
            event("task_complete", "one")
        )
        .unwrap();
        assert_eq!(reader.apply_append().unwrap(), RolloutUpdate::Changed);
        write!(file, "{}", event("task_started", "two")).unwrap();
        assert_eq!(reader.apply_append().unwrap(), RolloutUpdate::Changed);
        write!(file, "{}", event("task_complete", "one")).unwrap();
        assert_eq!(reader.apply_append().unwrap(), RolloutUpdate::Changed);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn one_append_publishes_lifecycle_and_all_metadata_together() {
        let (path, mut reader) = reader("coalesced-metadata");
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        write!(file, "{}", event("task_started", "one")).unwrap();
        assert_eq!(reader.apply_append().unwrap(), RolloutUpdate::Changed);
        file.write_all(
            br#"{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"FileChange","changes":[{"path":"src/main.rs"}]}}}
{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"CommandExecution","command":"cargo test","status":"completed","exit_code":0,"aggregated_output":"test result: ok"}}}
{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"AgentMessage","phase":"final_answer","content":[{"type":"Text","text":"done"}]}}}
"#,
        )
        .unwrap();
        write!(file, "{}", event("task_complete", "one")).unwrap();

        assert_eq!(reader.apply_append().unwrap(), RolloutUpdate::Changed);
        assert_eq!(reader.readiness, Readiness::Ready);
        assert_eq!(reader.latest_result.as_deref(), Some("done"));
        assert_eq!(reader.changed_files, vec!["src/main.rs"]);
        assert_eq!(
            reader
                .verification
                .as_ref()
                .map(|evidence| evidence.command.as_str()),
            Some("cargo test")
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn identity_mismatch_is_rejected() {
        let path =
            std::env::temp_dir().join(format!("agent-hud-watch-mismatch-{}", std::process::id()));
        rollout(&path, "different");
        assert!(IncrementalRollout::open(path.clone(), "root".into()).is_err());
        let _ = fs::remove_file(path);
    }
    #[test]
    fn truncation_requests_reconciliation() {
        let (path, mut reader) = reader("truncate");
        fs::write(&path, "").unwrap();
        assert_eq!(reader.apply_append().unwrap(), RolloutUpdate::Reconcile);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn reconciliation_reopens_tracked_rollout_after_missed_append() {
        let (root, mut watcher) = setup_live_watcher(
            "missed-append",
            &format!("{}{}", metadata(), event("task_started", "one")),
        );
        assert!(
            matches!(watcher.initial_changes().as_slice(), [SessionChange::Snapshot(items)] if items[0].readiness == Readiness::Working)
        );
        let rollout_path = root.join("rollout.jsonl");
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&rollout_path)
            .unwrap();
        write!(file, "{}", event("task_complete", "one")).unwrap();

        assert!(
            matches!(watcher.reconcile().unwrap().as_slice(), [SessionChange::Snapshot(items)] if items[0].readiness == Readiness::Ready)
        );
        assert!(watcher.reconcile().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_recovery_degrades_tracked_readiness_to_unknown() {
        let (root, mut watcher) = setup_live_watcher(
            "error-recovery",
            &format!(
                "{}{}{}",
                metadata(),
                event("task_started", "one"),
                event("task_complete", "one")
            ),
        );
        assert!(
            matches!(watcher.initial_changes().as_slice(), [SessionChange::Snapshot(items)] if items[0].readiness == Readiness::Ready)
        );
        fs::remove_file(root.join("state_5.sqlite")).unwrap();

        assert!(matches!(
            watcher.recover().as_slice(),
            [SessionChange::ObservationDegraded { id }, SessionChange::ObservationTerminated]
                if id == "root"
        ));
        assert!(
            matches!(watcher.initial_changes().as_slice(), [SessionChange::Snapshot(items)] if items[0].readiness == Readiness::Unknown)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
