//! Native plain-text clipboard boundary. Only the session/resource owner calls
//! these functions after protected local consent; no clipboard primitive is
//! exposed to a renderer or accepted as remote authorization.
use super::{input::ordinary_desktop, input_owner::InputOwner};
use std::{mem::size_of, ptr};
use windows_sys::{
    Win32::{
        Foundation::{GetLastError, GlobalFree, HGLOBAL, HWND, SetLastError},
        System::{
            DataExchange::{
                CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
                OpenClipboard, SetClipboardData,
            },
            Memory::{
                GMEM_MOVEABLE, GMEM_ZEROINIT, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
            },
        },
        UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, HWND_MESSAGE},
    },
    core::w,
};
use zeroize::Zeroizing;

const TEXT_BYTES: usize = 65_536;
const MAX_UNITS: usize = TEXT_BYTES + 1;
const MAX_MEMORY: usize = MAX_UNITS * size_of::<u16>();
// Win32 CF_UNICODETEXT. The generated constant resides in the otherwise unused
// OLE module; this adapter supports no OLE or rich clipboard formats.
const UNICODE_TEXT: u32 = 13;

fn validate_text(text: &str) -> Result<(), &'static str> {
    if text.is_empty() || text.len() > TEXT_BYTES || text.contains('\0') {
        return Err("invalid-clipboard-text");
    }
    Ok(())
}

fn encode_text(text: &str) -> Result<Zeroizing<Vec<u16>>, &'static str> {
    validate_text(text)?;
    Ok(Zeroizing::new(text.encode_utf16().chain(Some(0)).collect()))
}

fn decode_text(units: &[u16]) -> Result<Zeroizing<String>, &'static str> {
    if units.is_empty() || units.len() > MAX_UNITS {
        return Err("invalid-clipboard-text");
    }
    let end = units
        .iter()
        .position(|&unit| unit == 0)
        .ok_or("invalid-clipboard-text")?;
    let text =
        Zeroizing::new(String::from_utf16(&units[..end]).map_err(|_| "invalid-clipboard-text")?);
    validate_text(&text)?;
    Ok(text)
}

fn authorize_operation(
    authorize: &impl Fn() -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    authorize()?;
    if !cfg!(feature = "native-resources") {
        return Err("native-resources-disabled");
    }
    // Unit builds may validate the pure codec and refusal paths, never access
    // the developer's clipboard, even when all native features are compiled.
    if cfg!(test) {
        return Err("clipboard-test-os-disabled");
    }
    ordinary_desktop()?;
    Ok(())
}

// Cleanup results are evaluated in order before entering this function. Never
// use `?` between independent cleanup attempts: a failed unlock/close must not
// suppress closing the clipboard or destroying its owned message-only window.
fn finalize_action<T, const N: usize>(
    action: Result<T, &'static str>,
    cleanup: [Result<(), &'static str>; N],
) -> Result<T, &'static str> {
    match cleanup.into_iter().find_map(Result::err) {
        Some(error) => Err(error),
        None => action,
    }
}

struct ClipboardWindow(HWND);
impl ClipboardWindow {
    fn close(mut self) -> Result<(), &'static str> {
        if unsafe { DestroyWindow(self.0) } == 0 {
            return Err("clipboard-owner-cleanup-failed");
        }
        self.0 = ptr::null_mut();
        Ok(())
    }
    fn create() -> Result<Self, &'static str> {
        // SAFETY: STATIC is a system class; this hidden message-only window is
        // owned and destroyed on this thread. It exposes no UI or remote API.
        let window = unsafe {
            CreateWindowExW(
                0,
                w!("STATIC"),
                w!("Palisade Assist clipboard owner"),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            )
        };
        if window.is_null() {
            return Err("clipboard-owner-unavailable");
        }
        Ok(Self(window))
    }
}
impl Drop for ClipboardWindow {
    fn drop(&mut self) {
        // SAFETY: this is our own same-thread message-only window, and the
        // clipboard-open guard has already closed before this owner is dropped.
        if !self.0.is_null() {
            unsafe { DestroyWindow(self.0) };
        }
    }
}

