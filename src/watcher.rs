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
    readiness::{LifecycleKind, Readiness},
};

#[derive(Debug, Eq, PartialEq)]
pub enum RolloutUpdate {
    Readiness(Readiness),
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
        };
        reader.read_from_start()?;
        Ok(reader)
    }

    pub fn readiness(&self) -> Readiness {
        self.readiness
    }

    pub fn apply_append(&mut self) -> io::Result<RolloutUpdate> {
        let previous = self.readiness;
        let length = std::fs::metadata(&self.path)?.len();
        if length < self.offset {
            return Ok(RolloutUpdate::Reconcile);
        }
        self.read_new_bytes()?;
        if self.readiness == previous {
            Ok(RolloutUpdate::NoChange)
        } else {
            Ok(RolloutUpdate::Readiness(self.readiness))
        }
    }

    fn read_from_start(&mut self) -> io::Result<()> {
        self.offset = 0;
        self.partial.clear();
        self.active_turn = None;
        self.readiness = Readiness::Unknown;
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
        let Some((kind, turn_id, _)) = discovery::lifecycle_record(record)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        else {
            return Ok(());
        };
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

    pub fn initial_lines(&self) -> Vec<String> {
        self.tracked.values().map(line_for).collect()
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

    pub fn handle_event(&mut self, event: &Event) -> io::Result<Vec<String>> {
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
                    RolloutUpdate::Readiness(readiness) => {
                        session.snapshot.readiness = readiness;
                        output.push(format!(
                            "CHANGE {} {}",
                            session.snapshot.id,
                            readiness.as_str()
                        ));
                    }
                    RolloutUpdate::Reconcile => return self.reconcile(),
                    RolloutUpdate::NoChange => {}
                }
            }
        }
        Ok(output)
    }

    fn reconcile(&mut self) -> io::Result<Vec<String>> {
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
                if existing.snapshot.rollout_path != snapshot.rollout_path {
                    existing.rollout =
                        IncrementalRollout::open(snapshot.rollout_path.clone(), id.clone())
                            .map_err(io::Error::other)?;
                }
                existing.snapshot.title = snapshot.title.clone();
                existing.snapshot.recency_at_ms = snapshot.recency_at_ms;
                next.insert(id, existing);
            } else {
                let rollout = IncrementalRollout::open(snapshot.rollout_path.clone(), id.clone())
                    .map_err(io::Error::other)?;
                output.push(format!("CHANGE {} {}", id, rollout.readiness().as_str()));
                next.insert(id, TrackedSession { snapshot, rollout });
            }
        }
        for removed in previous_ids.difference(&next.keys().cloned().collect()) {
            output.push(format!("CHANGE {removed} REMOVED"));
        }
        self.tracked = next;
        Ok(output)
    }
}

fn line_for(session: &TrackedSession) -> String {
    format!(
        "INITIAL {} {}",
        session.snapshot.id,
        session.rollout.readiness().as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::{IncrementalRollout, RolloutUpdate};
    use crate::readiness::Readiness;
    use std::{fs, io::Write};

    fn rollout(path: &std::path::Path, id: &str) {
        fs::write(path, format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\",\"session_id\":\"{id}\",\"thread_source\":\"user\"}}}}\n")).unwrap();
    }
    fn event(kind: &str, turn: &str) -> String {
        format!(
            "{{\"timestamp\":\"2026-01-01T00:00:00Z\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"{kind}\",\"turn_id\":\"{turn}\"}}}}\n"
        )
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
        assert_eq!(
            reader.apply_append().unwrap(),
            RolloutUpdate::Readiness(Readiness::Working)
        );
        writeln!(
            file,
            "{{\"type\":\"response_item\",\"payload\":{{\"type\":\"command_execution\"}}}}"
        )
        .unwrap();
        assert_eq!(reader.apply_append().unwrap(), RolloutUpdate::NoChange);
        write!(file, "{}", event("task_complete", "one")).unwrap();
        assert_eq!(
            reader.apply_append().unwrap(),
            RolloutUpdate::Readiness(Readiness::Ready)
        );
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
        assert_eq!(
            reader.apply_append().unwrap(),
            RolloutUpdate::Readiness(Readiness::Working)
        );
        let _ = fs::remove_file(path);
    }
    #[test]
    fn duplicate_notification_has_no_semantic_change() {
        let (path, mut reader) = reader("duplicate");
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        write!(file, "{}", event("task_started", "one")).unwrap();
        assert_eq!(
            reader.apply_append().unwrap(),
            RolloutUpdate::Readiness(Readiness::Working)
        );
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
        assert_eq!(
            reader.apply_append().unwrap(),
            RolloutUpdate::Readiness(Readiness::Ready)
        );
        write!(file, "{}", event("task_started", "two")).unwrap();
        assert_eq!(
            reader.apply_append().unwrap(),
            RolloutUpdate::Readiness(Readiness::Working)
        );
        write!(file, "{}", event("task_complete", "one")).unwrap();
        assert_eq!(
            reader.apply_append().unwrap(),
            RolloutUpdate::Readiness(Readiness::Unknown)
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
}
