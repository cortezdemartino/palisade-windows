//! App-owned Win32 consent. No renderer callback can mark this prompt approved.
use crate::stream::{SessionGate, StreamGrant};
use crate::windows::{input::Source, lease::now};
use std::{
    cell::Cell,
    ptr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::{
    Win32::{
        Foundation::*,
        System::{
            LibraryLoader::GetModuleHandleW, SystemInformation::GetTickCount,
            Time::FileTimeToSystemTime,
        },
        UI::{
            Input::{
                KeyboardAndMouse::{
                    GetFocus, IsWindowEnabled, ReleaseCapture, SetCapture, SetFocus,
                },
                *,
            },
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
    core::w,
};
#[derive(Clone, Debug)]
pub enum Decision {
    Pending,
    Allowed(Box<StreamGrant>),
    Denied,
}
struct Context {
    cancel: Arc<AtomicBool>,
    session_stop: Option<crate::cancellation::SessionStop>,
    input_stop: Option<Arc<AtomicBool>>,
    result: Arc<Mutex<Option<bool>>>,
    until: Instant,
    alive: Cell<bool>,
    approval: Cell<ApprovalGesture>,
}
/// A fresh press/release delivered to Allow, not a button notification, is the
/// approval authority. InputOwner excludes live/cleanup injectors; provenance
/// rejects their previously queued SendInput events after that owner has left.
/// Windows also classifies trusted UIAccess software as HARDWARE. This is an OS
/// trust boundary, not a claim to resist a compromised local desktop.
#[derive(Clone, Copy, Default)]
struct ApprovalGesture {
    shown: u32,
    held: Option<u32>,
}
impl ApprovalGesture {
    fn event(&mut self, control: u32, down: bool, repeat: bool, trusted: bool, time: u32) -> bool {
        if !trusted || time == self.shown || time.wrapping_sub(self.shown) >= 1 << 31 {
            self.held = None;
            return false;
        }
        if down {
            if !repeat && self.held.is_none() {
                self.held = Some(control);
            }
            false
        } else {
            self.held.take() == Some(control)
        }
    }
    fn reset(&mut self) {
        self.held = None;
    }
}
impl Context {
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
            || self.session_stop.as_ref().is_some_and(|s| s.is_stopped())
            || self
                .input_stop
                .as_ref()
                .is_some_and(|s| s.load(Ordering::SeqCst))
    }
}
pub struct Prompt {
    cancel: Arc<AtomicBool>,
    result: Arc<Mutex<Decision>>,
    worker: crate::cleanup::RetainedWorker,
    gate: Arc<Mutex<SessionGate>>,
    grant_id: String,
    delivered: bool,
    cleanup_failed: bool,
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub(crate) fn safe_identity(s: &str) -> bool {
    !s.chars().any(|c| {
        c.is_control()
            || matches!(c,'\u{061c}'|'\u{00ad}'|'\u{034f}'|'\u{180e}'|'\u{200b}'..='\u{200f}'|'\u{2028}'..='\u{202e}'|'\u{2060}'..='\u{206f}'|'\u{feff}')
    })
}
fn expiry(time: u64) -> Result<String, &'static str> {
    unsafe {
        let ticks = time
            .checked_add(11_644_473_600_000)
            .and_then(|v| v.checked_mul(10000))
            .ok_or("invalid-time")?;
        let ft = FILETIME {
            dwLowDateTime: ticks as u32,
            dwHighDateTime: (ticks >> 32) as u32,
        };
        let mut st: SYSTEMTIME = std::mem::zeroed();
        if FileTimeToSystemTime(&ft, &mut st) == 0 {
            return Err("invalid-time");
        }
        Ok(format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
            st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
        ))
    }
}
// Independent capability decisions must not be invalidated by another grant's
// epoch. The exact original grant, own revocation and session clock still bind it.
fn approve_current(
    gate: &mut SessionGate,
    offered: &StreamGrant,
    time: u64,
) -> Result<(), &'static str> {
    let current = gate.pending_grant(&offered.grant_id, time)?;
    if current.canonical() != offered.canonical() {
        return Err("binding-mismatch");
    }
    gate.consent(&offered.grant_id, time)
}
impl Prompt {
    pub fn request(
        gate: Arc<Mutex<SessionGate>>,
        grant_id: &str,
        current_source: Option<Arc<Mutex<Source>>>,
        session_stop: impl Into<crate::cancellation::SessionStop>,
        input_stop: Arc<AtomicBool>,
    ) -> Result<Self, &'static str> {
        let session_stop = session_stop.into();
        if session_stop.is_stopped() {
            return Err("revoked");
        }
        let grant = {
            let mut guard = gate.lock().map_err(|_| "endpoint-failed")?;
            guard.pending_grant(grant_id, now()?)?
        };
        if !safe_identity(&grant.helper_account) || !safe_identity(&grant.helper_device) {
            return Err("unsafe-identity");
        }
        let input = matches!(
            grant.capability.as_str(),
            "control_mouse" | "control_keyboard"
        );
        let source = if input {
            Some(
                current_source
                    .as_ref()
                    .ok_or("source-required")?
                    .lock()
                    .map_err(|_| "source-failed")?
                    .clone(),
            )
        } else {
            None
        };
        if let Some(source) = &source {
            source.current_monitor()?;
        }
        let permission = match grant.capability.as_str() {
            "control_mouse" => "Mouse control",
            "control_keyboard" => "Keyboard control",
            "clipboard_to_host" => "Receive clipboard text",
            "clipboard_to_guest" => "Send clipboard text",
            "file_to_host" => "Receive selected files",
            "file_to_guest" => "Send selected files",
            _ => return Err("not-native-capability"),
        };
        let scope = if let Some(source) = &source {
            format!(
                "Mouse: display {} x {} at ({}, {}). Keyboard: focused application on the interactive desktop. Window-only confinement is not offered.",
                source.width, source.height, source.x, source.y
            )
        } else {
            "Only this separately approved clipboard/file direction. Files require local selection or acceptance. Clipboard text may contain sensitive information.".into()
        };
        let text = format!(
            "{}\r\nDevice: {}\r\n\r\nRequests: {}\r\nScope: {}\r\nEnds: {}\r\n\r\nPermission applies only to this session and ends with the stream. Clipboard and file directions require separate approval.\r\nStop Assist or Ctrl+Alt+Shift+F10 ends control.",
            grant.helper_account,
            grant.helper_device,
            permission,
            scope,
            expiry(grant.session_deadline)?
        );
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let result = Arc::new(Mutex::new(Decision::Pending));
        let output = result.clone();
        let retained_gate = gate.clone();
        let retained_id = grant.grant_id.clone();
        let worker = thread::spawn(move || {
            let context = Context {
                cancel: flag.clone(),
                session_stop: Some(session_stop.clone()),
                input_stop: input.then_some(input_stop),
                result: Arc::new(Mutex::new(None)),
                until: Instant::now() + Duration::from_secs(60),
                alive: Cell::new(false),
                approval: Cell::new(ApprovalGesture::default()),
            };
            let approved = unsafe { show(&context, &text, "Allow this session") };
            if approved {
                let accept = (|| {
                    let mut guard = gate.lock().map_err(|_| "endpoint-failed")?;
                    if context.cancelled() {
                        return Err("revoked");
                    }
                    if let Some(source) = &source {
                        let live = current_source
                            .as_ref()
                            .ok_or("source-required")?
                            .lock()
                            .map_err(|_| "source-failed")?;
                        if *live != *source {
                            return Err("source-changed");
                        };
                        source.current_monitor()?;
                    }
                    approve_current(&mut guard, &grant, now()?)?;
                    if context.cancelled() {
                        guard.revoke(&grant.grant_id);
                        return Err("revoked");
                    }
                    *output.lock().map_err(|_| "endpoint-failed")? =
                        Decision::Allowed(Box::new(grant));
                    Ok::<_, &'static str>(())
                })();
                if accept.is_err() {
                    *output.lock().unwrap() = Decision::Denied;
                }
            } else {
                *output.lock().unwrap() = Decision::Denied;
            }
            Ok(())
        });
        Ok(Self {
            cancel,
            result,
            worker: crate::cleanup::RetainedWorker::new(worker),
            gate: retained_gate,
            grant_id: retained_id,
            delivered: false,
            cleanup_failed: false,
        })
    }
    pub fn decision(&self) -> Decision {
        self.result
            .lock()
            .map(|v| v.clone())
            .unwrap_or(Decision::Denied)
    }
    pub fn take_decision(&mut self) -> Decision {
        let decision = self.decision();
        if matches!(decision, Decision::Allowed(_)) {
            self.delivered = true;
        }
        decision
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
    pub fn finish(&mut self) -> Result<(), &'static str> {
        // UI cancellation precedes even a poisoned/busy gate lock.
        self.cancel();
        let mut guard = self.gate.lock().unwrap_or_else(|error| {
            self.cleanup_failed = true;
            error.into_inner()
        });
        if !self.delivered {
            guard.revoke(&self.grant_id);
        }
        drop(guard);
        if self.worker.join().is_err() {
            self.cleanup_failed = true;
        }
        if self.cleanup_failed {
            Err("consent-cleanup-failed")
        } else {
            Ok(())
        }
    }
}
impl Drop for Prompt {
    fn drop(&mut self) {
        let _ = crate::cleanup::attempt(|| self.finish());
    }
}
pub(crate) fn confirm_file(
    grant: &StreamGrant,
    transfer: &str,
    metadata: &super::files::Metadata,
    cancel: Arc<AtomicBool>,
    session_stop: crate::cancellation::SessionStop,
) -> Result<bool, &'static str> {
    if cfg!(test) || !cfg!(feature = "native-resources") {
        return Err("native-resources-disabled");
    }
    metadata.validate()?;
    if grant.capability != "file_to_host"
        || !crate::grant::token(transfer, 8, 128)
        || !safe_identity(&metadata.name)
        || !safe_identity(&grant.helper_account)
        || !safe_identity(&grant.helper_device)
    {
        return Err("invalid-file-offer");
    }
    let context = Context {
        cancel,
        session_stop: Some(session_stop),
        input_stop: None,
        result: Arc::new(Mutex::new(None)),
        until: Instant::now() + Duration::from_secs(60),
        alive: Cell::new(false),
        approval: Cell::new(ApprovalGesture::default()),
    };
    let text = format!(
        "Receive this file from {} (device {})?\r\n\r\nFile: {}\r\nSize: {} bytes\r\nSHA-256: {}\r\nTransfer: {}\r\nPermission expires: {}\r\n\r\nReceived files are untrusted. Choose a save location after accepting.",
        grant.helper_account,
        grant.helper_device,
        metadata.name,
        metadata.size,
        metadata.sha256,
        transfer,
        expiry(grant.expires_at)?
    );
    Ok(unsafe { show(&context, &text, "Receive this file") })
}
pub(crate) fn confirm_clipboard(
    grant: &StreamGrant,
    incoming: Option<&str>,
    cancel: Arc<AtomicBool>,
    session_stop: crate::cancellation::SessionStop,
) -> Result<bool, &'static str> {
    if cfg!(test) {
        return Err("clipboard-test-os-disabled");
    }
    if !cfg!(feature = "native-resources") {
        return Err("native-resources-disabled");
    }
    super::input::ordinary_desktop()?;
    let expected = if incoming.is_some() {
        "clipboard_to_host"
    } else {
        "clipboard_to_guest"
    };
    if grant.capability != expected
        || !safe_identity(&grant.helper_account)
        || !safe_identity(&grant.helper_device)
    {
        return Err("not-granted");
    }
    if incoming.is_some_and(|text| text.is_empty() || text.len() > 65536 || text.contains('\0')) {
        return Err("invalid-clipboard-text");
    }
    let context = Context {
        cancel,
        session_stop: Some(session_stop),
        input_stop: None,
        result: Arc::new(Mutex::new(None)),
        until: Instant::now() + Duration::from_secs(60),
        alive: Cell::new(false),
        approval: Cell::new(ApprovalGesture::default()),
    };
    let action = match incoming {
        Some(text) => format!("Copy the following untrusted text to this computer's clipboard?\r\nIt will replace the current clipboard.\r\n\r\n{text}"),
        None => "Send the current plain text from this computer's clipboard?\r\nClipboard text may contain private information. Nothing is read until you approve. This does not enable clipboard synchronization.".into(),
    };
    let text = format!(
        "Helper: {}\r\nDevice: {}\r\nPermission expires: {}\r\nThis action only; no automatic clipboard access.\r\n\r\n{}",
        grant.helper_account,
        grant.helper_device,
        expiry(grant.expires_at)?,
        action
    );
    Ok(unsafe {
        show(
            &context,
            &text,
            if incoming.is_some() {
                "Copy to clipboard"
            } else {
                "Send clipboard text"
            },
        )
    })
}
/// Final confirmation for a path returned by a native dialog. The dialog's
/// FileOK notification is not itself evidence of a fresh local gesture.
pub(crate) fn confirm_selected_file(
    path: &std::path::Path,
    save: bool,
    cancel: Arc<AtomicBool>,
    session_stop: crate::cancellation::SessionStop,
) -> Result<bool, &'static str> {
    if cfg!(test) || !cfg!(feature = "native-resources") {
        return Err("native-resources-disabled");
    }
    let path = path
        .to_str()
        .filter(|s| safe_identity(s))
        .ok_or("unsafe-file-name")?;
    if path.len() > 32768 {
        return Err("unsafe-file-name");
    }
    let context = Context {
        cancel,
        session_stop: Some(session_stop),
        input_stop: None,
        result: Arc::new(Mutex::new(None)),
        until: Instant::now() + Duration::from_secs(60),
        alive: Cell::new(false),
        approval: Cell::new(ApprovalGesture::default()),
    };
    let (action, label) = if save {
        (
            "Save the verified received file at this location? Existing files will not be replaced.",
            "Save this file",
        )
    } else {
        (
            "Allow this Assist session to read and send only the selected file?",
            "Send this file",
        )
    };
    Ok(unsafe { show(&context, &format!("{action}\r\n\r\n{path}"), label) })
}
pub(crate) fn confirm_capture(
    description: &str,
    owner: &super::input_owner::InputOwner,
    session_stop: crate::cancellation::SessionStop,
) -> Result<bool, &'static str> {
    if cfg!(test) {
        return Err("capture-test-ui-disabled");
    }
    let context = Context {
        cancel: Arc::new(AtomicBool::new(false)),
        session_stop: Some(session_stop),
        input_stop: None,
        result: Arc::new(Mutex::new(None)),
        until: Instant::now() + Duration::from_secs(60),
        alive: Cell::new(false),
        approval: Cell::new(ApprovalGesture::default()),
    };
    let text = format!(
        "Share this selected source in Palisade?\r\n\r\n{description}\r\n\r\nYou can stop sharing at any time. This does not authorize remote control, clipboard or files."
    );
    Ok(unsafe { show_with_owner(&context, &text, "Share this source", owner) })
}
pub(crate) fn confirm_enrollment(
    text: &str,
    cancel: Arc<AtomicBool>,
) -> Result<bool, &'static str> {
    super::input::ordinary_desktop()?;
    let context = Context {
        cancel,
        session_stop: None,
        input_stop: None,
        result: Arc::new(Mutex::new(None)),
        until: Instant::now() + Duration::from_secs(60),
        alive: Cell::new(false),
        approval: Cell::new(ApprovalGesture::default()),
    };
    Ok(unsafe { show(&context, text, "Enroll this device") })
}
pub(crate) fn confirm_issuer(text: &str, cancel: Arc<AtomicBool>) -> Result<bool, &'static str> {
    super::input::ordinary_desktop()?;
    let context = Context {
        cancel,
        session_stop: None,
        input_stop: None,
        result: Arc::new(Mutex::new(None)),
        until: Instant::now() + Duration::from_secs(90),
        alive: Cell::new(false),
        approval: Cell::new(ApprovalGesture::default()),
    };
    Ok(unsafe { show(&context, text, "I compared the key") })
}
unsafe fn show(context: &Context, text: &str, allow_label: &str) -> bool {
    // The same kernel-object lifetime excludes every Palisade input executor,
    // including retained cleanup holders. Busy input must be terminally stopped
    // and settled first. The Allow subclass separately refuses queued injected
    // input; releasing an owner alone does not drain the Windows input queue.
    let Ok(_input_exclusion) = super::input_owner::InputOwner::acquire() else {
        return false;
    };
    unsafe { show_with_owner(context, text, allow_label, &_input_exclusion) }
}
unsafe fn show_with_owner(
    context: &Context,
    text: &str,
    allow_label: &str,
    _owner: &super::input_owner::InputOwner,
) -> bool {
    if super::input::ordinary_desktop().is_err() {
        return false;
    }
    if context.cancelled() {
        return false;
    }
    unsafe {
        let module = GetModuleHandleW(ptr::null());
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: module,
            lpszClassName: w!("PalisadeAssistConsent"),
            ..std::mem::zeroed()
        };
        RegisterClassW(&class);
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            class.lpszClassName,
            w!("Palisade Assist permission"),
            WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            660,
            350,
            ptr::null_mut(),
            ptr::null_mut(),
            module,
            (context as *const Context).cast(),
        );
        if hwnd.is_null() {
            return false;
        }
        let content = wide(text);
        let label = CreateWindowExW(
            0,
            w!("EDIT"),
            content.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_VSCROLL | 0x0004 | 0x0800,
            18,
            16,
            620,
            235,
            hwnd,
            ptr::null_mut(),
            module,
            ptr::null(),
        );
        let allow_label = wide(allow_label);
        let allow = CreateWindowExW(
            0,
            w!("BUTTON"),
            allow_label.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            290,
            260,
            160,
            34,
            hwnd,
            1001usize as HMENU,
            module,
            ptr::null(),
        );
        let deny = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Deny"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            470,
            260,
            150,
            34,
            hwnd,
            2usize as HMENU,
            module,
            ptr::null(),
        );
        if label.is_null()
            || allow.is_null()
            || deny.is_null()
            || SetWindowSubclass(
                allow,
                Some(allow_procedure),
                1,
                context as *const Context as usize,
            ) == 0
            || SetTimer(hwnd, 1, 50, None) == 0
        {
            destroy_owned_window(hwnd, context);
            return false;
        }
        if !super::lifecycle::show_required_window(hwnd, true) {
            destroy_owned_window(hwnd, context);
            return false;
        }
        SetForegroundWindow(hwnd);
        SetFocus(deny);
        context.approval.set(ApprovalGesture {
            shown: GetTickCount(),
            held: None,
        });
        let mut message = std::mem::zeroed();
        while !context.cancelled()
            && Instant::now() < context.until
            && context.result.lock().unwrap().is_none()
            && GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0
        {
            if IsDialogMessageW(hwnd, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        let approved = *context.result.lock().unwrap() == Some(true);
        if context.alive.get() {
            KillTimer(hwnd, 1);
        }
        destroy_owned_window(hwnd, context);
        !context.cancelled() && Instant::now() < context.until && approved
    }
}
unsafe fn destroy_owned_window(hwnd: HWND, context: &Context) {
    // HWND user data/subclass refer to this caller-owned Context. Failure to
    // destroy must retain both that memory and InputOwner; a timeout is not
    // cleanup evidence. The separate urgent Stop path remains available.
    *context.result.lock().unwrap() = Some(false);
    let mut gesture = context.approval.get();
    gesture.reset();
    context.approval.set(gesture);
    unsafe {
        while context.alive.get() || IsWindow(hwnd) != 0 {
            DestroyWindow(hwnd);
            if context.alive.get() || IsWindow(hwnd) != 0 {
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}
unsafe extern "system" fn allow_procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    unsafe {
        let context = &*(data as *const Context);
        if message == WM_NCDESTROY {
            RemoveWindowSubclass(hwnd, Some(allow_procedure), 1);
            return DefSubclassProc(hwnd, message, wparam, lparam);
        }
        if message == WM_GETDLGCODE && matches!(wparam, 13 | 32) {
            // Let the focused Allow button inspect Enter/Space itself. A dialog
            // manager's synthetic BM_CLICK/BN_CLICKED can never authorize it.
            return DLGC_WANTMESSAGE as LRESULT;
        }
        if matches!(
            message,
            BM_CLICK | WM_KILLFOCUS | WM_CANCELMODE | WM_CAPTURECHANGED
        ) || (message == WM_ENABLE && wparam == 0)
        {
            let mut gesture = context.approval.get();
            gesture.reset();
            context.approval.set(gesture);
            if message == BM_CLICK {
                return 0;
            }
        }
        let mouse = matches!(message, WM_LBUTTONDOWN | WM_LBUTTONUP);
        let key = matches!(message, WM_KEYDOWN | WM_KEYUP) && matches!(wparam, 13 | 32);
        if mouse || key {
            let mut source: INPUT_MESSAGE_SOURCE = std::mem::zeroed();
            let mut rect = std::mem::zeroed();
            let inside = if mouse {
                GetClientRect(hwnd, &mut rect) != 0
                    && (lparam as i16 as i32) >= 0
                    && (lparam as i16 as i32) < rect.right
                    && ((lparam >> 16) as i16 as i32) >= 0
                    && ((lparam >> 16) as i16 as i32) < rect.bottom
            } else {
                GetFocus() == hwnd
            };
            let trusted = GetCurrentInputMessageSource(&mut source) != 0
                && source.originId == IMO_HARDWARE
                && source.deviceType == if mouse { IMDT_MOUSE } else { IMDT_KEYBOARD }
                && inside
                && IsWindowEnabled(hwnd) != 0
                && IsWindowVisible(hwnd) != 0
                && GetForegroundWindow() == GetParent(hwnd)
                && !context.cancelled()
                && Instant::now() < context.until
                && super::input::ordinary_desktop().is_ok();
            let down = matches!(message, WM_LBUTTONDOWN | WM_KEYDOWN);
            let repeat = key && lparam & (1 << 30) != 0;
            let mut gesture = context.approval.get();
            let approved = gesture.event(
                if mouse { 1 } else { wparam as u32 },
                down,
                repeat,
                trusted,
                GetMessageTime() as u32,
            );
            context.approval.set(gesture);
            if mouse && down && trusted {
                SetFocus(hwnd);
                SetCapture(hwnd);
            }
            SendMessageW(hwnd, BM_SETSTATE, usize::from(down && trusted), 0);
            if mouse && !down {
                ReleaseCapture();
            }
            if approved
                && context.alive.get()
                && IsWindow(hwnd) != 0
                && IsWindowEnabled(hwnd) != 0
                && IsWindowVisible(hwnd) != 0
                && GetForegroundWindow() == GetParent(hwnd)
                && !context.cancelled()
                && Instant::now() < context.until
                && super::input::ordinary_desktop().is_ok()
            {
                // Cosmetic calls/capture release can reenter window processing.
                // A concurrent Deny/Close always wins over the earlier gesture.
                let mut result = context.result.lock().unwrap();
                if result.is_none() {
                    *result = Some(true);
                }
            }
            return 0;
        }
        DefSubclassProc(hwnd, message, wparam, lparam)
    }
}
unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
            (*(create.lpCreateParams as *const Context)).alive.set(true);
        }
        let context = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Context;
        if !context.is_null() {
            if message == WM_COMMAND
                && wparam & 0xffff == 1001
                && (wparam >> 16) & 0xffff == BN_CLICKED as usize
                && lparam as HWND == GetDlgItem(hwnd, 1001)
            {
                // Notifications, automation and posted commands are not local
                // consent. Only allow_procedure's proven fresh gesture approves.
                return 0;
            }
            if message == WM_CLOSE || (message == WM_COMMAND && wparam & 0xffff == 2) {
                *(*context).result.lock().unwrap() = Some(false);
                return 0;
            }
            if message == WM_NCDESTROY {
                (*context).alive.set(false);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
        }
        DefWindowProcW(hwnd, message, wparam, lparam)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_confirmation_tests_refuse_before_showing_ui() {
        let _serial = super::super::input_owner::APPROVAL_TEST_LOCK
            .lock()
            .unwrap();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/assist-control-v2.json"
        ))
        .unwrap();
        let grant: StreamGrant =
            serde_json::from_value(fixture["grants"]["valid"][1]["signed"]["grant"].clone())
                .unwrap();
        let stop = crate::cancellation::SessionStop::from(Arc::new(AtomicBool::new(false)));
        let cancel = Arc::new(AtomicBool::new(false));
        let metadata = super::super::files::Metadata {
            name: "synthetic.txt".into(),
            size: 0,
            sha256: "a".repeat(64),
        };
        assert_eq!(
            confirm_file(
                &grant,
                "fixture_transfer",
                &metadata,
                cancel.clone(),
                stop.clone()
            ),
            Err("native-resources-disabled")
        );
        assert_eq!(
            confirm_selected_file(
                std::path::Path::new("never-opened.txt"),
                false,
                cancel,
                stop
            ),
            Err("native-resources-disabled")
        );
        let owner = super::super::input_owner::InputOwner::acquire().unwrap();
        assert_eq!(
            confirm_capture(
                "synthetic source",
                &owner,
                crate::cancellation::SessionStop::from(Arc::new(AtomicBool::new(false)))
            ),
            Err("capture-test-ui-disabled")
        );
    }
    #[test]
    fn approval_requires_fresh_same_control_local_press_and_release() {
        let mut g = ApprovalGesture {
            shown: 100,
            held: None,
        };
        assert!(!g.event(1, true, false, true, 100));
        assert!(!g.event(1, false, false, true, 100));
        assert!(!g.event(1, false, false, true, 101));
        assert!(!g.event(1, true, false, true, 101));
        assert!(!g.event(32, false, false, true, 102));
        assert!(!g.event(1, false, false, true, 103));
        assert!(!g.event(1, true, false, true, 104));
        assert!(g.event(1, false, false, true, 105));
        assert!(!g.event(1, false, false, true, 106));
        assert!(!g.event(13, true, true, true, 107));
        assert!(!g.event(13, false, false, true, 108));
        assert!(!g.event(32, true, false, true, 109));
        assert!(g.event(32, false, false, true, 110));
    }
    #[test]
    fn queued_injected_stale_and_interrupted_gestures_cannot_approve() {
        let mut g = ApprovalGesture {
            shown: u32::MAX - 1,
            held: None,
        };
        assert!(!g.event(1, true, false, true, u32::MAX - 2));
        assert!(!g.event(1, false, false, true, u32::MAX));
        assert!(!g.event(1, true, false, false, u32::MAX));
        assert!(!g.event(1, false, false, true, 0));
        assert!(!g.event(1, true, false, true, 1));
        assert!(!g.event(1, false, false, false, 2));
        assert!(!g.event(1, false, false, true, 3));
        assert!(!g.event(1, true, false, true, 4));
        g.reset();
        assert!(!g.event(1, false, false, true, 5));
        assert!(!g.event(1, true, false, true, 6));
        assert!(g.event(1, false, false, true, 7));
    }
    #[test]
    fn owned_or_cleanup_pending_input_prevents_sensitive_ui_from_opening() {
        let _serial = super::super::input_owner::APPROVAL_TEST_LOCK
            .lock()
            .unwrap();
        let _input_owner = super::super::input_owner::InputOwner::acquire()
            .expect("test requires an unused Palisade input slot");
        let context = Context {
            cancel: Arc::new(AtomicBool::new(false)),
            session_stop: None,
            input_stop: None,
            result: Arc::new(Mutex::new(None)),
            until: Instant::now() + Duration::from_secs(1),
            alive: Cell::new(false),
            approval: Cell::new(ApprovalGesture::default()),
        };
        assert!(!unsafe {
            show(
                &context,
                "must never be displayed",
                "must never be approved",
            )
        });
        assert!(!context.alive.get());
        assert!(context.result.lock().unwrap().is_none());
    }
    fn context(
        session_stop: impl Into<crate::cancellation::SessionStop>,
        input_stop: Option<Arc<AtomicBool>>,
    ) -> Context {
        Context {
            cancel: Arc::new(AtomicBool::new(false)),
            session_stop: Some(session_stop.into()),
            input_stop,
            result: Arc::new(Mutex::new(None)),
            until: Instant::now() + Duration::from_secs(1),
            alive: Cell::new(false),
            approval: Cell::new(ApprovalGesture::default()),
        }
    }
    #[test]
    fn sibling_session_end_keeps_other_ui_live_but_launch_stop_cancels_it() {
        let launch = Arc::new(AtomicBool::new(false));
        let first = crate::cancellation::SessionStop::child(launch.clone());
        let sibling = crate::cancellation::SessionStop::child(launch.clone());
        let first_ui = context(first.clone(), None);
        let sibling_ui = context(sibling, None);
        first.revoke();
        assert!(first_ui.cancelled());
        assert!(!sibling_ui.cancelled());
        launch.store(true, Ordering::SeqCst);
        assert!(sibling_ui.cancelled());
        assert!(!unsafe { show(&sibling_ui, "unused fixture", "unused fixture") });
    }
    #[test]
    fn session_and_input_revocation_cancel_only_the_appropriate_pending_ui() {
        let stop = Arc::new(AtomicBool::new(false));
        let input = Arc::new(AtomicBool::new(false));
        let native_input = context(stop.clone(), Some(input.clone()));
        let resource = context(stop.clone(), None);
        input.store(true, Ordering::SeqCst);
        assert!(native_input.cancelled());
        assert!(!resource.cancelled());
        stop.store(true, Ordering::SeqCst);
        assert!(resource.cancelled());
        // Pre-cancelled UI never creates/shows a native window.
        assert!(!unsafe { show(&native_input, "unused fixture", "unused fixture") });
        assert!(!unsafe { show(&resource, "unused fixture", "unused fixture") });
    }
    #[test]
    fn exact_resource_consent_survives_input_revoke_but_never_own_revoke_or_changed_grant() {
        use crate::stream::{Binding, SignedStreamGrant};
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        use ed25519_dalek::{Signer, SigningKey};
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/assist-control-v2.json"
        ))
        .unwrap();
        let time = fixture["now"].as_u64().unwrap();
        let key = SigningKey::from_bytes(&[42; 32]);
        let input: SignedStreamGrant =
            serde_json::from_value(fixture["grants"]["valid"][1]["signed"].clone()).unwrap();
        let g = &input.grant;
        let binding = Binding {
            session: g.session.clone(),
            room: g.room.clone(),
            stream: g.stream.clone(),
            publisher_account: g.publisher_account.clone(),
            publisher_device: g.publisher_device.clone(),
            helper_account: g.helper_account.clone(),
            helper_device: g.helper_device.clone(),
            helper_transport: g.helper_transport.clone(),
            endpoint: g.endpoint.clone().unwrap(),
        };
        let mut gate = SessionGate::new(
            key.verifying_key().to_bytes(),
            binding,
            time,
            0,
            Instant::now(),
        );
        let sign = |grant: StreamGrant| SignedStreamGrant {
            signature: URL_SAFE_NO_PAD.encode(key.sign(&grant.canonical()).to_bytes()),
            grant,
        };
        gate.admit(&sign(g.clone()), time).unwrap();
        let mut resource = g.clone();
        resource.capability = "clipboard_to_host".into();
        resource.endpoint = None;
        resource.grant_id = "grant_resource_consent".into();
        resource.nonce = "resource_consent_nonce".into();
        resource.generation += 1;
        gate.admit(&sign(resource.clone()), time).unwrap();
        gate.suspend_input();
        approve_current(&mut gate, &resource, time).unwrap();
        assert!(approve_current(&mut gate, g, time).is_err());
        let mut changed = resource.clone();
        changed.helper_device = "DIFFERENT".into();
        assert!(approve_current(&mut gate, &changed, time).is_err());
        gate.revoke(&resource.grant_id);
        assert!(approve_current(&mut gate, &resource, time).is_err());
        gate.stop();
        assert!(approve_current(&mut gate, &resource, time).is_err());
    }
}
