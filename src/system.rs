//! The only outbound action GLYPH can take: opening the project channel.
//! The address is a compile-time constant, so no generated value, length,
//! entropy figure or clipboard content can ever reach the shell or the URL.

use crate::TELEGRAM_URL;

/// Windows 11 rounds the corners of top-level windows by default, including
/// undecorated ones. The SNATCH/RAZE family is square, so ask DWM not to
/// round this window. Windows 10 ignores the attribute.
#[cfg(windows)]
pub fn square_window_corners(cc: &eframe::CreationContext<'_>) {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_WINDOW_CORNER_PREFERENCE, DwmSetWindowAttribute,
    };
    let Ok(handle) = cc.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(window) = handle.as_raw() else {
        return;
    };
    // DWMWCP_DONOTROUND
    let preference: u32 = 1;
    // Windows 11 otherwise paints a one-pixel system-coloured border around
    // the window, which reads as a light line on a near-black theme.
    // DWMWA_COLOR_NONE removes it; Windows 10 ignores the attribute.
    let no_border: u32 = DWMWA_COLOR_NONE;
    // SAFETY: the HWND comes from eframe's live window; both DWORDs outlive
    // the calls and each attribute expects exactly a DWORD.
    unsafe {
        DwmSetWindowAttribute(
            window.hwnd.get() as *mut core::ffi::c_void,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            (&preference as *const u32).cast::<core::ffi::c_void>(),
            std::mem::size_of::<u32>() as u32,
        );
        DwmSetWindowAttribute(
            window.hwnd.get() as *mut core::ffi::c_void,
            DWMWA_BORDER_COLOR as u32,
            (&no_border as *const u32).cast::<core::ffi::c_void>(),
            std::mem::size_of::<u32>() as u32,
        );
    }
}

#[cfg(not(windows))]
pub fn square_window_corners(_: &eframe::CreationContext<'_>) {}

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
