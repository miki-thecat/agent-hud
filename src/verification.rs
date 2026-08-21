use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationOutcome {
    Passed,
    Failed,
}

impl VerificationOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationEvidence {
    pub command: String,
    pub outcome: VerificationOutcome,
}

/// Extracts conservative, informational verification evidence from a completed
/// command execution. This parser never produces or changes readiness state.
pub fn parse_command_execution(record: &Value) -> Option<VerificationEvidence> {
    let payload = record.get("payload")?;
    if payload.get("type").and_then(Value::as_str) != Some("command_execution")
        || payload.get("status").and_then(Value::as_str) != Some("completed")
    {
        return None;
    }

    let command = payload.get("command").and_then(Value::as_str)?.trim();
    let command_name = recognized_command(command)?;
    let output = ["stdout", "stderr", "aggregated_output"]
        .into_iter()
        .filter_map(|key| payload.get(key).and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    let exit_code = payload.get("exit_code").and_then(Value::as_i64);

    let outcome = if exit_code.is_some_and(|code| code != 0)
        || output.contains("test result: FAILED")
        || output.contains("error: could not compile")
    {
        VerificationOutcome::Failed
    } else if exit_code == Some(0)
        || output.contains("test result: ok")
        || output.contains("Finished ")
    {
        VerificationOutcome::Passed
    } else {
        return None;
    };

    Some(VerificationEvidence {
        command: command_name.to_owned(),
        outcome,
    })
}

fn recognized_command(command: &str) -> Option<&'static str> {
    let mut words = command.split_whitespace();
    if words.next()?.rsplit(['/', '\\']).next()? != "cargo" {
        return None;
    }
    match words.next()? {
        "test" => Some("cargo test"),
        "check" => Some("cargo check"),
        "clippy" => Some("cargo clippy"),
        "build" => Some("cargo build"),
        "fmt" if words.any(|word| word == "--check") => Some("cargo fmt --check"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{VerificationEvidence, VerificationOutcome, parse_command_execution};

    fn command(
        command: &str,
        status: &str,
        exit_code: Option<i64>,
        output: &str,
    ) -> serde_json::Value {
        let mut payload = json!({
            "type": "command_execution",
            "command": command,
            "status": status,
            "stdout": output,
        });
        if let Some(exit_code) = exit_code {
            payload["exit_code"] = json!(exit_code);
        }
        json!({"type": "response_item", "payload": payload})
    }

    #[test]
    fn recognizes_successful_cargo_test_by_exit_code() {
        assert_eq!(
            parse_command_execution(&command("cargo test --all", "completed", Some(0), "")),
            Some(VerificationEvidence {
                command: "cargo test".into(),
                outcome: VerificationOutcome::Passed,
            })
        );
    }

    #[test]
    fn recognizes_successful_output_without_exit_code() {
        let record = command(
            "cargo test",
            "completed",
            None,
            "test result: ok. 3 passed; 0 failed",
        );
        assert_eq!(
            parse_command_execution(&record).unwrap().outcome,
            VerificationOutcome::Passed
        );
    }

    #[test]
    fn recognizes_failed_exit_code() {
        assert_eq!(
            parse_command_execution(&command("cargo check", "completed", Some(101), ""))
                .unwrap()
                .outcome,
            VerificationOutcome::Failed
        );
    }

    #[test]
    fn ignores_running_commands() {
        assert_eq!(
            parse_command_execution(&command("cargo test", "in_progress", Some(0), "")),
            None
        );
    }

    #[test]
    fn ignores_unrecognized_commands() {
        assert_eq!(
            parse_command_execution(&command("npm test", "completed", Some(0), "")),
            None
        );
    }

    #[test]
    fn ignores_ambiguous_completed_output() {
        assert_eq!(
            parse_command_execution(&command(
                "cargo check",
                "completed",
                None,
                "Checking agent-hud"
            )),
            None
        );
    }

    #[test]
    fn does_not_treat_readiness_lifecycle_as_verification() {
        assert_eq!(
            parse_command_execution(
                &json!({"type":"event_msg","payload":{"type":"task_complete"}})
            ),
            None
        );
    }
}