struct OpenedClipboard {
    opened: bool,
}
impl OpenedClipboard {
    fn open(owner: &ClipboardWindow) -> Result<Self, &'static str> {
        // SAFETY: owner stays alive until after this guard closes the clipboard.
        if unsafe { OpenClipboard(owner.0) } == 0 {
            return Err("clipboard-busy");
        }
        Ok(Self { opened: true })
    }
    fn close(mut self) -> Result<(), &'static str> {
        // SAFETY: paired with this guard's successful same-thread open. A
        // failed close is returned as a failure even if Drop's retry succeeds.
        if unsafe { CloseClipboard() } == 0 {
            return Err("clipboard-close-failed");
        }
        self.opened = false;
        Ok(())
    }
}
impl Drop for OpenedClipboard {
    fn drop(&mut self) {
        // SAFETY: paired with one successful OpenClipboard on this thread.
        if self.opened {
            unsafe { CloseClipboard() };
        }
    }
}

struct LockedGlobal {
    handle: HGLOBAL,
    pointer: *mut u16,
    locked: bool,
}
impl LockedGlobal {
    fn lock(handle: HGLOBAL) -> Result<Self, &'static str> {
        // SAFETY: a clipboard-owned or locally allocated HGLOBAL. Its bounded
        // size is checked before the returned pointer is used as a UTF-16 slice.
        let pointer = unsafe { GlobalLock(handle) }.cast::<u16>();
        if pointer.is_null() {
            return Err("clipboard-memory-unavailable");
        }
        let guard = Self {
            handle,
            pointer,
            locked: true,
        };
        if pointer.align_offset(std::mem::align_of::<u16>()) != 0 {
            return finalize_action(Err("invalid-clipboard-text"), [guard.unlock()]);
        }
        Ok(guard)
    }
    fn unlock(mut self) -> Result<(), &'static str> {
        // GlobalUnlock returns zero both for the final successful unlock and
        // for failure; a reset last-error distinguishes those cases.
        unsafe { SetLastError(0) };
        if unsafe { GlobalUnlock(self.handle) } == 0 && unsafe { GetLastError() } != 0 {
            return Err("clipboard-unlock-failed");
        }
        self.locked = false;
        Ok(())
    }
}
impl Drop for LockedGlobal {
    fn drop(&mut self) {
        if self.locked {
            // SAFETY: an error/unwind path still owns this one successful lock.
            unsafe { GlobalUnlock(self.handle) };
        }
    }
}

struct OwnedGlobal {
    handle: HGLOBAL,
    bytes: usize,
}
impl OwnedGlobal {
    fn allocate(bytes: usize) -> Result<Self, &'static str> {
        if !(size_of::<u16>()..=MAX_MEMORY).contains(&bytes) {
            return Err("invalid-clipboard-text");
        }
        // SAFETY: bounded allocation with the flags required by SetClipboardData.
        let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes) };
        if handle.is_null() {
            return Err("clipboard-memory-unavailable");
        }
        Ok(Self { handle, bytes })
    }
    fn close(mut self) -> Result<(), &'static str> {
        if self.handle.is_null() {
            return Ok(());
        }
        // Attempt both wiping/unlocking and freeing. An earlier failed cleanup
        // stays failed even when later release or Drop's retry succeeds.
        let wiped = match LockedGlobal::lock(self.handle) {
            Ok(memory) => {
                // SAFETY: this owned, bounded allocation remains locked and has
                // never transferred to the system on this cleanup path.
                unsafe { ptr::write_bytes(memory.pointer.cast::<u8>(), 0, self.bytes) };
                memory.unlock()
            }
            Err("clipboard-unlock-failed") => Err("clipboard-unlock-failed"),
            Err(_) => Err("clipboard-memory-cleanup-failed"),
        };
        // SAFETY: only locally owned memory is freed. On failure the retained
        // handle remains available for Drop's best-effort unwind/retry path.
        let freed = if unsafe { GlobalFree(self.handle) }.is_null() {
            self.handle = ptr::null_mut();
            Ok(())
        } else {
            Err("clipboard-memory-cleanup-failed")
        };
        finalize_action(Ok(()), [wiped, freed])
    }
    fn transferred(&mut self) {
        self.handle = ptr::null_mut();
    }
}
impl Drop for OwnedGlobal {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: failed writes retain this allocation. Best-effort wiping
            // precedes freeing; a successful SetClipboardData transfers ownership
            // and clears our handle so the OS-owned object is never freed here.
            unsafe {
                let memory = GlobalLock(self.handle);
                if !memory.is_null() {
                    ptr::write_bytes(memory.cast::<u8>(), 0, self.bytes);
                    GlobalUnlock(self.handle);
                }
                GlobalFree(self.handle);
            }
        }
    }
}

