//! Timed, ownership-aware clipboard writes. Never erase a later copy.
use std::time::{Duration, Instant};

pub const LIFETIME: Duration = Duration::from_secs(30);
const RETRY_WINDOW: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardError {
    Unavailable,
}

impl std::fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Буфер обмена занят или недоступен. Попробуйте ещё раз.")
    }
}
impl std::error::Error for ClipboardError {}

trait Backend {
    fn write(&mut self, text: &str) -> Result<u32, ClipboardError>;
    /// Compare sequence while holding the OS clipboard lock, then clear.
    fn clear_if_current(&mut self, sequence: u32) -> Result<bool, ClipboardError>;
}

#[derive(Clone, Copy)]
struct Lease {
    sequence: u32,
    deadline: Instant,
    next_attempt: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PollResult {
    Idle,
    Cleared,
    Superseded,
    Failed,
}

struct TimedClipboard<B: Backend> {
    backend: B,
    lease: Option<Lease>,
}

impl<B: Backend> TimedClipboard<B> {
    fn copy(&mut self, text: &str, now: Instant) -> Result<(), ClipboardError> {
        let sequence = self.backend.write(text)?;
        self.lease = Some(Lease {
            sequence,
            deadline: now + LIFETIME,
            next_attempt: now + LIFETIME,
        });
        Ok(())
    }

    fn poll(&mut self, now: Instant) -> PollResult {
        let Some(lease) = self.lease else {
            return PollResult::Idle;
        };
        if now < lease.next_attempt {
            return PollResult::Idle;
        }
        match self.backend.clear_if_current(lease.sequence) {
            Ok(current) => {
                self.lease = None;
                if current {
                    PollResult::Cleared
                } else {
                    PollResult::Superseded
                }
            }
            Err(_) if now < lease.deadline + RETRY_WINDOW => {
                if let Some(lease) = self.lease.as_mut() {
                    lease.next_attempt = now + Duration::from_millis(200);
                }
                PollResult::Idle
            }
            Err(_) => {
                self.lease = None;
                PollResult::Failed
            }
        }
    }

    fn remaining(&self, now: Instant) -> Option<Duration> {
        self.lease
            .map(|lease| lease.next_attempt.saturating_duration_since(now))
    }
}

impl<B: Backend> Drop for TimedClipboard<B> {
    fn drop(&mut self) {
        if let Some(lease) = self.lease.take() {
            let _ = self.backend.clear_if_current(lease.sequence);
        }
    }
}

pub struct Clipboard(TimedClipboard<NativeClipboard>);

impl Default for Clipboard {
    fn default() -> Self {
        Self(TimedClipboard {
            backend: NativeClipboard::default(),
            lease: None,
        })
    }
}
impl Clipboard {
    pub fn copy(&mut self, text: &str) -> Result<(), ClipboardError> {
        self.0.copy(text, Instant::now())
    }
    pub fn poll(&mut self) -> PollResult {
        self.0.poll(Instant::now())
    }
    pub fn next_check(&self) -> Option<Duration> {
        self.0.remaining(Instant::now())
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::ptr::{copy_nonoverlapping, null, null_mut};
    use windows_sys::Win32::{
        Foundation::{GlobalFree, HANDLE, HWND},
        System::{
            DataExchange::{
                CloseClipboard, EmptyClipboard, GetClipboardSequenceNumber, OpenClipboard,
                RegisterClipboardFormatW, SetClipboardData,
            },
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
        },
        UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, HWND_MESSAGE},
    };
    use windows_sys::core::w;
    use zeroize::{Zeroize, Zeroizing};

