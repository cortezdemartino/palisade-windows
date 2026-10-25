use std::{
    cell::Cell,
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};
use windows_sys::{
    Win32::{
        Foundation::*,
        System::{LibraryLoader::GetModuleHandleW, RemoteDesktop::*},
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::w,
};

pub struct NativeStop {
    #[cfg(test)]
    hwnd: usize,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
struct Context {
    stop: Arc<AtomicBool>,
    alive: Cell<bool>,
}
impl NativeStop {
    pub fn start(stop: Arc<AtomicBool>, probe: bool) -> Result<Self, &'static str> {
        let (ready, receive) = mpsc::sync_channel(1);
        let thread_stop = stop.clone();
        let worker = thread::spawn(move || unsafe {
            let module = GetModuleHandleW(ptr::null());
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: module,
                lpszClassName: w!("PalisadeAssistGuardian"),
                ..std::mem::zeroed()
            };
            RegisterClassW(&class);
            let context = Context {
                stop: thread_stop,
                alive: Cell::new(false),
            };
            let context_pointer = (&context as *const Context).cast_mut();
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST,
                class.lpszClassName,
                w!("Palisade Assist"),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                420,
                140,
                ptr::null_mut(),
                ptr::null_mut(),
                module,
                context_pointer.cast(),
            );
            if hwnd.is_null() {
                let _ = ready.send(Err("native-stop-window"));
                return;
            }
            let registered = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) != 0;
            let hotkey = probe
                || RegisterHotKey(
                    hwnd,
                    1,
                    MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_NOREPEAT,
                    VK_F10 as u32,
                ) != 0;
            if !registered || !hotkey {
                if registered {
                    WTSUnRegisterSessionNotification(hwnd);
                }
                if hotkey && !probe {
                    UnregisterHotKey(hwnd, 1);
                }
                DestroyWindow(hwnd);
                let _ = ready.send(Err("native-stop-unavailable"));
                return;
            }
            if SetTimer(hwnd, 1, 50, None) == 0 {
                WTSUnRegisterSessionNotification(hwnd);
                UnregisterHotKey(hwnd, 1);
                DestroyWindow(hwnd);
                let _ = ready.send(Err("native-stop-timer"));
                return;
            }
            if !probe {
                CreateWindowExW(
                    0,
                    w!("STATIC"),
                    w!("Native control is disabled in this build."),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    12,
                    380,
                    24,
                    hwnd,
                    ptr::null_mut(),
                    module,
                    ptr::null(),
                );
                CreateWindowExW(
                    0,
                    w!("BUTTON"),
                    w!("Stop Assist"),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    16,
                    44,
                    160,
                    30,
                    hwnd,
                    1usize as HMENU,
                    module,
                    ptr::null(),
                );
                ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
            if ready.send(Ok(hwnd as usize)).is_err() {
                context.stop.store(true, Ordering::SeqCst);
            }
            let mut message = std::mem::zeroed();
            while !context.stop.load(Ordering::SeqCst)
                && GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0
            {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            context.stop.store(true, Ordering::SeqCst);
            if context.alive.get() {
                KillTimer(hwnd, 1);
                if !probe {
                    UnregisterHotKey(hwnd, 1);
                }
                WTSUnRegisterSessionNotification(hwnd);
                DestroyWindow(hwnd);
            }
        });
        match receive
            .recv_timeout(std::time::Duration::from_secs(3))
            .map_err(|_| "native-stop-timeout")??
        {
            0 => Err("native-stop-window"),
            _hwnd => Ok(Self {
                #[cfg(test)]
                hwnd: _hwnd,
                stop,
                thread: Some(worker),
            }),
        }
    }
    #[cfg(test)]
    fn post(&self, message: u32) {
        unsafe {
            PostMessageW(self.hwnd as HWND, message, 0, 0);
        }
    }
}
impl Drop for NativeStop {
    fn drop(&mut self) {
        // No messages to an HWND that its owning thread may already have destroyed.
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
unsafe extern "system" fn window_proc(
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
        let context = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Context;
        if !context.is_null()
            && matches!(
                message,
                WM_CLOSE
                    | WM_HOTKEY
                    | WM_WTSSESSION_CHANGE
                    | WM_POWERBROADCAST
                    | WM_QUERYENDSESSION
                    | WM_ENDSESSION
                    | WM_COMMAND
            )
        {
            (*context).stop.store(true, Ordering::SeqCst);
            if message == WM_CLOSE {
                return 0;
            }
            if message == WM_QUERYENDSESSION {
                return 1;
            }
        }
        if message == WM_DESTROY {
            PostQuitMessage(0);
            return 0;
        }
        if message == WM_NCDESTROY && !context.is_null() {
            (*context).alive.set(false);
            (*context).stop.store(true, Ordering::SeqCst);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        }
        DefWindowProcW(hwnd, message, wparam, lparam)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_message_pump_stops_without_renderer_or_pipe() {
        // Posts to our own hidden test window. Does not lock/suspend this machine.
        for message in [WM_CLOSE, WM_WTSSESSION_CHANGE, WM_POWERBROADCAST, WM_HOTKEY] {
            let stop = Arc::new(AtomicBool::new(false));
            let surface = NativeStop::start(stop.clone(), true).unwrap();
            surface.post(message);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while !stop.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
                thread::sleep(std::time::Duration::from_millis(10));
            }
            assert!(stop.load(Ordering::SeqCst));
            drop(surface);
        }
    }
}
