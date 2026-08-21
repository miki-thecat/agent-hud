use std::{
    cell::Cell,
    path::PathBuf,
    rc::Rc,
    sync::mpsc::{self, Sender},
    thread,
};

use windows_canvas::*;
use windows_sys::Win32::{
    UI::HiDpi::GetDpiForWindow,
    UI::WindowsAndMessaging::{PostMessageW, WM_APP, WM_DPICHANGED, WM_LBUTTONUP},
};
use windows_window::*;

use crate::{
    model::{ApplicationState, SessionChange},
    readiness::Readiness,
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
    let requested_dpi = Rc::new(Cell::new(96.0_f32));
    let dpi_for_message = Rc::clone(&requested_dpi);
    let (click_tx, click_rx) = mpsc::channel::<f32>();
    let window = Window::new("agent-hud — Recent local sessions")
        .size(620, 720)
        .on_message(move |hwnd, message, wparam, lparam| {
            if message == WM_DPICHANGED {
                dpi_for_message.set((wparam & 0xffff) as f32);
            }
            if message == WM_LBUTTONUP {
                let client_y_px = ((lparam >> 16) & 0xffff) as i16 as f32;
                let _ = click_tx.send(client_y_px);
                post_wake(hwnd as usize);
            }
            None
        })
        .create()?;
    let initial_dpi = unsafe { GetDpiForWindow(window.hwnd() as _) };
    requested_dpi.set(if initial_dpi == 0 {
        96.0
    } else {
        initial_dpi as f32
    });
    let hwnd = window.hwnd() as usize;
    let (tx, rx) = mpsc::channel();
    let sessions_dir = database_path
        .parent()
        .map(|path| path.join("sessions"))
        .unwrap_or_else(|| PathBuf::from("sessions"));
    thread::spawn(move || {
        let mut watcher = match LiveWatcher::new(database_path, sessions_dir) {
            Ok(watcher) => watcher,
            Err(_) => {
                send_change(&tx, hwnd, SessionChange::ObservationTerminated);
                return;
            }
        };
        for change in watcher.initial_changes() {
            let _ = tx.send(change);
        }
        post_wake(hwnd);
        let events = match watcher.watch() {
            Ok(events) => events,
            Err(_) => {
                send_changes(&tx, hwnd, watcher.degrade());
                return;
            }
        };
        for event in events {
            let changes = match event {
                Ok(event) => match watcher.handle_event(&event) {
                    Ok(changes) => changes,
                    Err(_) => {
                        send_changes(&tx, hwnd, watcher.degrade());
                        return;
                    }
                },
                Err(_) => {
                    send_changes(&tx, hwnd, watcher.degrade());
                    return;
                }
            };
            send_changes(&tx, hwnd, changes);
        }
        send_changes(&tx, hwnd, watcher.degrade());
    });

    let (width, height) = window.client_size();
    let mut device = GpuDevice::new_or_warp()?;
    let mut chain = create_chain(
        &device,
        &window,
        width as u32,
        height as u32,
        requested_dpi.get(),
    )?;
    let mut state = ApplicationState::default();
    let mut applied_dpi = requested_dpi.get();
    let mut retry_after_recreate = false;
    let mut dirty = true;
    run_with(|| {
        while let Ok(change) = rx.try_recv() {
            dirty |= state.apply(change);
        }
        while let Ok(y) = click_rx.try_recv() {
            if let Some(item) = session_at_client_y(&state, y, requested_dpi.get()) {
                dirty |= state.acknowledge(&item);
            }
        }
        let (width, height) = window.client_size();
        if width as u32 != chain.width() || height as u32 != chain.height() {
            chain.resize(width as u32, height as u32)?;
            dirty = true;
        }
        let dpi = requested_dpi.get();
        if dpi != applied_dpi {
            applied_dpi = dpi;
            dirty = true;
        }
        chain.set_dpi(dpi, dpi);
        // This is a raw HWND swap chain, not a composition surface. Keep the
        // composition transform at identity; set_dpi carries the monitor DPI.
        chain.set_composition_scale(1.0, 1.0);
        if !dirty {
            return Ok(false);
        }
        match draw(&mut chain, &state) {
            Ok(true) => {
                retry_after_recreate = false;
            }
            Ok(false) => {
                if retry_after_recreate {
                    return Err(device_lost_error());
                }
                device = GpuDevice::new_or_warp()?;
                chain = create_chain(
                    &device,
                    &window,
                    width as u32,
                    height as u32,
                    requested_dpi.get(),
                )?;
                applied_dpi = requested_dpi.get();
                retry_after_recreate = true;
                return Ok(true);
            }
            Err(error) if is_device_lost(error.code()) || chain.is_device_lost() => {
                if retry_after_recreate {
                    return Err(device_lost_error());
                }
                device = GpuDevice::new_or_warp()?;
                chain = create_chain(
                    &device,
                    &window,
                    width as u32,
                    height as u32,
                    requested_dpi.get(),
                )?;
                applied_dpi = requested_dpi.get();
                retry_after_recreate = true;
                return Ok(true);
            }
            Err(error) => return Err(error),
        }
        dirty = false;
        Ok(false)
    })
}