/// Reads only one bounded plain Unicode text value after native local consent.
/// The caller must retain its exact grant, peer and cancellation binding in
/// `authorize`; it must not substitute a renderer-provided approval Boolean.
pub(crate) fn read_text(
    authorize: impl Fn() -> Result<(), &'static str>,
) -> Result<Zeroizing<String>, &'static str> {
    authorize_operation(&authorize)?;
    let _input_exclusion = InputOwner::acquire()?;
    authorize_operation(&authorize)?;
    let owner = ClipboardWindow::create()?;
    let clipboard = match OpenedClipboard::open(&owner) {
        Ok(clipboard) => clipboard,
        Err(error) => return finalize_action(Err(error), [owner.close()]),
    };
    let copied = (|| {
        authorize_operation(&authorize)?;
        if unsafe { IsClipboardFormatAvailable(UNICODE_TEXT) } == 0 {
            return Err("clipboard-text-unavailable");
        }
        // GetClipboardData may wait for another application's delayed rendering.
        // Retain the resource worker through this OS call; cancellation is checked
        // before inspecting bytes and before returning any copied text.
        let handle = unsafe { GetClipboardData(UNICODE_TEXT) };
        if handle.is_null() {
            return Err("clipboard-text-unavailable");
        }
        authorize_operation(&authorize)?;
        let size = unsafe { GlobalSize(handle) };
        if !(size_of::<u16>()..=MAX_MEMORY).contains(&size) || size % size_of::<u16>() != 0 {
            return Err("invalid-clipboard-text");
        }
        let memory = LockedGlobal::lock(handle)?;
        let copied = (|| {
            if unsafe { GlobalSize(handle) } != size {
                return Err("invalid-clipboard-text");
            }
            // SAFETY: clipboard is open; HGLOBAL is locked, aligned and bounded.
            // Copy before unlocking; never free the system's clipboard object.
            Ok(Zeroizing::new(unsafe {
                std::slice::from_raw_parts(memory.pointer, size / size_of::<u16>()).to_vec()
            }))
        })();
        finalize_action(copied, [memory.unlock()])
    })();
    let copied = finalize_action(copied, [clipboard.close(), owner.close()])?;
    let text = decode_text(&copied)?;
    authorize_operation(&authorize)?;
    Ok(text)
}