    struct Open;
    impl Open {
        fn new(owner: HWND) -> Result<Self, ClipboardError> {
            // SAFETY: owner belongs to this UI thread and outlives this lock.
            if unsafe { OpenClipboard(owner) } == 0 {
                Err(ClipboardError::Unavailable)
            } else {
                Ok(Self)
            }
        }
    }
    impl Drop for Open {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
            }
        }
    }

    struct Allocation {
        handle: HANDLE,
        size: usize,
    }
    impl Allocation {
        fn new(bytes: &[u8]) -> Result<Self, ClipboardError> {
            let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) };
            if handle.is_null() {
                return Err(ClipboardError::Unavailable);
            }
            let allocation = Self {
                handle,
                size: bytes.len(),
            };
            let ptr = unsafe { GlobalLock(handle) };
            if ptr.is_null() {
                return Err(ClipboardError::Unavailable);
            }
            // SAFETY: GlobalAlloc reserved bytes.len() and the handle is locked.
            unsafe {
                copy_nonoverlapping(bytes.as_ptr(), ptr.cast::<u8>(), bytes.len());
                GlobalUnlock(handle);
            }
            Ok(allocation)
        }
        fn transfer(self, format: u32) -> Result<(), ClipboardError> {
            // Ownership transfers only after SetClipboardData succeeds.
            if unsafe { SetClipboardData(format, self.handle) }.is_null() {
                return Err(ClipboardError::Unavailable);
            }
            std::mem::forget(self);
            Ok(())
        }
    }
    impl Drop for Allocation {
        fn drop(&mut self) {
            unsafe {
                let ptr = GlobalLock(self.handle);
                if !ptr.is_null() {
                    std::slice::from_raw_parts_mut(ptr.cast::<u8>(), self.size).zeroize();
                    GlobalUnlock(self.handle);
                }
                GlobalFree(self.handle);
            }
        }
    }

    pub(super) struct NativeClipboard {
        owner: HWND,
    }
    impl Default for NativeClipboard {
        fn default() -> Self {
            Self { owner: null_mut() }
        }
    }
    impl NativeClipboard {
        fn owner(&mut self) -> Result<HWND, ClipboardError> {
            if self.owner.is_null() {
                // A message-only window gives the clipboard a non-null owner;
                // OpenClipboard(NULL) + EmptyClipboard can break SetClipboardData.
                self.owner = unsafe {
                    CreateWindowExW(
                        0,
                        w!("STATIC"),
                        w!("GLYPH clipboard"),
                        0,
                        0,
                        0,
                        0,
                        0,
                        HWND_MESSAGE,
                        null_mut(),
                        null_mut(),
                        null(),
                    )
                };
            }
            if self.owner.is_null() {
                Err(ClipboardError::Unavailable)
            } else {
                Ok(self.owner)
            }
        }
    }
    impl Backend for NativeClipboard {
        fn write(&mut self, text: &str) -> Result<u32, ClipboardError> {
            let mut wide = Zeroizing::new(Vec::with_capacity(text.encode_utf16().count() + 1));
            wide.extend(text.encode_utf16());
            wide.push(0);
            // SAFETY: u16 is plain data and this read-only view fits wide exactly.
            let bytes =
                unsafe { std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * 2) };
            let allocation = Allocation::new(bytes)?;
            let _lock = Open::new(self.owner()?)?;
            if unsafe { EmptyClipboard() } == 0 {
                return Err(ClipboardError::Unavailable);
            }
            // Opt this write out of Windows history and cloud sync. This is not
            // a promise that third-party clipboard managers obey these formats.
            for name in [
                w!("CanIncludeInClipboardHistory"),
                w!("CanUploadToCloudClipboard"),
            ] {
                let format = unsafe { RegisterClipboardFormatW(name) };
                if format == 0 {
                    return Err(ClipboardError::Unavailable);
                }
                Allocation::new(&0_u32.to_ne_bytes())?.transfer(format)?;
            }
            allocation.transfer(13 /* CF_UNICODETEXT */)?;
            Ok(unsafe { GetClipboardSequenceNumber() })
        }

        fn clear_if_current(&mut self, sequence: u32) -> Result<bool, ClipboardError> {
            let _lock = Open::new(self.owner()?)?;
            if unsafe { GetClipboardSequenceNumber() } != sequence {
                return Ok(false);
            }
            if unsafe { EmptyClipboard() } == 0 {
                return Err(ClipboardError::Unavailable);
            }
            Ok(true)
        }
    }
    impl Drop for NativeClipboard {
        fn drop(&mut self) {
            if !self.owner.is_null() {
                unsafe {
                    DestroyWindow(self.owner);
                }
            }
        }
    }
}

