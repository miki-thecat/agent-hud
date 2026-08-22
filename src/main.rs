pub mod attention;
pub mod capability;
#[cfg(windows)]
mod config;
mod discovery;
pub mod extensions;
pub mod linking;
mod metrics;
mod model;
mod project;
pub mod queue;
mod readiness;
pub mod result_bundle;
mod verification;
mod watcher;

#[cfg(test)]
mod benchmark;

#[cfg(windows)]
mod hud;
pub mod navigation;

use std::{env, path::PathBuf, process::ExitCode};

#[cfg(not(windows))]
use discovery::{RECENT_SESSION_LIMIT, snapshot_from_paths};
use model::SessionChange;

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

#[cfg(not(windows))]
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
    let codex_home = match codex_home() {
        Ok(home) => home,
        Err(error) => {
            eprintln!("agent-hud: {error}");
            return ExitCode::FAILURE;
        }
    };
    let database_path = codex_home.join("state_5.sqlite");

    if env::args().any(|argument| argument == "--watch") {
        return watch(database_path);
    }
    #[cfg(windows)]
    {
        let config = match config::Config::load_optional(&codex_home.join(config::CONFIG_FILE_NAME))
        {
            Ok(config) => config,
            Err(error) => {
                eprintln!("agent-hud: ignoring local config: {error}");
                config::Config::default()
            }
        };
        hud::run(database_path, config)
    }

    #[cfg(not(windows))]
    snapshot_cli(database_path)
}

#[cfg(not(windows))]
fn snapshot_cli(database_path: PathBuf) -> ExitCode {
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
            "{}\t{}\t{}\trecency={}\tlifecycle={}\tverification={}",
            session.id,
            session.readiness.as_str(),
            display_label(session.title.as_deref()),
            session.recency_at_ms,
            lifecycle,
            session
                .verification
                .as_ref()
                .map(|evidence| format!("{} ({})", evidence.outcome.as_str(), evidence.command))
                .unwrap_or_else(|| "-".to_owned())
        );
    }
    ExitCode::SUCCESS
}

fn watch(database_path: PathBuf) -> ExitCode {
    let sessions_dir = database_path
        .parent()
        .map(|path| path.join("sessions"))
        .unwrap_or_else(|| PathBuf::from("sessions"));
    let mut watcher = match watcher::LiveWatcher::new(database_path, sessions_dir) {
        Ok(watcher) => watcher,
        Err(error) => {
            eprintln!("agent-hud: unable to start watcher: {error}");
            return ExitCode::FAILURE;
        }
    };
    for change in watcher.initial_changes() {
        print_change(change);
    }
    let events = match watcher.watch() {
        Ok(events) => events,
        Err(error) => {
            eprintln!("agent-hud: unable to watch persisted state: {error}");
            return ExitCode::FAILURE;
        }
    };
    for event in events {
        match event {
            Ok(event) => match watcher.handle_event(&event) {
                Ok(lines) => {
                    for change in lines {
                        print_change(change);
                    }
                }
                Err(error) => {
                    eprintln!("agent-hud: observer event failed; recovering: {error}");
                    for change in watcher.recover() {
                        print_change(change);
                    }
                }
            },
            Err(error) => {
                eprintln!("agent-hud: filesystem observation error; recovering: {error}");
                for change in watcher.recover() {
                    print_change(change);
                }
            }
        }
    }
    ExitCode::SUCCESS
}

fn print_change(change: SessionChange) {
    match change {
        SessionChange::Snapshot(sessions) => {
            for session in sessions {
                println!(
                    "INITIAL {} {}{}",
                    session.id,
                    session.readiness.as_str(),
                    session_metadata_label(&session)
                );
            }
        }
        SessionChange::Updated(session) => {
            println!(
                "CHANGE {} {}{}",
                session.id,
                session.readiness.as_str(),
                session_metadata_label(&session)
            );
        }
        SessionChange::Removed(id) => println!("CHANGE {id} REMOVED"),
        SessionChange::ObservationDegraded { id } => println!("CHANGE {id} UNKNOWN"),
        SessionChange::ObservationTerminated => {
            eprintln!("agent-hud: observation terminated; readiness is UNKNOWN");
        }
    }
}

fn session_metadata_label(session: &model::SessionViewModel) -> String {
    let mut label = String::new();
    if session.readiness == readiness::Readiness::Ready && !session.changed_files.is_empty() {
        label.push_str(&format!(" files={}", session.changed_files.join(",")));
    }
    if let Some(evidence) = &session.verification {
        label.push_str(&format!(
            " verification={} ({})",
            evidence.outcome.as_str(),
            evidence.command
        ));
    }
    label
}
