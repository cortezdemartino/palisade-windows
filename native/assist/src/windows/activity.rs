//! No keystroke contents are retained. Any non-endpoint input terminates control.
use std::{
    cell::RefCell,
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::*, System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*,
};
thread_local! { static CONTEXT: RefCell<Option<(Arc<AtomicBool>,usize)>> = const {RefCell::new(None)}; }
pub struct ActivityMonitor {
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl ActivityMonitor {
    pub fn start(stop: Arc<AtomicBool>, tag: usize) -> Result<Self, &'static str> {
        let (ready, receive) = mpsc::sync_channel(1);
        let flag = stop.clone();
        let worker = thread::spawn(move || unsafe {
            CONTEXT.with(|ctx| *ctx.borrow_mut() = Some((flag.clone(), tag)));
            let module = GetModuleHandleW(ptr::null());
            let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), module, 0);
            let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), module, 0);
            let timer = SetTimer(ptr::null_mut(), 0, 25, None);
            let ok = !keyboard.is_null() && !mouse.is_null() && timer != 0;
            let _ = ready.send(ok);
            let mut message = std::mem::zeroed();
            while ok
                && !flag.load(Ordering::SeqCst)
                && GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0
            {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            flag.store(true, Ordering::SeqCst);
            if !keyboard.is_null() {
                UnhookWindowsHookEx(keyboard);
            }
            if !mouse.is_null() {
                UnhookWindowsHookEx(mouse);
            }
            if timer != 0 {
                KillTimer(ptr::null_mut(), timer);
            }
            CONTEXT.with(|ctx| *ctx.borrow_mut() = None);
        });
        if !receive
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or(false)
        {
            stop.store(true, Ordering::SeqCst);
            let _ = worker.join();
            return Err("activity-monitor-unavailable");
        }
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}
impl Drop for ActivityMonitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn observe(own: bool, tag: usize) {
    CONTEXT.with(|ctx| {
        if let Some((stop, expected)) = &*ctx.borrow() {
            if !own || tag != *expected {
                stop.store(true, Ordering::SeqCst);
            }
        }
    });
}
unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if code >= 0 {
            let event = &*(lparam as *const KBDLLHOOKSTRUCT);
            observe(event.flags & LLKHF_INJECTED != 0, event.dwExtraInfo);
        }
        CallNextHookEx(ptr::null_mut(), code, wparam, lparam)
    }
}
unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if code >= 0 {
            let event = &*(lparam as *const MSLLHOOKSTRUCT);
            observe(event.flags & LLMHF_INJECTED != 0, event.dwExtraInfo);
        }
        CallNextHookEx(ptr::null_mut(), code, wparam, lparam)
    }
}
