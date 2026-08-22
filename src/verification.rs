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
    // Current rollout evidence records completed commands as
    // event_msg.item_completed with an item whose observed type is
    // CommandExecution. Do not broaden this to synthetic response_item
    // command shapes that are not present in the observed fixtures.
    let item = record
        .get("payload")
        .filter(|payload| payload.get("type").and_then(Value::as_str) == Some("item_completed"))
        .and_then(|payload| payload.get("item"))
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("CommandExecution"))?;
    let command = item.get("command").and_then(Value::as_array)?;
    let command_name = recognized_command(command)?;
    let status = item.get("status").and_then(Value::as_str)?;
    if !matches!(status, "completed" | "failed") {
        return None;
    }
    let output = ["stdout", "stderr", "aggregated_output"]
        .into_iter()
        .filter_map(|key| item.get(key).and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    let exit_code = item.get("exit_code").and_then(Value::as_i64);

    let outcome = if status == "failed"
        || exit_code.is_some_and(|code| code != 0)
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

fn recognized_command(argv: &[Value]) -> Option<&'static str> {
    let argv = argv.iter().map(Value::as_str).collect::<Option<Vec<_>>>()?;
    let mut words = if is_shell_wrapper(&argv) {
        argv[2..]
            .iter()
            .flat_map(|arg| arg.split_whitespace())
            .collect::<Vec<_>>()
    } else if argv.first()?.rsplit(['/', '\\']).next()? == "cargo" {
        argv
    } else {
        return None;
    };
    let cargo_positions = words
        .iter()
        .enumerate()
        .filter(|(_, word)| {
            word.trim_matches(['\'', '"']).rsplit(['/', '\\']).next() == Some("cargo")
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if cargo_positions.len() != 1 {
        return None;
    }
    words = words.split_off(cargo_positions[0]);
    if words
        .iter()
        .any(|word| matches!(*word, ";" | "&&" | "||" | "|"))
    {
        return None;
    }
    let mut words = words.into_iter();
    words.next()?;
    match words.next()?.trim_matches(['\'', '"']) {
        "test" => Some("cargo test"),
        "check" => Some("cargo check"),
        "clippy" => Some("cargo clippy"),
        "build" => Some("cargo build"),
        "fmt" if words.any(|word| word == "--check") => Some("cargo fmt --check"),
        _ => None,
    }
}

fn is_shell_wrapper(argv: &[&str]) -> bool {
    argv.len() >= 3
        && matches!(
            argv[0].rsplit(['/', '\\']).next(),
            Some("powershell.exe" | "pwsh.exe" | "cmd.exe" | "powershell" | "pwsh" | "cmd")
        )
        && matches!(argv[1], "-Command" | "-c" | "/c")
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{VerificationEvidence, VerificationOutcome, parse_command_execution};

    fn command(
        command: Value,
        status: &str,
        exit_code: Option<i64>,
        output: &str,
    ) -> serde_json::Value {
        let mut item = json!({
            "type": "CommandExecution",
            "command": command,
            "status": status,
            "stdout": output,
        });
        if let Some(exit_code) = exit_code {
            item["exit_code"] = json!(exit_code);
        }
        json!({"type": "event_msg", "payload": {"type": "item_completed", "item": item}})
    }

    #[test]
    fn recognizes_successful_cargo_test_by_exit_code() {
        assert_eq!(
            parse_command_execution(&command(
                json!(["cargo", "test", "--all"]),
                "completed",
                Some(0),
                ""
            )),
            Some(VerificationEvidence {
                command: "cargo test".into(),
                outcome: VerificationOutcome::Passed,
            })
        );
    }

    #[test]
    fn recognizes_successful_output_without_exit_code() {
        let record = command(
            json!(["cargo", "test"]),
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
            parse_command_execution(&command(
                json!(["cargo", "check"]),
                "completed",
                Some(101),
                ""
            ))
            .unwrap()
            .outcome,
            VerificationOutcome::Failed
        );
    }

    #[test]
    fn ignores_running_commands() {
        assert_eq!(
            parse_command_execution(&command(
                json!(["cargo", "test"]),
                "in_progress",
                Some(0),
                ""
            )),
            None
        );
    }

    #[test]
    fn ignores_unrecognized_commands() {
        assert_eq!(
            parse_command_execution(&command(json!(["npm", "test"]), "completed", Some(0), "")),
            None
        );
    }

    #[test]
    fn ignores_ambiguous_completed_output() {
        assert_eq!(
            parse_command_execution(&command(
                json!(["cargo", "check"]),
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

    #[test]
    fn recognizes_current_windows_shell_argv() {
        let record = command(
            json!([
                "C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe",
                "-Command",
                "cargo test --all"
            ]),
            "completed",
            Some(0),
            "",
        );
        assert_eq!(
            parse_command_execution(&record).unwrap().outcome,
            VerificationOutcome::Passed
        );
    }

    #[test]
    fn explicit_failed_status_is_failed_evidence() {
        assert_eq!(
            parse_command_execution(&command(json!(["cargo", "test"]), "failed", None, ""))
                .unwrap()
                .outcome,
            VerificationOutcome::Failed
        );
    }

    #[test]
    fn declined_and_in_progress_are_not_evidence() {
        for status in ["declined", "in_progress"] {
            assert_eq!(
                parse_command_execution(&command(json!(["cargo", "test"]), status, None, "")),
                None
            );
        }
    }

    #[test]
    fn rejects_string_and_ambiguous_commands() {
        assert_eq!(
            parse_command_execution(&command(json!("cargo test"), "completed", Some(0), "")),
            None
        );
        assert_eq!(
            parse_command_execution(&command(
                json!(["cargo", "test", ";", "cargo", "check"]),
                "completed",
                Some(0),
                ""
            )),
            None
        );
    }
}
