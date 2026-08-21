#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Readiness {
    Working,
    Ready,
    Unknown,
}

impl Readiness {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Working => "WORKING",
            Self::Ready => "READY",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleKind {
    TaskStarted,
    TaskComplete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleEvent<'a> {
    pub kind: LifecycleKind,
    pub turn_id: &'a str,
}

/// Reduces only validated root/user lifecycle facts in rollout source order.
/// Item completions and every other rollout event intentionally have no effect.
/// A completion for another turn is ambiguous and therefore fails closed.
pub fn reduce_lifecycle<'a>(events: impl IntoIterator<Item = LifecycleEvent<'a>>) -> Readiness {
    let mut readiness = Readiness::Unknown;
    let mut active_turn = None;
    for event in events {
        readiness = match event {
            LifecycleEvent {
                kind: LifecycleKind::TaskStarted,
                turn_id,
            } => {
                active_turn = Some(turn_id);
                Readiness::Working
            }
            LifecycleEvent {
                kind: LifecycleKind::TaskComplete,
                turn_id,
            } if active_turn == Some(turn_id) => Readiness::Ready,
            LifecycleEvent {
                kind: LifecycleKind::TaskComplete,
                ..
            } => Readiness::Unknown,
        };
    }
    readiness
}

#[cfg(test)]
mod tests {
    use super::{LifecycleEvent, LifecycleKind, Readiness, reduce_lifecycle};

    fn event(kind: LifecycleKind, turn_id: &str) -> LifecycleEvent<'_> {
        LifecycleEvent { kind, turn_id }
    }

    #[test]
    fn task_started_is_working() {
        assert_eq!(
            reduce_lifecycle([event(LifecycleKind::TaskStarted, "turn")]),
            Readiness::Working
        );
    }

    #[test]
    fn task_complete_is_ready() {
        assert_eq!(
            reduce_lifecycle([
                event(LifecycleKind::TaskStarted, "turn"),
                event(LifecycleKind::TaskComplete, "turn"),
            ]),
            Readiness::Ready
        );
    }

    #[test]
    fn newer_start_supersedes_completion() {
        assert_eq!(
            reduce_lifecycle([
                event(LifecycleKind::TaskStarted, "one"),
                event(LifecycleKind::TaskComplete, "one"),
                event(LifecycleKind::TaskStarted, "two"),
            ]),
            Readiness::Working
        );
    }

    #[test]
    fn stale_completion_for_another_turn_fails_closed() {
        assert_eq!(
            reduce_lifecycle([
                event(LifecycleKind::TaskStarted, "one"),
                event(LifecycleKind::TaskStarted, "two"),
                event(LifecycleKind::TaskComplete, "one"),
            ]),
            Readiness::Unknown
        );
    }
}
