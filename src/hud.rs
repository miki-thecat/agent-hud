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
    UI::WindowsAndMessaging::{PostMessageW, WM_APP, WM_DPICHANGED},
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
    let window = Window::new("agent-hud — Recent local sessions")
        .size(620, 720)
        .on_message(move |_hwnd, message, wparam, _lparam| {
            if message == WM_DPICHANGED {
                dpi_for_message.set((wparam & 0xffff) as f32);
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
    let row_format = TextFormat::new("Segoe UI", 15.0)?;
    let status_format = TextFormat::new_bold("Segoe UI", 12.0)?;
    let status_left = (width - 136.0).max(200.0);
    if state.observation_degraded {
        draw_status_badge(
            &session,
            &status_format,
            &status_brush,
            Readiness::Unknown,
            status_left,
            20.0,
        );
    }
    for (index, item) in state.sessions().iter().enumerate() {
        let top = 92.0 + index as f32 * 34.0;
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
        session.draw_text(
            &label,
            &row_format,
            &Rect::new(28.0, top, status_left - 16.0, top + 28.0),
            &brush,
        );
        draw_status_badge(
            &session,
            &status_format,
            &status_brush,
            item.readiness,
            status_left,
            top + 4.0,
        );
        if top + 34.0 > height {
            break;
        }
    }
    drop(session);
    chain.present()
}

fn draw_status_badge(
    session: &DrawingSession<'_>,
    format: &TextFormat,
    brush: &Brush,
    readiness: Readiness,
    left: f32,
    top: f32,
) {
    let (background, foreground) = match readiness {
        Readiness::Working => (
            ColorF::from_rgb8(219, 234, 254),
            ColorF::from_rgb8(18, 73, 140),
        ),
        Readiness::Ready => (
            ColorF::from_rgb8(220, 244, 228),
            ColorF::from_rgb8(24, 105, 54),
        ),
        Readiness::Unknown => (
            ColorF::from_rgb8(255, 237, 196),
            ColorF::from_rgb8(126, 77, 0),
        ),
    };
    brush.set_color(background);
    session.fill_rounded_rect(
        &RoundedRect::uniform(Rect::new(left, top, left + 112.0, top + 24.0), 7.0),
        brush,
    );
    brush.set_color(foreground);
    session.draw_text(
        readiness.as_str(),
        format,
        &Rect::new(left + 10.0, top + 3.0, left + 106.0, top + 22.0),
        brush,
    );
}
