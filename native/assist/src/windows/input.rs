//! Dormant OS adapter. Default builds cannot dispatch input, even with a fabricated caller.
use crate::executor::Backend;
pub use crate::executor::Source;
use crate::input::{Action, Ledger, scan_code};
use std::mem::{size_of, zeroed};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{RemoteDesktop::ProcessIdToSessionId, StationsAndDesktops::*, Threading::*},
    UI::{HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};
impl Source {
    fn contains(&self, p: POINT) -> bool {
        let x = i64::from(p.x);
        let y = i64::from(p.y);
        x >= i64::from(self.x)
            && y >= i64::from(self.y)
            && x < i64::from(self.x) + i64::from(self.width)
            && y < i64::from(self.y) + i64::from(self.height)
    }
    pub fn current_monitor(&self) -> Result<(), &'static str> {
        self.point(0., 0.)?;
        unsafe {
            let _dpi = DpiScope::enter()?;
            let handle = MonitorFromPoint(
                POINT {
                    x: self.x,
                    y: self.y,
                },
                MONITOR_DEFAULTTONULL,
            );
            let mut info: MONITORINFO = zeroed();
            info.cbSize = size_of::<MONITORINFO>() as u32;
            let valid = !handle.is_null()
                && GetMonitorInfoW(handle, &mut info) != 0
                && info.rcMonitor.left == self.x
                && info.rcMonitor.top == self.y
                && i64::from(info.rcMonitor.right) - i64::from(info.rcMonitor.left)
                    == i64::from(self.width)
                && i64::from(info.rcMonitor.bottom) - i64::from(info.rcMonitor.top)
                    == i64::from(self.height);
            if !valid {
                return Err("source-changed");
            }
        }
        Ok(())
    }
}
pub fn ordinary_desktop() -> Result<(), &'static str> {
    unsafe {
        let desktop = OpenInputDesktop(0, 0, DESKTOP_READOBJECTS);
        if desktop.is_null() {
            return Err("protected-desktop");
        }
        let mut name = [0u16; 128];
        let mut needed = 0;
        let ok = GetUserObjectInformationW(
            desktop,
            UOI_NAME,
            name.as_mut_ptr().cast(),
            (name.len() * 2) as u32,
            &mut needed,
        );
        CloseDesktop(desktop);
        if ok == 0
            || needed < 2
            || needed as usize > name.len() * 2
            || String::from_utf16_lossy(&name[..needed as usize / 2 - 1]) != "Default"
        {
            return Err("protected-desktop");
        }
        let mut session = 0;
        if ProcessIdToSessionId(GetCurrentProcessId(), &mut session) == 0 || session == 0 {
            return Err("session-unavailable");
        }
    }
    Ok(())
}
pub(crate) fn local_input_idle() -> bool {
    (1..=254).all(|key| unsafe { GetAsyncKeyState(key) >= 0 })
}
fn target(source: &Source, keyboard: bool) -> Result<(), &'static str> {
    ordinary_desktop()?;
    source.current_monitor()?;
    unsafe {
        let mut point: POINT = zeroed();
        if GetPhysicalCursorPos(&mut point) == 0 {
            return Err("target-unavailable");
        }
        let window = if keyboard {
            GetForegroundWindow()
        } else {
            GetAncestor(WindowFromPoint(point), GA_ROOT)
        };
        if window.is_null() {
            return Err("target-unavailable");
        }
        if keyboard {
            let mut rect: RECT = zeroed();
            if GetWindowRect(window, &mut rect) == 0
                || !source.contains(POINT {
                    x: rect.left,
                    y: rect.top,
                })
                || !source.contains(POINT {
                    x: rect.right - 1,
                    y: rect.bottom - 1,
                })
            {
                return Err("target-outside-source");
            }
        } else if !source.contains(point) {
            return Err("target-outside-source");
        }
        let mut pid = 0;
        GetWindowThreadProcessId(window, &mut pid);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return Err("target-unavailable");
        }
        let result = (|| {
            let mut target_session = 0;
            let mut own_session = 0;
            if ProcessIdToSessionId(pid, &mut target_session) == 0
                || ProcessIdToSessionId(GetCurrentProcessId(), &mut own_session) == 0
                || target_session != own_session
            {
                return Err("cross-session");
            }
            if pid == GetCurrentProcessId()
                || !crate::windows::parent::same_unelevated_user(process)
            {
                return Err("protected-target");
            }
            let mut image = [0u16; 32768];
            let mut length = image.len() as u32;
            if QueryFullProcessImageNameW(process, 0, image.as_mut_ptr(), &mut length) == 0 {
                return Err("target-unavailable");
            }
            let image = String::from_utf16_lossy(&image[..length as usize]).to_ascii_lowercase();
            if image.ends_with("\\palisade.exe") || image.ends_with("\\palisade-assist.exe") {
                return Err("consent-surface-protected");
            }
            Ok(())
        })();
        CloseHandle(process);
        result
    }
}
struct DpiScope(DPI_AWARENESS_CONTEXT);
impl DpiScope {
    fn enter() -> Result<Self, &'static str> {
        let old =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if old.is_null() {
            Err("dpi-unavailable")
        } else {
            Ok(Self(old))
        }
    }
}
impl Drop for DpiScope {
    fn drop(&mut self) {
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
        }
    }
}
pub(crate) struct WindowsBackend {
    tag: usize,
    dpi: Option<DpiScope>,
}
impl WindowsBackend {
    pub(crate) fn new(tag: usize) -> Self {
        Self { tag, dpi: None }
    }
    fn submit(&self, input: INPUT) -> Result<(), &'static str> {
        if !cfg!(feature = "native-input") {
            return Err("native-input-disabled");
        }
        // SAFETY: fixed one-event INPUT; no secure desktop, elevation or cross-user bypass.
        if unsafe { SendInput(1, &input, size_of::<INPUT>() as i32) } != 1 {
            Err("input-refused")
        } else {
            Ok(())
        }
    }
    fn send_move(&self, (x, y): (i32, i32)) -> Result<(), &'static str> {
        unsafe {
            let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            if width <= 1 || height <= 1 {
                return Err("display-unavailable");
            }
            let dx = ((i64::from(x) - i64::from(vx)) * 65535 / i64::from(width - 1)).clamp(0, 65535)
                as i32;
            let dy = ((i64::from(y) - i64::from(vy)) * 65535 / i64::from(height - 1))
                .clamp(0, 65535) as i32;
            self.submit(INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx,
                        dy,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                        time: 0,
                        dwExtraInfo: self.tag,
                    },
                },
            })
        }
    }
    fn send(&self, action: &Action) -> Result<(), &'static str> {
        match action {
            Action::Key { code, down } => {
                let (scan, extended) = scan_code(code).ok_or("invalid-message")?;
                self.submit(INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: 0,
                            wScan: scan,
                            dwFlags: KEYEVENTF_SCANCODE
                                | if extended { KEYEVENTF_EXTENDEDKEY } else { 0 }
                                | if *down { 0 } else { KEYEVENTF_KEYUP },
                            time: 0,
                            dwExtraInfo: self.tag,
                        },
                    },
                })
            }
            Action::Button { button, down, .. } => {
                let flag = match (button.as_str(), *down) {
                    ("left", true) => MOUSEEVENTF_LEFTDOWN,
                    ("left", false) => MOUSEEVENTF_LEFTUP,
                    ("right", true) => MOUSEEVENTF_RIGHTDOWN,
                    ("right", false) => MOUSEEVENTF_RIGHTUP,
                    ("middle", true) => MOUSEEVENTF_MIDDLEDOWN,
                    ("middle", false) => MOUSEEVENTF_MIDDLEUP,
                    _ => return Err("invalid-message"),
                };
                self.mouse(flag, 0)
            }
            Action::Wheel { dx, dy, .. } => {
                if *dy != 0 {
                    self.mouse(MOUSEEVENTF_WHEEL, dy.saturating_neg() as u32)?
                }
                if *dx != 0 {
                    self.mouse(MOUSEEVENTF_HWHEEL, *dx as u32)?
                }
                Ok(())
            }
            _ => Err("invalid-message"),
        }
    }
    fn mouse(&self, flag: u32, data: u32) -> Result<(), &'static str> {
        self.submit(INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: data,
                    dwFlags: flag,
                    time: 0,
                    dwExtraInfo: self.tag,
                },
            },
        })
    }
}

