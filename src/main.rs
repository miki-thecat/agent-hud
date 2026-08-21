mod discovery;
mod readiness;

use std::{env, path::PathBuf, process::ExitCode};

use discovery::{RECENT_SESSION_LIMIT, snapshot_from_paths};

fn codex_home() -> Result<PathBuf, String> {
    env::var_os("USERPROFILE")
        .or_else(|| {
            let drive = env::var_os("HOMEDRIVE")?;
            let path = env::var_os("HOMEPATH")?;
            Some(PathBuf::from(drive).join(path).into_os_string())
        })
        .map(PathBuf::from)
        .map(|home| home.join(".codex"))
        .ok_or_else(|| "cannot resolve the Windows user profile".to_owned())
}

fn display_label(title: Option<&str>) -> String {
    let title = title
        .filter(|title| !title.is_empty())
        .unwrap_or("(untitled)");
    let compact: String = title
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    compact.chars().take(80).collect()
}

fn main() -> ExitCode {
    let database_path = match codex_home() {
        Ok(home) => home.join("state_5.sqlite"),
        Err(error) => {
            eprintln!("agent-hud: {error}");
            return ExitCode::FAILURE;
        }
    };

    let snapshots = match snapshot_from_paths(&database_path, RECENT_SESSION_LIMIT) {
        Ok(snapshots) => snapshots,
        Err(error) => {
            eprintln!(
                "agent-hud: unable to read {}: {error}",
                database_path.display()
            );
            return ExitCode::FAILURE;
        }
    };

    println!(
        "Recent local sessions (recorded readiness; not live): {}",
        snapshots.len()
    );
    for session in snapshots {
        let lifecycle = session.lifecycle_timestamp.as_deref().unwrap_or("-");
        println!(
            "{}\t{}\t{}\trecency={}\tlifecycle={}",
            session.id,
            session.readiness.as_str(),
            display_label(session.title.as_deref()),
            session.recency_at_ms,
            lifecycle
        );
    }
    ExitCode::SUCCESS
}