fn send_changes(
    tx: &Sender<SessionChange>,
    hwnd: usize,
    changes: impl IntoIterator<Item = SessionChange>,
) {
    for change in changes {
        let _ = tx.send(change);
    }
    post_wake(hwnd);
}

fn send_change(tx: &Sender<SessionChange>, hwnd: usize, change: SessionChange) {
    let _ = tx.send(change);
    post_wake(hwnd);
}

fn post_wake(hwnd: usize) {
    unsafe {
        PostMessageW(hwnd as _, WATCHER_UPDATE, 0, 0);
    }
}

fn create_chain(
    device: &GpuDevice,
    window: &Window,
    width: u32,
    height: u32,
    dpi: f32,
) -> windows_canvas::Result<SwapChain> {
    let mut chain = device.create_swap_chain_for_window(window, width, height)?;
    chain.set_dpi(dpi, dpi);
    Ok(chain)
}

fn draw(chain: &mut SwapChain, state: &ApplicationState) -> windows_canvas::Result<bool> {
    let width = chain.width() as f32;
    let height = chain.height() as f32;
    let session = chain.begin_draw()?;
    session.clear(ColorF::from_rgb8(248, 249, 251));
    let brush = session.create_solid_brush(ColorF::from_rgb8(30, 35, 42))?;
    let status_brush = session.create_solid_brush(ColorF::from_rgb8(30, 35, 42))?;
    if state.observation_degraded {
        status_brush.set_color(ColorF::from_rgb8(126, 77, 0));
    }
    let header = TextFormat::new_bold("Segoe UI", 22.0)?;
    let title = if state.observation_degraded {
        "Observation unavailable"
    } else {
        "Recent local sessions"
    };
    session.draw_text(
        title,
        &header,
        &Rect::new(24.0, 18.0, width - 24.0, 52.0),
        if state.observation_degraded {
            &status_brush
        } else {
            &brush
        },
    );
    let subhead = TextFormat::new("Segoe UI", 12.0)?;
    session.draw_text(
        "Recorded local readiness — not exact open chats",
        &subhead,
        &Rect::new(24.0, 52.0, width - 24.0, 76.0),
        &brush,
    );
    let project_format = TextFormat::new_bold("Segoe UI", 15.0)?;
    let row_format = TextFormat::new("Segoe UI", 13.0)?;
    let status_format = TextFormat::new_bold("Segoe UI", 12.0)?;
    let row_layout = row_layout(width);
    if state.observation_degraded {
        draw_status_badge(
            &session,
            &status_format,
            &status_brush,
            Readiness::Unknown,
            false,
            row_layout.badge,
            20.0,
        );
    }
    for (index, item) in state.sessions().iter().enumerate() {
        let top = 92.0 + index as f32 * 30.0;
        let title = item
            .title
            .as_deref()
            .filter(|title| !title.is_empty())
            .unwrap_or("(untitled)");
        let verification_suffix = item
            .verification
            .as_ref()
            .map(|evidence| format!(" · verify {}", evidence.outcome.as_str()))
            .unwrap_or_default();
        let files_suffix = if item.readiness == Readiness::Ready && !item.changed_files.is_empty() {
            format!(" · files: {}", item.changed_files.join(", "))
        } else {
            String::new()
        };
        let title: String = format!("{title}{verification_suffix}{files_suffix}")
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .take(64)
            .collect();
        if let Some(project_rect) = row_layout.project {
            let project = item.project_label.as_deref().unwrap_or_default();
            session.draw_text(
                project,
                &project_format,
                &Rect::new(project_rect.left, top, project_rect.right, top + 28.0),
                &brush,
            );
        }
        if let Some(title_rect) = row_layout.title {
            session.draw_text(
                &title,
                &row_format,
                &Rect::new(title_rect.left, top + 2.0, title_rect.right, top + 26.0),
                &brush,
            );
        }
        draw_status_badge(
            &session,
            &status_format,
            &status_brush,
            item.readiness,
            item.needs_attention,
            row_layout.badge,
            top + 4.0,
        );
        if top + 30.0 > height {
            break;
        }
    }
    drop(session);
    chain.present()
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TextRect {
    left: f32,
    right: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RowLayout {
    badge: TextRect,
    project: Option<TextRect>,
    title: Option<TextRect>,
}

fn row_layout(width: f32) -> RowLayout {
    let width = width.max(0.0);
    let badge_width = width.min(112.0);
    let badge_left = width - badge_width;
    let text_right = badge_left - 16.0;
    let project = (text_right > 28.0).then_some(TextRect {
        left: 28.0,
        right: text_right.min(150.0),
    });
    let title = (text_right > 158.0).then_some(TextRect {
        left: 158.0,
        right: text_right,
    });
    RowLayout {
        badge: TextRect {
            left: badge_left,
            right: badge_left + badge_width,
        },
        project,
        title,
    }
}

fn draw_status_badge(
    session: &DrawingSession<'_>,
    format: &TextFormat,
    brush: &Brush,
    readiness: Readiness,
    needs_attention: bool,
    badge: TextRect,
    top: f32,
) {
    let (background, foreground) = if readiness == Readiness::Ready && needs_attention {
        (
            ColorF::from_rgb8(255, 220, 220),
            ColorF::from_rgb8(170, 24, 24),
        )
    } else {
        match readiness {
            Readiness::Working => (
                ColorF::from_rgb8(219, 234, 254),
                ColorF::from_rgb8(18, 73, 140),
            ),
            Readiness::Ready => (
                ColorF::from_rgb8(219, 234, 254),
                ColorF::from_rgb8(18, 73, 140),
            ),
            Readiness::Unknown => (
                ColorF::from_rgb8(255, 237, 196),
                ColorF::from_rgb8(126, 77, 0),
            ),
        }
    };
    brush.set_color(background);
    session.fill_rounded_rect(
        &RoundedRect::uniform(Rect::new(badge.left, top, badge.right, top + 24.0), 7.0),
        brush,
    );
    brush.set_color(foreground);
    session.draw_text(
        readiness.as_str(),
        format,
        &Rect::new(
            badge.left + 10.0,
            top + 3.0,
            (badge.right - 6.0).max(badge.left + 10.0),
            top + 22.0,
        ),
        brush,
    );
}

fn session_at_client_y(state: &ApplicationState, client_y_px: f32, dpi: f32) -> Option<String> {
    session_at_y(state, client_y_px_to_dips(client_y_px, dpi))
}

fn client_y_px_to_dips(client_y_px: f32, dpi: f32) -> f32 {
    client_y_px * 96.0 / dpi.max(1.0)
}

fn session_at_y(state: &ApplicationState, y: f32) -> Option<String> {
    state
        .sessions()
        .iter()
        .enumerate()
        .find(|(index, _)| {
            let top = 92.0 + *index as f32 * 30.0;
            y >= top && y < top + 30.0
        })
        .map(|(_, item)| item.id.clone())
}

#[cfg(test)]
mod tests {
    use super::{TextRect, client_y_px_to_dips, row_layout, session_at_client_y};
    use crate::{
        model::{ApplicationState, SessionChange, SessionViewModel},
        readiness::Readiness,
    };

    fn session(id: &str, recency_at_ms: i64) -> SessionViewModel {
        SessionViewModel {
            id: id.into(),
            title: None,
            latest_result: None,
            project_label: None,
            changed_files: Vec::new(),
            readiness: Readiness::Ready,
            needs_attention: false,
            recency_at_ms,
            verification: None,
        }
    }

    fn two_rows() -> ApplicationState {
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![
            session("first", 2),
            session("second", 1),
        ]));
        state
    }

    #[test]
    fn hit_testing_uses_dips_at_96_dpi() {
        let state = two_rows();

        assert_eq!(client_y_px_to_dips(122.0, 96.0), 122.0);
        assert_eq!(
            session_at_client_y(&state, 122.0, 96.0).as_deref(),
            Some("second")
        );
    }

    #[test]
    fn hit_testing_uses_dips_at_144_dpi() {
        let state = two_rows();

        assert_eq!(client_y_px_to_dips(183.0, 144.0), 122.0);
        assert_eq!(
            session_at_client_y(&state, 183.0, 144.0).as_deref(),
            Some("second")
        );
    }

    #[test]
    fn normal_width_keeps_project_title_and_badge_separated() {
        let layout = row_layout(620.0);

        assert_eq!(
            layout.badge,
            TextRect {
                left: 508.0,
                right: 620.0
            }
        );
        assert_eq!(
            layout.project,
            Some(TextRect {
                left: 28.0,
                right: 150.0
            })
        );
        assert_eq!(
            layout.title,
            Some(TextRect {
                left: 158.0,
                right: 492.0
            })
        );
        assert!(layout.title.unwrap().right < layout.badge.left);
    }

    #[test]
    fn narrow_width_omits_text_before_it_can_invert_or_overlap_badge() {
        let layout = row_layout(160.0);

        assert_eq!(
            layout.badge,
            TextRect {
                left: 48.0,
                right: 160.0
            }
        );
        assert_eq!(
            layout.project,
            Some(TextRect {
                left: 28.0,
                right: 32.0
            })
        );
        assert_eq!(layout.title, None);
        assert!(
            layout
                .project
                .is_none_or(|rect| rect.right <= layout.badge.left)
        );
    }
}
