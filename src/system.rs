//! The only outbound action GLYPH can take: opening the project channel.
//! The address is a compile-time constant, so no generated value, length,
//! entropy figure or clipboard content can ever reach the shell or the URL.

use crate::TELEGRAM_URL;

/// Opens the fixed channel URL in the default browser.
/// Returns `false` when the shell rejected the request; the UI then shows the
/// address instead of pretending that a browser opened.
pub fn open_telegram() -> bool {
    open_https(TELEGRAM_URL)
}

#[cfg(windows)]
fn open_https(url: &str) -> bool {
    use windows_sys::{
        Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        core::w,
    };

    // Defence in depth for the constant above: this helper must never become a
    // generic "open anything" primitive that a future call site could point at
    // secret-bearing text.
    if !url.starts_with("https://t.me/") || url.chars().any(char::is_whitespace) {
        return false;
    }
    let wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `wide` is NUL-terminated and alive for the call, the operation
    // and directory arguments are static/absent, and a URL needs no HWND.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            w!("open"),
            wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecute returns a value greater than 32 on success.
    result as isize > 32
}

#[cfg(not(windows))]
fn open_https(_: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_url_is_fixed_https_without_payload() {
        assert_eq!(TELEGRAM_URL, "https://t.me/rercon");
        assert_eq!(crate::TELEGRAM_HANDLE, "t.me/rercon");
        assert!(TELEGRAM_URL.starts_with("https://t.me/"));
        assert!(
            !TELEGRAM_URL
                .chars()
                .any(|ch| ch.is_whitespace() || matches!(ch, '?' | '#'))
        );
    }

    #[cfg(windows)]
    #[test]
    fn opener_refuses_anything_but_the_channel_prefix() {
        // Must reject before ShellExecuteW, so no browser is launched by tests.
        assert!(!open_https("https://example.com/"));
        assert!(!open_https("file:///C:/Windows/System32/calc.exe"));
        assert!(!open_https("https://t.me/rercon extra"));
    }
}