/// Used only by the authenticated native child, never a hosted renderer. The
/// guardian validates the session and mirrors intent before sending each step.
pub(crate) struct ChildInput {
    tag: usize,
    owned: Ledger,
}
impl ChildInput {
    pub(crate) fn new(tag: usize) -> Self {
        Self {
            tag,
            owned: Ledger::default(),
        }
    }
    pub(crate) fn step(
        &mut self,
        source: &Source,
        action: &Action,
        valid: impl Fn() -> bool,
    ) -> Result<(), &'static str> {
        let mut backend = WindowsBackend::new(self.tag);
        let result = (|| {
            action.validate()?;
            if matches!(action, Action::Shortcut(_) | Action::ReleaseAll) {
                return Err("invalid-message");
            }
            backend.prepare()?;
            if !valid() {
                return Err("revoked");
            }
            if !self.owned.owns(action) {
                return Ok(());
            }
            backend.validate(source, action, &self.owned)?;
            if !valid() {
                return Err("revoked");
            }
            let down = matches!(
                action,
                Action::Key { down: true, .. } | Action::Button { down: true, .. }
            );
            if down {
                self.owned.plan(action)?;
            }
            backend.apply(source, action)?;
            if !down {
                self.owned.plan(action)?;
            }
            Ok(())
        })();
        backend.finish();
        result
    }
    pub(crate) fn cleanup(&mut self, source: &Source) -> Result<(), &'static str> {
        let mut backend = WindowsBackend::new(self.tag);
        let result = (|| {
            backend.prepare()?;
            for action in self.owned.clone().releases() {
                backend.cleanup_allowed()?;
                backend.apply(source, &action)?;
                self.owned.plan(&action)?;
            }
            Ok(())
        })();
        backend.finish();
        result
    }
}

