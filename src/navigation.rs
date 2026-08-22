//! Native Windows navigation to an existing Codex thread.
//!
//! This module only hands a documented deep-link to the Windows URI handler.
//! A successful return means that Windows accepted the dispatch; it does not
//! establish that Codex focused a particular window or completed navigation.

const THREAD_URI_PREFIX: &str = "codex://threads/";

#[derive(Debug, Eq, PartialEq)]
pub enum NavigationError {
    InvalidThreadId,
    #[cfg(not(windows))]
    UnsupportedPlatform,
    WindowsDispatchFailed(u32),
}

/// Build the documented URI for an existing Codex thread.
pub fn thread_uri(thread_id: &str) -> Result<String, NavigationError> {
    if !is_valid_thread_id(thread_id) {
        return Err(NavigationError::InvalidThreadId);
    }
    Ok(format!("{THREAD_URI_PREFIX}{thread_id}"))
}

/// Ask Windows to dispatch an existing Codex thread deep-link.
pub fn open_thread(thread_id: &str) -> Result<(), NavigationError> {
    let uri = thread_uri(thread_id)?;
    launch_uri(&uri)
}

fn is_valid_thread_id(thread_id: &str) -> bool {
    !thread_id.is_empty()
        && thread_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

#[cfg(windows)]
fn launch_uri(uri: &str) -> Result<(), NavigationError> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

    use windows_sys::Win32::{UI::Shell::ShellExecuteW, UI::WindowsAndMessaging::SW_SHOWNORMAL};

    let operation: Vec<u16> = OsStr::new("open")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let uri: Vec<u16> = OsStr::new(uri)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // ShellExecuteW returns a value greater than 32 when the request was
    // handed to the registered URI handler.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            uri.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as usize;
    if result <= 32 {
        return Err(NavigationError::WindowsDispatchFailed(result as u32));
    }
    Ok(())
}

#[cfg(not(windows))]
fn launch_uri(_uri: &str) -> Result<(), NavigationError> {
    Err(NavigationError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::{NavigationError, open_thread, thread_uri};

    #[test]
    fn constructs_existing_thread_uri() {
        assert_eq!(
            thread_uri("8f5d0a4e-7f1c-4ca4-9d8b-0123456789ab"),
            Ok("codex://threads/8f5d0a4e-7f1c-4ca4-9d8b-0123456789ab".to_owned())
        );
    }

    #[test]
    fn rejects_values_that_cannot_be_one_uri_path_segment() {
        for thread_id in [
            "",
            " ",
            "thread/id",
            "thread?query",
            "thread#fragment",
            "тред",
        ] {
            assert_eq!(thread_uri(thread_id), Err(NavigationError::InvalidThreadId));
        }
    }

    #[test]
    fn invalid_thread_id_fails_before_platform_launch() {
        assert_eq!(
            open_thread("not a thread id"),
            Err(NavigationError::InvalidThreadId)
        );
    }
}
