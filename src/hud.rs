use std::{
    cell::Cell,
    ffi::c_void,
    path::PathBuf,
    ptr,
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
    config::Config,
    model::{ApplicationState, SessionChange},
    project::ProjectIdentity,
    readiness::Readiness,
    result_bundle::ProjectResultBundle,
    watcher::LiveWatcher,
};

const WATCHER_UPDATE: u32 = WM_APP + 1;
const COPY_BUTTON_WIDTH: f32 = 86.0;
const PROJECT_COPY_BUTTON_WIDTH: f32 = 112.0;
const CARD_GAP: f32 = 10.0;
const CARD_HEIGHT: f32 = 122.0;
const MAX_RESULT_CHARS: usize = 180;
const MAX_FILE_CHARS: usize = 42;
const MAX_VISIBLE_FILES: usize = 3;

#[derive(Clone, Copy, Debug)]
struct Click {
    x: f32,
    y: f32,
}

pub fn run(database_path: PathBuf, config: Config) -> std::process::ExitCode {
    if let Err(error) = run_window(database_path, config) {
        eprintln!("agent-hud: unable to start native HUD: {error}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}

fn run_window(database_path: PathBuf, config: Config) -> windows_canvas::Result<()> {
    let requested_dpi = Rc::new(Cell::new(96.0_f32));
    let dpi_for_message = Rc::clone(&requested_dpi);
    let (click_tx, click_rx) = mpsc::channel::<Click>();
    let window = Window::new("agent-hud — Recent local sessions")
        .size(config.window_width as i32, config.window_height as i32)
        .on_message(move |hwnd, message, wparam, lparam| {
            if message == WM_DPICHANGED {
                dpi_for_message.set((wparam & 0xffff) as f32);
            }
            if message == WM_LBUTTONUP {
                let client_x_px = (lparam & 0xffff) as i16 as f32;
                let client_y_px = ((lparam >> 16) & 0xffff) as i16 as f32;
                let _ = click_tx.send(Click {
                    x: client_x_px,
                    y: client_y_px,
                });
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
    let mut view = ViewState::default();
    let mut dirty = true;
    run_with(|| {
        while let Ok(change) = rx.try_recv() {
            dirty |= state.apply(change);
        }
        while let Ok(click) = click_rx.try_recv() {
            let (x, y) = client_px_to_dips(click.x, click.y, requested_dpi.get());
            match hit_test(&state, &view, x, y, chain.width() as f32) {
                Some(HitTarget::Project(identity)) => {
                    view.toggle_project(identity);
                    dirty = true;
                }
                Some(HitTarget::Session(id)) => dirty |= state.acknowledge(&id),
                Some(HitTarget::CopyResult(id)) => {
                    dirty |= state
                        .sessions()
                        .iter()
                        .find(|session| session.id == id)
                        .and_then(copy_payload)
                        .is_some_and(copy_to_clipboard);
                }
                Some(HitTarget::CopyProject(identity)) => {
                    dirty |= project_result_payload(&identity, &state)
                        .is_some_and(|payload| copy_to_clipboard(&payload));
                }
                None => {}
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
        match draw(&mut chain, &state, &view) {
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

fn draw(
    chain: &mut SwapChain,
    state: &ApplicationState,
    view: &ViewState,
) -> windows_canvas::Result<bool> {
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
    let project_format = TextFormat::new_bold("Segoe UI", 14.0)?;
    let row_format = TextFormat::new("Segoe UI", 13.0)?;
    let detail_format = TextFormat::new("Segoe UI", 11.0)?;
    let button_format = TextFormat::new_bold("Segoe UI", 10.0)?;
    let status_format = TextFormat::new_bold("Segoe UI", 12.0)?;
    if state.observation_degraded {
        draw_status_badge(
            &session,
            &status_format,
            &status_brush,
            Readiness::Unknown,
            false,
            row_layout(width).badge,
            20.0,
        );
    }
    let mut top = 92.0;
    for group in project_groups(state) {
        let collapsed = view.is_collapsed(group.identity);
        let project = group
            .identity
            .map(|identity| identity.normalized_name.as_str())
            .unwrap_or("(unknown project)");
        let header_layout = project_header_layout(width, top);
        session.draw_text(
            &format!(
                "{} {} ({})",
                if collapsed { "▶" } else { "▼" },
                project,
                group.sessions.len()
            ),
            &project_format,
            &header_layout.title,
            &brush,
        );
        if group
            .identity
            .and_then(|identity| project_result_payload(identity, state))
            .is_some()
        {
            draw_button(
                &session,
                &button_format,
                header_layout.copy_button,
                "Copy results",
                &brush,
            );
        }
        top += 28.0;
        if collapsed {
            continue;
        }
        for item in group.sessions {
            let layout = card_layout(width, top);
            if layout.top + CARD_HEIGHT > height {
                break;
            }
            let card_brush = session.create_solid_brush(ColorF::from_rgb8(255, 255, 255))?;
            session.fill_rounded_rect(&RoundedRect::uniform(layout.card, 8.0), &card_brush);
            let title = item
                .title
                .as_deref()
                .filter(|title| !title.is_empty())
                .unwrap_or("(untitled)");
            let title: String = title
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .take(64)
                .collect();
            session.draw_text(&title, &row_format, &layout.title, &brush);
            draw_status_badge(
                &session,
                &status_format,
                &status_brush,
                item.readiness,
                item.needs_attention,
                layout.badge,
                top + 10.0,
            );
            if let Some(result) = item.latest_result.as_deref() {
                session.draw_text(
                    &preview(result, MAX_RESULT_CHARS),
                    &detail_format,
                    &layout.result,
                    &brush,
                );
                draw_button(
                    &session,
                    &button_format,
                    layout.copy_button,
                    "Copy result",
                    &brush,
                );
            }
            let files = changed_files_preview(&item.changed_files);
            if !files.is_empty() {
                session.draw_text(&files, &detail_format, &layout.files, &brush);
            }
            if let Some(verification) = item.verification.as_ref() {
                session.draw_text(
                    &format!(
                        "Verification: {} ({})",
                        verification.command,
                        verification.outcome.as_str()
                    ),
                    &detail_format,
                    &layout.verification,
                    &brush,
                );
            }
            top += CARD_HEIGHT + CARD_GAP;
        }
        if top > height {
            break;
        }
    }
    drop(session);
    chain.present()
}

fn draw_button(
    session: &DrawingSession<'_>,
    format: &TextFormat,
    rect: Rect,
    text: &str,
    brush: &Brush,
) {
    brush.set_color(ColorF::from_rgb8(232, 239, 248));
    session.fill_rounded_rect(&RoundedRect::uniform(rect, 5.0), brush);
    brush.set_color(ColorF::from_rgb8(18, 73, 140));
    session.draw_text(text, format, &rect, brush);
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct CardLayout {
    top: f32,
    card: Rect,
    project: Rect,
    title: Rect,
    badge: TextRect,
    result: Rect,
    files: Rect,
    verification: Rect,
    copy_button: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ProjectHeaderLayout {
    title: Rect,
    copy_button: Rect,
}

fn project_header_layout(width: f32, top: f32) -> ProjectHeaderLayout {
    let right = width.max(220.0) - 16.0;
    let copy_button = Rect::new(
        right - PROJECT_COPY_BUTTON_WIDTH,
        top + 3.0,
        right,
        top + 25.0,
    );
    ProjectHeaderLayout {
        title: Rect::new(28.0, top, copy_button.left - 8.0, top + 28.0),
        copy_button,
    }
}

fn card_layout(width: f32, top: f32) -> CardLayout {
    let width = width.max(220.0);
    let right = width - 16.0;
    let badge = TextRect {
        left: right - 104.0,
        right,
    };
    let copy_button = Rect::new(right - COPY_BUTTON_WIDTH, top + 66.0, right, top + 88.0);
    CardLayout {
        top,
        card: Rect::new(16.0, top, right, top + CARD_HEIGHT),
        project: Rect::new(28.0, top + 10.0, badge.left - 12.0, top + 30.0),
        title: Rect::new(28.0, top + 31.0, badge.left - 12.0, top + 52.0),
        badge,
        result: Rect::new(28.0, top + 58.0, copy_button.left - 8.0, top + 82.0),
        files: Rect::new(28.0, top + 88.0, right - 8.0, top + 103.0),
        verification: Rect::new(28.0, top + 104.0, right - 8.0, top + 121.0),
        copy_button,
    }
}

fn client_px_to_dips(x: f32, y: f32, dpi: f32) -> (f32, f32) {
    let scale = 96.0 / dpi.max(1.0);
    (x * scale, y * scale)
}

fn preview(value: &str, max_chars: usize) -> String {
    let value: String = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    truncate(&value, max_chars)
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    let mut result: String = value.chars().take(max_chars.saturating_sub(1)).collect();
    result.push('…');
    result
}

fn changed_files_preview(files: &[String]) -> String {
    if files.is_empty() {
        return String::new();
    }
    let mut displayed = files
        .iter()
        .take(MAX_VISIBLE_FILES)
        .map(|file| truncate(&preview(file, MAX_FILE_CHARS), MAX_FILE_CHARS))
        .collect::<Vec<_>>();
    if files.len() > MAX_VISIBLE_FILES {
        displayed.push(format!("+{} more", files.len() - MAX_VISIBLE_FILES));
    }
    format!("Changed: {}", displayed.join(" · "))
}

#[cfg(windows)]
fn copy_to_clipboard(value: &str) -> bool {
    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 0x0002;
    unsafe extern "system" {
        fn OpenClipboard(window: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(format: u32, memory: *mut c_void) -> *mut c_void;
        fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
        fn GlobalFree(memory: *mut c_void) -> *mut c_void;
        fn GlobalLock(memory: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(memory: *mut c_void) -> i32;
    }
    let utf16 = value.encode_utf16().chain([0]).collect::<Vec<_>>();
    unsafe {
        if OpenClipboard(ptr::null_mut()) == 0 {
            return false;
        }
        let memory = GlobalAlloc(GMEM_MOVEABLE, utf16.len() * std::mem::size_of::<u16>());
        if memory.is_null() {
            CloseClipboard();
            return false;
        }
        let target = GlobalLock(memory) as *mut u16;
        if target.is_null() {
            GlobalFree(memory);
            CloseClipboard();
            return false;
        }
        ptr::copy_nonoverlapping(utf16.as_ptr(), target, utf16.len());
        GlobalUnlock(memory);
        if EmptyClipboard() == 0 {
            GlobalFree(memory);
            CloseClipboard();
            return false;
        }
        let clipboard_memory = SetClipboardData(CF_UNICODETEXT, memory);
        CloseClipboard();
        if clipboard_memory.is_null() {
            GlobalFree(memory);
            return false;
        }
        true
    }
}

#[cfg(not(windows))]
fn copy_to_clipboard(_value: &str) -> bool {
    false
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

fn copy_payload(session: &crate::model::SessionViewModel) -> Option<&str> {
    session.latest_result.as_deref()
}

fn project_result_payload(identity: &ProjectIdentity, state: &ApplicationState) -> Option<String> {
    let bundle = ProjectResultBundle::from_sessions(identity, state.sessions());
    matches!(&bundle, ProjectResultBundle::Available { .. }).then(|| bundle.format())
}

#[cfg(test)]
fn client_y_px_to_dips(client_y_px: f32, dpi: f32) -> f32 {
    client_y_px * 96.0 / dpi.max(1.0)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ViewState {
    collapsed_projects: Vec<Option<ProjectIdentity>>,
}

impl ViewState {
    fn is_collapsed(&self, identity: Option<&ProjectIdentity>) -> bool {
        self.collapsed_projects
            .iter()
            .any(|collapsed| collapsed.as_ref() == identity)
    }

    fn toggle_project(&mut self, identity: Option<ProjectIdentity>) {
        if let Some(index) = self
            .collapsed_projects
            .iter()
            .position(|collapsed| collapsed.as_ref() == identity.as_ref())
        {
            self.collapsed_projects.remove(index);
        } else {
            self.collapsed_projects.push(identity);
        }
    }
}

struct ProjectGroup<'a> {
    identity: Option<&'a ProjectIdentity>,
    sessions: Vec<&'a crate::model::SessionViewModel>,
}

fn project_groups(state: &ApplicationState) -> Vec<ProjectGroup<'_>> {
    let mut groups = Vec::new();
    for session in state.sessions_for_project(None) {
        let identity = session.project_identity.as_ref();
        if let Some(group) = groups
            .iter_mut()
            .find(|group: &&mut ProjectGroup<'_>| group.identity == identity)
        {
            group.sessions.push(session);
        } else {
            groups.push(ProjectGroup {
                identity,
                sessions: vec![session],
            });
        }
    }
    groups
}

enum HitTarget {
    Project(Option<ProjectIdentity>),
    Session(String),
    CopyResult(String),
    CopyProject(ProjectIdentity),
}

fn hit_test(
    state: &ApplicationState,
    view: &ViewState,
    x: f32,
    y: f32,
    width: f32,
) -> Option<HitTarget> {
    let mut top = 92.0;
    for group in project_groups(state) {
        let header_layout = project_header_layout(width, top);
        if let Some(identity) = group.identity
            && project_result_payload(identity, state).is_some()
            && x >= header_layout.copy_button.left
            && x < header_layout.copy_button.right
            && y >= header_layout.copy_button.top
            && y < header_layout.copy_button.bottom
        {
            return Some(HitTarget::CopyProject(identity.clone()));
        }
        if y >= top && y < top + 28.0 {
            return Some(HitTarget::Project(group.identity.cloned()));
        }
        top += 28.0;
        if view.is_collapsed(group.identity) {
            continue;
        }
        for session in group.sessions {
            let layout = card_layout(width, top);
            if session.latest_result.is_some()
                && x >= layout.copy_button.left
                && x < layout.copy_button.right
                && y >= layout.copy_button.top
                && y < layout.copy_button.bottom
            {
                return Some(HitTarget::CopyResult(session.id.clone()));
            }
            if y >= top && y < top + CARD_HEIGHT {
                return Some(HitTarget::Session(session.id.clone()));
            }
            top += CARD_HEIGHT + CARD_GAP;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        HitTarget, TextRect, ViewState, card_layout, changed_files_preview, client_y_px_to_dips,
        copy_payload, hit_test, preview, project_header_layout, project_result_payload, row_layout,
    };
    use crate::{
        model::{ApplicationState, SessionChange, SessionViewModel},
        project::ProjectIdentity,
        readiness::Readiness,
        verification::{VerificationEvidence, VerificationOutcome},
    };

    fn session(id: &str, recency_at_ms: i64) -> SessionViewModel {
        SessionViewModel {
            id: id.into(),
            title: None,
            latest_result: None,
            project_identity: None,
            changed_files: Vec::new(),
            readiness: Readiness::Ready,
            needs_attention: false,
            recency_at_ms,
            verification: None,
        }
    }

    fn rich_session(id: &str) -> SessionViewModel {
        SessionViewModel {
            latest_result: Some("full result with details that must remain copyable".into()),
            changed_files: vec![
                "src/one.rs".into(),
                "src/two.rs".into(),
                "src/three.rs".into(),
                "src/four.rs".into(),
            ],
            verification: Some(VerificationEvidence {
                command: "cargo test".into(),
                outcome: VerificationOutcome::Passed,
            }),
            ..session(id, 1)
        }
    }

    fn project(name: &str, root: &str, repository: Option<&str>) -> ProjectIdentity {
        ProjectIdentity {
            normalized_name: name.into(),
            root_path: Some(root.into()),
            repository_identity: repository.map(str::to_owned),
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
        assert!(matches!(
            hit_test(&state, &ViewState::default(), 0.0, 122.0, 620.0),
            Some(HitTarget::Session(id)) if id == "first"
        ));
    }

    #[test]
    fn hit_testing_uses_dips_at_144_dpi() {
        let state = two_rows();

        assert_eq!(client_y_px_to_dips(183.0, 144.0), 122.0);
        assert!(matches!(
            hit_test(&state, &ViewState::default(), 0.0, 122.0, 620.0),
            Some(HitTarget::Session(id)) if id == "first"
        ));
    }

    #[test]
    fn sessions_are_grouped_by_full_project_identity() {
        let mut state = ApplicationState::default();
        let project = ProjectIdentity {
            normalized_name: "same-name".into(),
            root_path: Some("C:\\one".into()),
            repository_identity: None,
        };
        let other_project = ProjectIdentity {
            root_path: Some("C:\\two".into()),
            ..project.clone()
        };
        let mut first = session("first", 3);
        first.project_identity = Some(project.clone());
        let mut second = session("second", 2);
        second.project_identity = Some(project);
        let mut third = session("third", 1);
        third.project_identity = Some(other_project);
        state.apply(SessionChange::Snapshot(vec![third, first, second]));

        let groups = super::project_groups(&state);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].sessions.len(), 2);
        assert_eq!(groups[1].sessions.len(), 1);
    }

    #[test]
    fn project_header_and_session_hit_testing_follow_expansion_state() {
        let mut state = ApplicationState::default();
        let project = ProjectIdentity {
            normalized_name: "agent-hud".into(),
            root_path: None,
            repository_identity: None,
        };
        let mut item = session("first", 1);
        item.project_identity = Some(project.clone());
        state.apply(SessionChange::Snapshot(vec![item]));

        assert!(matches!(
            hit_test(&state, &ViewState::default(), 0.0, 92.0, 620.0),
            Some(HitTarget::Project(Some(identity))) if identity == project
        ));
        let mut view = ViewState::default();
        view.toggle_project(Some(project.clone()));
        assert!(matches!(
            hit_test(&state, &view, 0.0, 100.0, 620.0),
            Some(HitTarget::Project(Some(identity))) if identity == project
        ));
        assert!(hit_test(&state, &view, 0.0, 120.0, 620.0).is_none());
    }

    #[test]
    fn project_copy_hit_target_is_distinct_from_collapse_and_uses_full_bundle() {
        let selected = project("agent-hud", r"C:\worktrees\one", Some("repo:one"));
        let linked = project("agent-hud", r"C:\worktrees\two", Some("repo:one"));
        let mut first = rich_session("first");
        first.project_identity = Some(selected.clone());
        first.latest_result = Some("first full result".into());
        let mut second = rich_session("second");
        second.project_identity = Some(linked);
        second.latest_result = Some("second full result with details".into());

        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![first, second]));
        let header = project_header_layout(620.0, 92.0);
        let click = (header.copy_button.left + 2.0, header.copy_button.top + 2.0);

        assert!(
            project_result_payload(&selected, &state)
                .is_some_and(|payload| payload.contains("second full result with details"))
        );
        assert!(matches!(
            hit_test(&state, &ViewState::default(), click.0, click.1, 620.0),
            Some(HitTarget::CopyProject(identity)) if identity == selected
        ));
        assert!(matches!(
            hit_test(&state, &ViewState::default(), 28.0, 92.0, 620.0),
            Some(HitTarget::Project(Some(identity))) if identity == selected
        ));
    }

    #[test]
    fn project_copy_does_not_mix_same_name_with_different_repository() {
        let selected = project("agent-hud", r"C:\one", Some("repo:one"));
        let other = project("agent-hud", r"C:\two", Some("repo:two"));
        let mut included = rich_session("included");
        included.project_identity = Some(selected.clone());
        included.latest_result = Some("included result".into());
        let mut excluded = rich_session("excluded");
        excluded.project_identity = Some(other);
        excluded.latest_result = Some("excluded result".into());

        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![included, excluded]));
        let payload = project_result_payload(&selected, &state).unwrap();
        assert!(payload.contains("included result"));
        assert!(!payload.contains("excluded result"));
    }

    #[test]
    fn project_copy_is_unavailable_when_all_results_are_empty() {
        let selected = project("agent-hud", r"C:\agent-hud", Some("repo:one"));
        let mut empty = session("empty", 1);
        empty.project_identity = Some(selected.clone());
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![empty]));

        assert!(project_result_payload(&selected, &state).is_none());
        assert!(hit_test(&state, &ViewState::default(), 520.0, 98.0, 620.0).is_some_and(
            |target| matches!(target, HitTarget::Project(Some(identity)) if identity == selected)
        ));
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

    #[test]
    fn result_preview_is_bounded_but_copy_payload_is_full() {
        let value = "a".repeat(200);
        assert_eq!(preview(&value, 10).chars().count(), 10);
        let item = rich_session("result");
        assert_eq!(copy_payload(&item), item.latest_result.as_deref());
        assert!(copy_payload(&item).unwrap().len() > 10);
    }

    #[test]
    fn changed_files_are_bounded_with_remaining_count() {
        let files = vec![
            "one.rs".into(),
            "two.rs".into(),
            "three.rs".into(),
            "four.rs".into(),
        ];
        let text = changed_files_preview(&files);
        assert!(text.contains("one.rs"));
        assert!(text.contains("three.rs"));
        assert!(!text.contains("four.rs"));
        assert!(text.contains("+1 more"));
    }

    #[test]
    fn card_layout_stays_inside_narrow_width_and_copy_is_a_distinct_action() {
        let layout = card_layout(240.0, 120.0);
        assert!(layout.card.right <= 240.0);
        assert!(layout.copy_button.left >= layout.result.left);
        let mut state = ApplicationState::default();
        state.apply(SessionChange::Snapshot(vec![rich_session("result")]));
        let click = (layout.copy_button.left + 2.0, layout.copy_button.top + 2.0);
        assert!(matches!(
            hit_test(&state, &ViewState::default(), click.0, click.1, 240.0),
            Some(HitTarget::CopyResult(id)) if id == "result"
        ));
    }

    #[test]
    fn metadata_does_not_change_readiness_and_acknowledgement_still_works() {
        let mut state = ApplicationState::default();
        let mut working = rich_session("result");
        working.readiness = Readiness::Working;
        state.apply(SessionChange::Snapshot(vec![working.clone()]));
        let mut ready = working;
        ready.readiness = Readiness::Ready;
        state.apply(SessionChange::Updated(ready));
        assert_eq!(state.sessions()[0].readiness, Readiness::Ready);
        assert!(state.sessions()[0].needs_attention);
        assert!(state.acknowledge("result"));
        assert!(!state.sessions()[0].needs_attention);
    }

    #[test]
    fn absent_metadata_is_quiet() {
        let item = session("empty", 1);
        assert_eq!(preview(item.latest_result.as_deref().unwrap_or(""), 20), "");
        assert_eq!(changed_files_preview(&item.changed_files), "");
        assert!(item.verification.is_none());
    }
}
