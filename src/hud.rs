use std::{path::PathBuf, sync::mpsc, thread};

use windows_canvas::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows_window::*;

use crate::{
    model::{SessionChange, SessionViewModel},
    watcher::LiveWatcher,
};

const WATCHER_UPDATE: u32 = WM_APP + 1;

pub fn run(database_path: PathBuf) -> std::process::ExitCode {
    if let Err(error) = run_window(database_path) {
        eprintln!("agent-hud: unable to start native HUD: {error}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}

fn run_window(database_path: PathBuf) -> windows_canvas::Result<()> {
    let window = Window::new("agent-hud — Recent local sessions")
        .size(620, 720)
        .create()?;
    let hwnd = window.hwnd() as usize;
    let (tx, rx) = mpsc::channel();
    let sessions_dir = database_path
        .parent()
        .map(|path| path.join("sessions"))
        .unwrap_or_else(|| PathBuf::from("sessions"));
    thread::spawn(move || {
        let mut watcher = match LiveWatcher::new(database_path, sessions_dir) {
            Ok(watcher) => watcher,
            Err(_) => return,
        };
        for change in watcher.initial_changes() {
            let _ = tx.send(change);
        }
        let events = match watcher.watch() {
            Ok(events) => events,
            Err(_) => return,
        };
        for event in events {
            let changes = match event {
                Ok(event) => watcher
                    .handle_event(&event)
                    .unwrap_or_else(|_| watcher.recover()),
                Err(_) => watcher.recover(),
            };
            for change in changes {
                let _ = tx.send(change);
            }
            unsafe {
                PostMessageW(hwnd as _, WATCHER_UPDATE, 0, 0);
            }
        }
    });

    let device = GpuDevice::new_or_warp()?;
    let (width, height) = window.client_size();
    let mut chain = device.create_swap_chain_for_window(&window, width as u32, height as u32)?;
    let mut sessions = Vec::<SessionViewModel>::new();
    let mut dirty = true;
    run_with(|| {
        while let Ok(change) = rx.try_recv() {
            dirty |= apply_change(&mut sessions, change);
        }
        let (width, height) = window.client_size();
        if width as u32 != chain.width() || height as u32 != chain.height() {
            chain.resize(width as u32, height as u32)?;
            dirty = true;
        }
        if !dirty {
            return Ok(false);
        }
        let width = chain.width() as f32;
        let height = chain.height() as f32;
        let session = chain.begin_draw()?;
        session.clear(ColorF::from_rgb8(248, 249, 251));
        let brush = session.create_solid_brush(ColorF::from_rgb8(30, 35, 42))?;
        let header = TextFormat::new_bold("Segoe UI", 22.0)?;
        session.draw_text(
            "Recent local sessions",
            &header,
            &Rect::new(24.0, 18.0, width - 24.0, 52.0),
            &brush,
        );
        let subhead = TextFormat::new("Segoe UI", 12.0)?;
        session.draw_text(
            "Recorded local readiness — not exact open chats",
            &subhead,
            &Rect::new(24.0, 52.0, width - 24.0, 76.0),
            &brush,
        );
        let row_format = TextFormat::new("Segoe UI", 15.0)?;
        for (index, item) in sessions.iter().enumerate() {
            let top = 92.0 + index as f32 * 30.0;
            let label = item
                .title
                .as_deref()
                .filter(|title| !title.is_empty())
                .unwrap_or("(untitled)");
            let label: String = label
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .take(64)
                .collect();
            let text = format!("{label}    {}", item.readiness.as_str());
            session.draw_text(
                &text,
                &row_format,
                &Rect::new(28.0, top, width - 24.0, top + 26.0),
                &brush,
            );
            if top + 30.0 > height {
                break;
            }
        }
        drop(session);
        chain.present()?;
        dirty = false;
        Ok(false)
    })
}

fn apply_change(sessions: &mut Vec<SessionViewModel>, change: SessionChange) -> bool {
    match change {
        SessionChange::Snapshot(items) => {
            sessions.clear();
            sessions.extend(items);
            true
        }
        SessionChange::Updated(item) => {
            if let Some(existing) = sessions.iter_mut().find(|existing| existing.id == item.id) {
                *existing = item;
            } else {
                sessions.push(item);
            }
            true
        }
        SessionChange::Removed(id) => {
            let length = sessions.len();
            sessions.retain(|item| item.id != id);
            sessions.len() != length
        }
        SessionChange::ObservationDegraded { id } => sessions
            .iter_mut()
            .find(|item| item.id == id)
            .map(|item| {
                item.readiness = crate::readiness::Readiness::Unknown;
                true
            })
            .unwrap_or(false),
    }
}
