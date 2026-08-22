use crate::model::{WorkflowEvent, WorkflowEventKind};

/// Informational timing facts reconstructed from recorded workflow events.
///
/// Missing or unparseable timestamps remain missing. This model deliberately
/// has no bearing on readiness or any other lifecycle state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionMetrics {
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub verification_completed_at: Option<String>,
}

impl SessionMetrics {
    pub fn from_workflow_events(events: &[WorkflowEvent]) -> Self {
        let mut metrics = Self::default();
        let mut active_turn = None;

        for event in events {
            match event.kind {
                WorkflowEventKind::TaskStarted => {
                    if metrics.started_at.is_none() {
                        metrics.started_at = event.timestamp.clone();
                    }
                    active_turn = event.summary.clone();
                    metrics.ended_at = None;
                    metrics.duration_ms = None;
                }
                WorkflowEventKind::TaskCompleted
                    if active_turn.as_deref() == event.summary.as_deref() =>
                {
                    metrics.ended_at = event.timestamp.clone();
                    metrics.duration_ms = match (
                        metrics.started_at.as_deref().and_then(parse_timestamp_ms),
                        metrics.ended_at.as_deref().and_then(parse_timestamp_ms),
                    ) {
                        (Some(start), Some(end)) if end >= start => Some((end - start) as u64),
                        _ => None,
                    };
                    active_turn = None;
                }
                WorkflowEventKind::CommandExecution if event.timestamp.is_some() => {
                    metrics.verification_completed_at = event.timestamp.clone();
                }
                _ => {}
            }
        }

        metrics
    }
}

// Small RFC 3339 parser for the timestamps emitted by rollout records. It is
// intentionally conservative: unsupported forms simply do not produce a
// duration, while the source timestamp is still retained for inspection.
fn parse_timestamp_ms(value: &str) -> Option<i64> {
    let (date, time) = value.split_once('T')?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i64>().ok()?;
    let month = date_parts.next()?.parse::<i64>().ok()?;
    let day = date_parts.next()?.parse::<i64>().ok()?;
    let zone_start = time.find(['Z', '+', '-']).unwrap_or(time.len());
    let clock = &time[..zone_start];
    let mut clock_parts = clock.split(':');
    let hour = clock_parts.next()?.parse::<i64>().ok()?;
    let minute = clock_parts.next()?.parse::<i64>().ok()?;
    let seconds = clock_parts.next()?;
    let (second, fraction) = seconds.split_once('.').unwrap_or((seconds, ""));
    let second = second.parse::<i64>().ok()?;
    let fraction_ms = if fraction.is_empty() {
        0
    } else {
        let digits = fraction.chars().take(3).collect::<String>();
        digits.parse::<i64>().ok()? * 10_i64.pow(3 - digits.len() as u32)
    };
    let offset_minutes = match &time[zone_start..] {
        "Z" | "" => 0,
        offset => {
            let sign = if offset.starts_with('+') { 1 } else { -1 };
            let offset = &offset[1..];
            let (hours, minutes) = offset.split_once(':')?;
            sign * (hours.parse::<i64>().ok()? * 60 + minutes.parse::<i64>().ok()?)
        }
    };
    let days = days_from_civil(year, month, day)?;
    Some((((days * 24 + hour) * 60 + minute - offset_minutes) * 60 + second) * 1000 + fraction_ms)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146097 + day_of_era - 719468)
}

#[cfg(test)]
mod tests {
    use super::SessionMetrics;
    use crate::model::{WorkflowEvent, WorkflowEventKind};

    fn event(kind: WorkflowEventKind, timestamp: &str, summary: Option<&str>) -> WorkflowEvent {
        WorkflowEvent {
            sequence: 0,
            timestamp: Some(timestamp.into()),
            kind,
            summary: summary.map(str::to_owned),
        }
    }

    #[test]
    fn derives_duration_only_for_matching_ordered_lifecycle_events() {
        let metrics = SessionMetrics::from_workflow_events(&[
            event(
                WorkflowEventKind::TaskStarted,
                "2026-01-01T00:00:01.125Z",
                Some("turn-1"),
            ),
            event(
                WorkflowEventKind::TaskCompleted,
                "2026-01-01T00:00:03.500Z",
                Some("turn-1"),
            ),
        ]);
        assert_eq!(
            metrics.started_at.as_deref(),
            Some("2026-01-01T00:00:01.125Z")
        );
        assert_eq!(
            metrics.ended_at.as_deref(),
            Some("2026-01-01T00:00:03.500Z")
        );
        assert_eq!(metrics.duration_ms, Some(2_375));
    }

    #[test]
    fn an_unfinished_newer_turn_removes_end_and_duration() {
        let metrics = SessionMetrics::from_workflow_events(&[
            event(
                WorkflowEventKind::TaskStarted,
                "2026-01-01T00:00:01Z",
                Some("one"),
            ),
            event(
                WorkflowEventKind::TaskCompleted,
                "2026-01-01T00:00:02Z",
                Some("one"),
            ),
            event(
                WorkflowEventKind::TaskStarted,
                "2026-01-01T00:00:03Z",
                Some("two"),
            ),
        ]);
        assert!(metrics.ended_at.is_none());
        assert!(metrics.duration_ms.is_none());
    }

    #[test]
    fn keeps_verification_time_separate_and_does_not_require_it_for_duration() {
        let metrics = SessionMetrics::from_workflow_events(&[
            event(
                WorkflowEventKind::TaskStarted,
                "not-a-timestamp",
                Some("turn"),
            ),
            event(
                WorkflowEventKind::TaskCompleted,
                "also-invalid",
                Some("turn"),
            ),
            event(
                WorkflowEventKind::CommandExecution,
                "2026-01-01T00:00:04Z",
                None,
            ),
        ]);
        assert!(metrics.duration_ms.is_none());
        assert_eq!(
            metrics.verification_completed_at.as_deref(),
            Some("2026-01-01T00:00:04Z")
        );
    }

    #[test]
    fn ignores_unmatched_completion() {
        let metrics = SessionMetrics::from_workflow_events(&[event(
            WorkflowEventKind::TaskCompleted,
            "2026-01-01T00:00:02Z",
            Some("other"),
        )]);
        assert_eq!(metrics, SessionMetrics::default());
    }
}