#[cfg(windows)]
use native::NativeClipboard;

#[cfg(not(windows))]
#[derive(Default)]
struct NativeClipboard;
#[cfg(not(windows))]
impl Backend for NativeClipboard {
    fn write(&mut self, _: &str) -> Result<u32, ClipboardError> {
        Err(ClipboardError::Unavailable)
    }
    fn clear_if_current(&mut self, _: u32) -> Result<bool, ClipboardError> {
        Err(ClipboardError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Fake {
        sequence: u32,
        clears: usize,
        busy: bool,
    }
    impl Backend for Fake {
        fn write(&mut self, _: &str) -> Result<u32, ClipboardError> {
            if self.busy {
                return Err(ClipboardError::Unavailable);
            }
            self.sequence += 1;
            Ok(self.sequence)
        }
        fn clear_if_current(&mut self, sequence: u32) -> Result<bool, ClipboardError> {
            if self.busy {
                return Err(ClipboardError::Unavailable);
            }
            if self.sequence != sequence {
                return Ok(false);
            }
            self.clears += 1;
            self.sequence += 1;
            Ok(true)
        }
    }
    fn clipboard() -> TimedClipboard<Fake> {
        TimedClipboard {
            backend: Fake::default(),
            lease: None,
        }
    }

    #[test]
    fn clears_only_after_thirty_seconds() {
        let mut cb = clipboard();
        let now = Instant::now();
        cb.copy("test-only", now).unwrap();
        assert_eq!(cb.poll(now + Duration::from_secs(29)), PollResult::Idle);
        assert_eq!(cb.backend.clears, 0);
        assert_eq!(cb.poll(now + LIFETIME), PollResult::Cleared);
        assert_eq!(cb.backend.clears, 1);
        assert_eq!(cb.poll(now + LIFETIME), PollResult::Idle);
    }
    #[test]
    fn later_copies_from_other_apps_are_never_erased() {
        let mut cb = clipboard();
        let now = Instant::now();
        cb.copy("test-only", now).unwrap();
        cb.backend.sequence += 1;
        assert_eq!(cb.poll(now + LIFETIME), PollResult::Superseded);
        assert_eq!(cb.backend.clears, 0);
    }
    #[test]
    fn a_second_copy_restarts_the_timer() {
        let mut cb = clipboard();
        let now = Instant::now();
        cb.copy("first-test", now).unwrap();
        cb.copy("second-test", now + Duration::from_secs(20))
            .unwrap();
        assert_eq!(cb.poll(now + LIFETIME), PollResult::Idle);
        assert_eq!(cb.poll(now + Duration::from_secs(50)), PollResult::Cleared);
    }
    #[test]
    fn transient_contention_is_retried_but_not_forever() {
        let mut cb = clipboard();
        let now = Instant::now();
        cb.copy("test-only", now).unwrap();
        cb.backend.busy = true;
        assert_eq!(cb.poll(now + LIFETIME), PollResult::Idle);
        cb.backend.busy = false;
        assert_eq!(
            cb.poll(now + LIFETIME + Duration::from_secs(1)),
            PollResult::Cleared
        );
        cb.copy("test-only", now).unwrap();
        cb.backend.busy = true;
        assert_eq!(cb.poll(now + LIFETIME + RETRY_WINDOW), PollResult::Failed);
        assert!(cb.lease.is_none());
    }
    #[test]
    fn failed_copy_does_not_claim_success_or_replace_lease() {
        let mut cb = clipboard();
        let now = Instant::now();
        cb.backend.busy = true;
        assert_eq!(cb.copy("test-only", now), Err(ClipboardError::Unavailable));
        assert!(cb.lease.is_none());
    }
}