impl Backend for WindowsBackend {
    fn prepare(&mut self) -> Result<(), &'static str> {
        if self.dpi.is_none() {
            self.dpi = Some(DpiScope::enter()?)
        };
        ordinary_desktop()
    }
    fn validate(
        &mut self,
        source: &Source,
        action: &Action,
        owned: &Ledger,
    ) -> Result<(), &'static str> {
        ordinary_desktop()?;
        source.current_monitor()?;
        // A pre-existing local mouse hold must never become a remote drag.
        unsafe {
            for (button, vk) in [
                ("left", VK_LBUTTON),
                ("middle", VK_MBUTTON),
                ("right", VK_RBUTTON),
            ] {
                if GetAsyncKeyState(vk as i32) < 0
                    && !owned.is_held(&Action::Button {
                        button: button.into(),
                        down: true,
                        source_gen: source.generation,
                    })
                {
                    return Err("local-input-held");
                }
            }
        }
        match action {
            Action::Move { .. } => (),
            _ => target(source, matches!(action, Action::Key { .. }))?,
        }
        if matches!(action, Action::Key { .. }) {
            unsafe {
                for (code, vk) in [
                    ("ControlLeft", VK_LCONTROL),
                    ("ControlRight", VK_RCONTROL),
                    ("AltLeft", VK_LMENU),
                    ("AltRight", VK_RMENU),
                    ("ShiftLeft", VK_LSHIFT),
                    ("ShiftRight", VK_RSHIFT),
                    ("MetaLeft", VK_LWIN),
                    ("MetaRight", VK_RWIN),
                ] {
                    if GetAsyncKeyState(vk as i32) < 0
                        && !owned.is_held(&Action::Key {
                            code: code.into(),
                            down: true,
                        })
                    {
                        return Err("local-modifier-held");
                    }
                }
            }
        }
        if !owned.is_held(action) {
            unsafe {
                let vk = match action {
                    Action::Key { code, down: true } => {
                        let (scan, extended) = scan_code(code).ok_or("invalid-message")?;
                        MapVirtualKeyW(
                            u32::from(scan) | if extended { 0xe000 } else { 0 },
                            MAPVK_VSC_TO_VK_EX,
                        ) as i32
                    }
                    Action::Button {
                        button, down: true, ..
                    } => match button.as_str() {
                        "left" => VK_LBUTTON as i32,
                        "middle" => VK_MBUTTON as i32,
                        "right" => VK_RBUTTON as i32,
                        _ => return Err("invalid-message"),
                    },
                    _ => 0,
                };
                if vk != 0 && GetAsyncKeyState(vk) < 0 {
                    return Err("local-input-held");
                }
            }
        }
        Ok(())
    }
    fn intent(&mut self, _ledger: &Ledger) -> Result<(), &'static str> {
        Ok(())
    }
    fn apply(&mut self, source: &Source, action: &Action) -> Result<(), &'static str> {
        match action {
            Action::Move { x, y, .. } => self.send_move(source.point(*x, *y)?),
            _ => self.send(action),
        }
    }
    fn fence(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
    fn cleanup_allowed(&mut self) -> Result<(), &'static str> {
        ordinary_desktop()
    }
    fn finish(&mut self) {
        self.dpi.take();
    }
}
