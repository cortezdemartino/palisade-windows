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
    session_stop: Option<Arc<AtomicBool>>,
    input_stop: Option<Arc<AtomicBool>>,
    result: Arc<Mutex<Option<bool>>>,
    until: Instant,
    alive: Cell<bool>,
}
impl Context {
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
            || self
                .session_stop
                .as_ref()
                .is_some_and(|s| s.load(Ordering::SeqCst))
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
        session_stop: Arc<AtomicBool>,
        input_stop: Arc<AtomicBool>,
    ) -> Result<Self, &'static str> {
        if session_stop.load(Ordering::SeqCst) {
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
        session_stop: None,
        input_stop: None,
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
    };
    Ok(unsafe { show(&context, text, "I compared the key") })
}
unsafe fn show(context: &Context, text: &str, allow_label: &str) -> bool {
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
        if context.alive.get() {
            KillTimer(hwnd, 1);
            DestroyWindow(hwnd);
        }
        !context.cancelled() && *context.result.lock().unwrap() == Some(true)
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
#[cfg(test)]
mod tests {
    use super::*;
    fn context(session_stop: Arc<AtomicBool>, input_stop: Option<Arc<AtomicBool>>) -> Context {
        Context {
            cancel: Arc::new(AtomicBool::new(false)),
            session_stop: Some(session_stop),
            input_stop,
            result: Arc::new(Mutex::new(None)),
            until: Instant::now() + Duration::from_secs(1),
            alive: Cell::new(false),
        }
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