/// Replaces clipboard text only after the exact native Copy action. Other
/// formats are not exposed. Allocation/encoding precede EmptyClipboard, but
/// Windows does not promise an atomic restore if SetClipboardData then fails.
pub(crate) fn write_text(
    text: &str,
    authorize: impl Fn() -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    authorize_operation(&authorize)?;
    let units = encode_text(text)?;
    let _input_exclusion = InputOwner::acquire()?;
    authorize_operation(&authorize)?;
    let mut allocation = OwnedGlobal::allocate(units.len() * size_of::<u16>())?;
    let written = (|| {
        let memory = LockedGlobal::lock(allocation.handle)?;
        // SAFETY: the source length sized this uniquely owned, locked allocation;
        // both pointers are valid, bounded and nonoverlapping.
        unsafe { ptr::copy_nonoverlapping(units.as_ptr(), memory.pointer, units.len()) };
        memory.unlock()?;
        let owner = ClipboardWindow::create()?;
        let clipboard = match OpenedClipboard::open(&owner) {
            Ok(clipboard) => clipboard,
            Err(error) => return finalize_action(Err(error), [owner.close()]),
        };
        let written = (|| {
            authorize_operation(&authorize)?;
            if unsafe { EmptyClipboard() } == 0 {
                return Err("clipboard-write-failed");
            }
            authorize_operation(&authorize)?;
            // A synchronous OS call already entered cannot be retracted.
            // Cancellation prevents subsequent admission, not a fictional rollback.
            if unsafe { SetClipboardData(UNICODE_TEXT, allocation.handle) }.is_null() {
                return Err("clipboard-write-failed");
            }
            allocation.transferred();
            Ok(())
        })();
        finalize_action(written, [clipboard.close(), owner.close()])
    })();
    finalize_action(written, [allocation.close()])?;
    authorize_operation(&authorize)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_cleanup_attempts_all_run_and_cleanup_failure_has_priority() {
        let attempted = std::cell::RefCell::new(Vec::new());
        let attempt = |name, outcome| {
            attempted.borrow_mut().push(name);
            outcome
        };
        assert_eq!(
            finalize_action::<(), 3>(
                Err("revoked"),
                [
                    attempt("unlock", Err("clipboard-unlock-failed")),
                    attempt("close", Err("clipboard-close-failed")),
                    attempt("window", Ok(())),
                ]
            ),
            Err("clipboard-unlock-failed")
        );
        assert_eq!(*attempted.borrow(), ["unlock", "close", "window"]);
    }

    #[test]
    fn successful_cleanup_preserves_action_result_and_late_cleanup_failure_is_not_hidden() {
        assert_eq!(finalize_action::<_, 2>(Ok(7), [Ok(()), Ok(())]), Ok(7));
        assert_eq!(
            finalize_action::<(), 2>(Err("clipboard-busy"), [Ok(()), Ok(())]),
            Err("clipboard-busy")
        );
        assert_eq!(
            finalize_action::<(), 2>(
                Err("invalid-clipboard-text"),
                [Ok(()), Err("clipboard-owner-cleanup-failed")]
            ),
            Err("clipboard-owner-cleanup-failed")
        );
        assert_eq!(
            finalize_action::<(), 1>(Ok(()), [Err("clipboard-memory-cleanup-failed")]),
            Err("clipboard-memory-cleanup-failed")
        );
    }

    #[test]
    fn bounded_unicode_text_round_trips_without_loss() {
        for text in ["ordinary text", "café 日本", "🙂 \r\n text", "\tvalue"] {
            let units = encode_text(text).unwrap();
            assert_eq!(units.last(), Some(&0));
            assert_eq!(decode_text(&units).unwrap().as_str(), text);
        }
        let maximum = "a".repeat(TEXT_BYTES);
        assert_eq!(encode_text(&maximum).unwrap().len(), MAX_UNITS);
        assert_eq!(
            decode_text(&encode_text(&maximum).unwrap()).unwrap().len(),
            TEXT_BYTES
        );
    }

    #[test]
    fn text_bounds_count_utf8_bytes_and_refuse_empty_nul_and_oversize() {
        for text in [
            String::new(),
            "before\0after".into(),
            "a".repeat(TEXT_BYTES + 1),
            "€".repeat(TEXT_BYTES / 3 + 1),
        ] {
            assert!(encode_text(&text).is_err());
        }
        let units = vec![0x20ac; TEXT_BYTES / 3 + 1];
        let mut terminated = units;
        terminated.push(0);
        assert!(decode_text(&terminated).is_err());
    }

    #[test]
    fn untrusted_clipboard_requires_bounded_terminated_well_formed_utf16() {
        for units in [
            vec![],
            vec![0],
            vec![65],
            vec![0xd800, 0],
            vec![0xdc00, 0],
            vec![0xd800, 65, 0],
            vec![65; MAX_UNITS + 1],
        ] {
            assert!(decode_text(&units).is_err());
        }
        // Only the first terminated text is copied; allocation padding is not
        // interpreted as another string or sent as hidden trailing content.
        assert_eq!(decode_text(&[65, 0, 0xd800]).unwrap().as_str(), "A");
        assert_eq!(decode_text(&[0xd83d, 0xde42, 0]).unwrap().as_str(), "🙂");
    }

    #[test]
    fn revoked_authority_refuses_before_any_clipboard_or_window_operation() {
        assert!(matches!(read_text(|| Err("revoked")), Err("revoked")));
        assert_eq!(write_text("not copied", || Err("revoked")), Err("revoked"));
    }

    #[test]
    fn unit_builds_never_read_or_write_the_real_clipboard_even_with_all_features() {
        let expected = if cfg!(feature = "native-resources") {
            "clipboard-test-os-disabled"
        } else {
            "native-resources-disabled"
        };
        assert!(matches!(read_text(|| Ok(())), Err(error) if error == expected));
        assert_eq!(write_text("not copied", || Ok(())), Err(expected));
    }
}
