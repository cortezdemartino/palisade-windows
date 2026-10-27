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
        System::{LibraryLoader::GetModuleHandleW, Time::FileTimeToSystemTime},
        UI::{Input::KeyboardAndMouse::SetFocus, WindowsAndMessaging::*},
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
    result: Arc<Mutex<Option<bool>>>,
    until: Instant,
    alive: Cell<bool>,
}
pub struct Prompt {
    cancel: Arc<AtomicBool>,
    result: Arc<Mutex<Decision>>,
    worker: Option<thread::JoinHandle<()>>,
    gate: Arc<Mutex<SessionGate>>,
    grant_id: String,
    delivered: bool,
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn safe_identity(s: &str) -> bool {
    !s.chars().any(|c| {
        c.is_control()
            || matches!(c,'\u{200b}'..='\u{200f}'|'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
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
impl Prompt {
    pub fn request(
        gate: Arc<Mutex<SessionGate>>,
        grant_id: &str,
        current_source: Option<Arc<Mutex<Source>>>,
    ) -> Result<Self, &'static str> {
        let (grant, epoch) = {
            let mut guard = gate.lock().map_err(|_| "endpoint-failed")?;
            (guard.pending_grant(grant_id, now()?)?, guard.epoch())
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
                result: Arc::new(Mutex::new(None)),
                until: Instant::now() + Duration::from_secs(60),
                alive: Cell::new(false),
            };
            let approved = unsafe { show(&context, &text, "Allow this session") };
            if approved {
                let accept = (|| {
                    let mut guard = gate.lock().map_err(|_| "endpoint-failed")?;
                    if flag.load(Ordering::SeqCst) || guard.epoch() != epoch {
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
                    let current = guard.pending_grant(&grant.grant_id, now()?)?;
                    if current.canonical() != grant.canonical() {
                        return Err("binding-mismatch");
                    };
                    guard.consent(&grant.grant_id, now()?)?;
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
        });
        Ok(Self {
            cancel,
            result,
            worker: Some(worker),
            gate: retained_gate,
            grant_id: retained_id,
            delivered: false,
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
}
impl Drop for Prompt {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.gate.lock() {
            self.cancel.store(true, Ordering::SeqCst);
            if !self.delivered {
                guard.revoke(&self.grant_id);
            }
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub(crate) fn confirm_file(
    grant: &StreamGrant,
    transfer: &str,
    metadata: &super::files::Metadata,
    cancel: Arc<AtomicBool>,
) -> Result<bool, &'static str> {
    metadata.validate()?;
    if grant.capability != "file_to_host"
        || !crate::grant::token(transfer, 8, 128)
        || !safe_identity(&grant.helper_account)
        || !safe_identity(&grant.helper_device)
    {
        return Err("invalid-file-offer");
    }
    let context = Context {
        cancel,
        result: Arc::new(Mutex::new(None)),
        until: Instant::now() + Duration::from_secs(60),
        alive: Cell::new(false),
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
unsafe fn show(context: &Context, text: &str, allow_label: &str) -> bool {
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
        if label.is_null() || allow.is_null() || deny.is_null() || SetTimer(hwnd, 1, 50, None) == 0
        {
            DestroyWindow(hwnd);
            return false;
        }
        if !super::lifecycle::show_required_window(hwnd, true) {
            DestroyWindow(hwnd);
            return false;
        }
        SetForegroundWindow(hwnd);
        SetFocus(deny);
        let mut message = std::mem::zeroed();
        while !context.cancel.load(Ordering::SeqCst)
            && Instant::now() < context.until
            && context.result.lock().unwrap().is_none()
            && GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0
        {
            if IsDialogMessageW(hwnd, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        if context.alive.get() {
            KillTimer(hwnd, 1);
            DestroyWindow(hwnd);
        }
        *context.result.lock().unwrap() == Some(true)
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
                // The payload is never returned: the owning thread supplies its verified grant.
                *(*context).result.lock().unwrap() = Some(true);
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
