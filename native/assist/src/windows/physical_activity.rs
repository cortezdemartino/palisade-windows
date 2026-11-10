//! Dormant host observation. No input injection, suppression, consent, grants or
//! page IPC. A baseline/healthy hook is NOT a cooperative delivery qualification.
use super::activity::ActivitySlot;
use crate::{
    cancellation::SessionStop,
    cooperative::{InputFence, LocalObservation},
    observation::{Control, EVENT_CAPACITY, NativeEvent, ObservationState, Snapshot, StampedEvent},
};
use std::{
    cell::{OnceCell, RefCell},
    collections::{BTreeMap, VecDeque},
    mem::{size_of, zeroed},
    ptr,
    rc::Rc,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::{
    Win32::{
        Foundation::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{Input::*, WindowsAndMessaging::*},
    },
    core::w,
};

pub struct NativeSample {
    pub observation: LocalObservation,
    pub snapshot: Snapshot,
}
pub struct PhysicalMonitor {
    quit: Arc<AtomicBool>,
    input_stop: Arc<AtomicBool>,
    fence: Arc<InputFence>,
    samples: mpsc::Receiver<NativeSample>,
    worker: Option<thread::JoinHandle<Result<(), &'static str>>>,
    failed: bool,
    _slot: ActivitySlot,
}
struct Sink {
    fence: Arc<InputFence>,
    queue: RefCell<VecDeque<StampedEvent>>,
    lost: AtomicBool,
}
struct Invalidate {
    input_stop: Arc<AtomicBool>,
    fence: Arc<InputFence>,
}
impl Drop for Invalidate {
    fn drop(&mut self) {
        self.input_stop.store(true, Ordering::SeqCst);
        self.fence.end();
    }
}
impl Sink {
    fn push(&self, event: NativeEvent) {
        // Arc clone and the preallocated queue do not allocate in this callback.
        // There are no registry/UI/network locks or reentrant UI calls here.
        let Ok(entry) = StampedEvent::capture(&self.fence, event, Instant::now()) else {
            self.lost.store(true, Ordering::SeqCst);
            return;
        };
        let Ok(mut queue) = self.queue.try_borrow_mut() else {
            self.lost.store(true, Ordering::SeqCst);
            return;
        };
        if queue.len() == EVENT_CAPACITY {
            self.lost.store(true, Ordering::SeqCst);
        } else {
            queue.push_back(entry);
        }
    }
}
type NativeDevices = (u64, BTreeMap<usize, (u64, u32)>);
thread_local! {
    static SINK: OnceCell<Rc<Sink>> = const {OnceCell::new()};
    static DEVICES: RefCell<NativeDevices> = const {RefCell::new((0,BTreeMap::new()))};
}
fn push(event: NativeEvent) {
    SINK.with(|s| {
        if let Some(s) = s.get() {
            s.push(event)
        }
    });
}
fn uncertain() {
    push(NativeEvent::Uncertain);
}

impl PhysicalMonitor {
    /// Native coordinator only, after session validation. This observes but
    /// never authorizes input or advertises localYield. No production caller yet.
    pub fn start(
        stop: SessionStop,
        input_stop: Arc<AtomicBool>,
        fence: Arc<InputFence>,
    ) -> Result<Self, &'static str> {
        if !cfg!(feature = "native-input") {
            return Err("native-input-disabled");
        }
        if stop.is_stopped() || input_stop.load(Ordering::SeqCst) {
            return Err("revoked");
        }
        super::input::ordinary_desktop()?;
        let slot = ActivitySlot::acquire()?;
        let quit = Arc::new(AtomicBool::new(false));
        let (ready, startup) = mpsc::sync_channel(1);
        let (output, samples) = mpsc::sync_channel(EVENT_CAPACITY);
        let (exit, ended, native_fence) = (quit.clone(), input_stop.clone(), fence.clone());
        let worker = thread::spawn(move || {
            let _invalidate = Invalidate {
                input_stop: ended.clone(),
                fence: native_fence.clone(),
            };
            let result = run(
                stop,
                exit,
                ended.clone(),
                native_fence.clone(),
                ready,
                output,
            );
            // Unexpected observation loss is terminal for input only. Independent
            // resource permissions are not revoked by this observer.
            ended.store(true, Ordering::SeqCst);
            native_fence.end();
            result
        });
        let mut monitor = Self {
            quit,
            input_stop,
            fence,
            samples,
            worker: Some(worker),
            failed: false,
            _slot: slot,
        };
        match startup.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => Ok(monitor),
            Ok(Err(error)) => {
                let _ = monitor.stop();
                Err(error)
            }
            Err(_) => {
                let _ = monitor.stop();
                Err("physical-observer-start")
            }
        }
    }
    pub fn try_recv(&self) -> Result<Option<NativeSample>, &'static str> {
        if self.quit.load(Ordering::SeqCst) || self.input_stop.load(Ordering::SeqCst) {
            return Err("physical-observer-ended");
        }
        match self.samples.try_recv() {
            Ok(v) => Ok(Some(v)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(_) => Err("physical-observer-ended"),
        }
    }
    pub fn stop(&mut self) -> Result<(), &'static str> {
        self.quit.store(true, Ordering::SeqCst);
        self.input_stop.store(true, Ordering::SeqCst);
        self.fence.end();
        if let Some(worker) = self.worker.take() {
            match worker.join() {
                Ok(Ok(())) => (),
                _ => self.failed = true,
            }
        }
        if self.failed {
            Err("physical-observer-cleanup")
        } else {
            Ok(())
        }
    }
}
impl Drop for PhysicalMonitor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn registrations() -> Result<Vec<RAWINPUTDEVICE>, &'static str> {
    let mut count = 0;
    if unsafe {
        GetRegisteredRawInputDevices(
            ptr::null_mut(),
            &mut count,
            size_of::<RAWINPUTDEVICE>() as u32,
        )
    } == u32::MAX
        || count > 128
    {
        return Err("raw-registration-query");
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut list = vec![RAWINPUTDEVICE::default(); count as usize];
    let found = unsafe {
        GetRegisteredRawInputDevices(
            list.as_mut_ptr(),
            &mut count,
            size_of::<RAWINPUTDEVICE>() as u32,
        )
    };
    if found == u32::MAX || found as usize > list.len() {
        return Err("raw-registration-query");
    }
    list.truncate(found as usize);
    Ok(list)
}
fn relevant(d: &RAWINPUTDEVICE) -> bool {
    d.usUsagePage == 1 && [2, 6].contains(&d.usUsage)
}
struct NativeResources {
    window: HWND,
    keyboard: HHOOK,
    mouse: HHOOK,
    registration_attempted: bool,
    timer: usize,
    cleanup_failed: Arc<AtomicBool>,
}
impl Drop for NativeResources {
    fn drop(&mut self) {
        unsafe {
            if !self.keyboard.is_null() && UnhookWindowsHookEx(self.keyboard) == 0 {
                self.cleanup_failed.store(true, Ordering::SeqCst);
            }
            if !self.mouse.is_null() && UnhookWindowsHookEx(self.mouse) == 0 {
                self.cleanup_failed.store(true, Ordering::SeqCst);
            }
            if self.timer != 0 && KillTimer(self.window, self.timer) == 0 {
                self.cleanup_failed.store(true, Ordering::SeqCst);
            }
            if self.registration_attempted {
                // Never unregister a replacement target owned by another component.
                match registrations() {
                    Ok(list) => {
                        for d in list
                            .iter()
                            .filter(|d| relevant(d) && d.hwndTarget == self.window)
                        {
                            let remove = RAWINPUTDEVICE {
                                usUsagePage: 1,
                                usUsage: d.usUsage,
                                dwFlags: RIDEV_REMOVE,
                                hwndTarget: ptr::null_mut(),
                            };
                            if RegisterRawInputDevices(
                                &remove,
                                1,
                                size_of::<RAWINPUTDEVICE>() as u32,
                            ) == 0
                            {
                                self.cleanup_failed.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                    Err(_) => self.cleanup_failed.store(true, Ordering::SeqCst),
                }
            }
            if !self.window.is_null() && DestroyWindow(self.window) == 0 {
                self.cleanup_failed.store(true, Ordering::SeqCst);
            }
        }
    }
}
fn window_class() -> Result<(), &'static str> {
    static CLASS: OnceLock<Result<(), &'static str>> = OnceLock::new();
    *CLASS.get_or_init(|| {
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: unsafe { GetModuleHandleW(ptr::null()) },
            lpszClassName: w!("PalisadeAssistPhysicalObservation.v1"),
            ..unsafe { zeroed() }
        };
        if unsafe { RegisterClassW(&class) } == 0 {
            Err("physical-observer-class")
        } else {
            Ok(())
        }
    })
}
fn run(
    stop: SessionStop,
    quit: Arc<AtomicBool>,
    input_stop: Arc<AtomicBool>,
    fence: Arc<InputFence>,
    ready: mpsc::SyncSender<Result<(), &'static str>>,
    output: mpsc::SyncSender<NativeSample>,
) -> Result<(), &'static str> {
    let cleanup_failed = Arc::new(AtomicBool::new(false));
    let result = (|| {
        let sink = Rc::new(Sink {
            fence: fence.clone(),
            queue: RefCell::new(VecDeque::with_capacity(EVENT_CAPACITY)),
            lost: AtomicBool::new(false),
        });
        SINK.with(|s| s.set(sink.clone()))
            .map_err(|_| "physical-observer-context")?;
        if registrations()?.iter().any(relevant) {
            return Err("raw-registration-busy");
        }
        window_class()?;
        let window = unsafe {
            CreateWindowExW(
                0,
                w!("PalisadeAssistPhysicalObservation.v1"),
                w!("Palisade Assist observation"),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                ptr::null_mut(),
                GetModuleHandleW(ptr::null()),
                ptr::null(),
            )
        };
        if window.is_null() {
            return Err("physical-observer-window");
        }
        let mut resources = NativeResources {
            window,
            keyboard: ptr::null_mut(),
            mouse: ptr::null_mut(),
            registration_attempted: false,
            timer: 0,
            cleanup_failed: cleanup_failed.clone(),
        };
        // Reverse drop order fences admission BEFORE any hook/window cleanup,
        // including early errors and unwinding.
        let _invalidate_before_cleanup = Invalidate {
            input_stop: input_stop.clone(),
            fence: fence.clone(),
        };
        let devices = [
            RAWINPUTDEVICE {
                usUsagePage: 1,
                usUsage: 2,
                dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
                hwndTarget: window,
            },
            RAWINPUTDEVICE {
                usUsagePage: 1,
                usUsage: 6,
                dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
                hwndTarget: window,
            },
        ];
        // Batch registration is not assumed transactional. On failure, query
        // and remove only targets actually bound to this owned window.
        resources.registration_attempted = true;
        if unsafe {
            RegisterRawInputDevices(devices.as_ptr(), 2, size_of::<RAWINPUTDEVICE>() as u32)
        } == 0
        {
            return Err("raw-registration");
        }
        unsafe {
            resources.keyboard = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_hook),
                GetModuleHandleW(ptr::null()),
                0,
            );
            resources.mouse = SetWindowsHookExW(
                WH_MOUSE_LL,
                Some(mouse_hook),
                GetModuleHandleW(ptr::null()),
                0,
            );
            resources.timer = SetTimer(window, 1, 25, None);
        }
        if resources.keyboard.is_null() || resources.mouse.is_null() || resources.timer == 0 {
            return Err("physical-observer-hooks");
        }
        let mut state = ObservationState::new(fence.clone());
        ready.send(Ok(())).map_err(|_| "physical-observer-start")?;
        let mut message = unsafe { zeroed() };
        while !quit.load(Ordering::SeqCst)
            && !input_stop.load(Ordering::SeqCst)
            && !stop.is_stopped()
        {
            if super::input::ordinary_desktop().is_err() {
                stop.revoke_launch();
                return Err("protected-desktop");
            }
            let got = unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) };
            if got <= 0 {
                return Err("physical-observer-message");
            }
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            // Deferred mapping/reconciliation and bounded delivery happen after
            // native callbacks release their borrows. Never call SendInput here.
            let entries: Vec<_> = sink.queue.borrow_mut().drain(..).collect();
            for entry in entries {
                let observation = state.consume(entry)?;
                output
                    .try_send(NativeSample {
                        observation,
                        snapshot: state.snapshot(),
                    })
                    .map_err(|_| "physical-observer-backpressure")?;
            }
            if sink.lost.swap(false, Ordering::SeqCst) {
                sink.push(NativeEvent::Uncertain);
            }
            if state.expired(Instant::now()) {
                sink.push(NativeEvent::Uncertain);
            }
            let active = registrations()?;
            if devices.iter().any(|expected| {
                !active.iter().any(|d| {
                    d.usUsagePage == 1
                        && d.usUsage == expected.usUsage
                        && d.hwndTarget == window
                        && d.dwFlags == expected.dwFlags
                })
            }) {
                return Err("raw-registration-replaced");
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = ready.try_send(Err(error));
    }
    if cleanup_failed.load(Ordering::SeqCst) {
        Err("physical-observer-cleanup")
    } else {
        result
    }
}
fn keyboard_event(event: &KBDLLHOOKSTRUCT, message: u32) -> NativeEvent {
    if event.flags & (LLKHF_INJECTED | LLKHF_LOWER_IL_INJECTED) != 0
        || event.scanCode == 0
        || event.scanCode > 255
        || ![WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP].contains(&message)
    {
        NativeEvent::Uncertain
    } else {
        NativeEvent::LowLevel {
            control: Control::Key {
                scan: event.scanCode as u16,
                extended: event.flags & LLKHF_EXTENDED != 0,
            },
            down: [WM_KEYDOWN, WM_SYSKEYDOWN].contains(&message),
        }
    }
}
unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        unsafe {
            push(keyboard_event(
                &*(lparam as *const KBDLLHOOKSTRUCT),
                wparam as u32,
            ));
        }
    }
    // No borrow crosses this call. Physical/foreign input is never suppressed.
    unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) }
}
unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        unsafe {
            let e = &*(lparam as *const MSLLHOOKSTRUCT);
            let event = if e.flags & (LLMHF_INJECTED | LLMHF_LOWER_IL_INJECTED) != 0 {
                NativeEvent::Uncertain
            } else {
                let transition = match wparam as u32 {
                    WM_LBUTTONDOWN => Some((0, true)),
                    WM_LBUTTONUP => Some((0, false)),
                    WM_RBUTTONDOWN => Some((1, true)),
                    WM_RBUTTONUP => Some((1, false)),
                    WM_MBUTTONDOWN => Some((2, true)),
                    WM_MBUTTONUP => Some((2, false)),
                    WM_XBUTTONDOWN | WM_XBUTTONUP => match e.mouseData >> 16 {
                        1 => Some((3, wparam as u32 == WM_XBUTTONDOWN)),
                        2 => Some((4, wparam as u32 == WM_XBUTTONDOWN)),
                        _ => {
                            uncertain();
                            None
                        }
                    },
                    _ => None,
                };
                transition.map_or(NativeEvent::Activity, |(button, down)| {
                    NativeEvent::LowLevel {
                        control: Control::Button(button),
                        down,
                    }
                })
            };
            push(event);
        }
    }
    unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) }
}
fn device_generation(raw: HANDLE, kind: u32) -> Option<u64> {
    if raw.is_null() {
        return None;
    }
    let mut info = RID_DEVICE_INFO {
        cbSize: size_of::<RID_DEVICE_INFO>() as u32,
        ..unsafe { zeroed() }
    };
    let mut size = size_of::<RID_DEVICE_INFO>() as u32;
    let copied = unsafe {
        GetRawInputDeviceInfoW(
            raw,
            RIDI_DEVICEINFO,
            (&mut info as *mut RID_DEVICE_INFO).cast(),
            &mut size,
        )
    };
    if copied != size_of::<RID_DEVICE_INFO>() as u32
        || size != size_of::<RID_DEVICE_INFO>() as u32
        || info.cbSize != size_of::<RID_DEVICE_INFO>() as u32
        || info.dwType != kind
    {
        return None;
    }
    DEVICES.with(|context| {
        let Ok(mut context) = context.try_borrow_mut() else {
            return None;
        };
        if let Some((generation, known)) = context.1.get(&(raw as usize)) {
            return (*known == kind).then_some(*generation);
        }
        if context.1.len() == 32 {
            return None;
        }
        context.0 = context.0.checked_add(1)?;
        let generation = context.0;
        context.1.insert(raw as usize, (generation, kind));
        push(NativeEvent::Device {
            device: raw as usize,
            generation,
        });
        Some(generation)
    })
}
fn valid_packet(data: &RAWINPUT, bytes: usize) -> bool {
    if bytes > size_of::<RAWINPUT>()
        || data.header.dwSize as usize != bytes
        || data.header.hDevice.is_null()
        || data.header.wParam > 1
    {
        return false;
    }
    let payload = match data.header.dwType {
        RIM_TYPEKEYBOARD => size_of::<RAWKEYBOARD>(),
        RIM_TYPEMOUSE => size_of::<RAWMOUSE>(),
        _ => return false,
    };
    if bytes != size_of::<RAWINPUTHEADER>() + payload {
        return false;
    }
    unsafe {
        if data.header.dwType == RIM_TYPEKEYBOARD {
            let key = data.data.keyboard;
            (1..=254).contains(&key.MakeCode)
                && key.Flags & !3 == 0
                && key.Reserved == 0
                && key.VKey != 255
                && if key.Flags & 1 == 0 {
                    [WM_KEYDOWN, WM_SYSKEYDOWN].contains(&key.Message)
                } else {
                    [WM_KEYUP, WM_SYSKEYUP].contains(&key.Message)
                }
        } else {
            let mouse = data.data.mouse;
            let flags = mouse.Anonymous.Anonymous.usButtonFlags;
            mouse.usFlags & !0x0b == 0
                && flags & !0x0fff == 0
                && (0..5).all(|button| (flags >> (button * 2)) & 3 != 3)
        }
    }
}
fn raw_received(raw: HRAWINPUT, reader: impl FnOnce(HRAWINPUT)) {
    push(NativeEvent::Activity);
    reader(raw);
}
fn read_raw(raw: HRAWINPUT) {
    let mut size = 0;
    if unsafe {
        GetRawInputData(
            raw,
            RID_INPUT,
            ptr::null_mut(),
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        )
    } != 0
        || size < size_of::<RAWINPUTHEADER>() as u32
        || size > size_of::<RAWINPUT>() as u32
    {
        uncertain();
        return;
    }
    let expected = size;
    let mut data: RAWINPUT = unsafe { zeroed() };
    let read = unsafe {
        GetRawInputData(
            raw,
            RID_INPUT,
            (&mut data as *mut RAWINPUT).cast(),
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        )
    };
    if read != expected || size != expected || data.header.dwSize != expected {
        uncertain();
        return;
    }
    if !valid_packet(&data, expected as usize) {
        uncertain();
        return;
    }
    let kind = data.header.dwType;
    let Some(generation) = device_generation(data.header.hDevice, kind) else {
        uncertain();
        return;
    };
    let device = data.header.hDevice as usize;
    unsafe {
        if kind == RIM_TYPEKEYBOARD {
            let key = data.data.keyboard;
            // E1/overrun/zero scans are explicit unsupported provenance, never
            // guessed from vkCode or character/layout translation.
            push(NativeEvent::Raw {
                device,
                generation,
                control: Control::Key {
                    scan: key.MakeCode,
                    extended: key.Flags & 2 != 0,
                },
                down: key.Flags & 1 == 0,
            });
        } else {
            let mouse = data.data.mouse;
            let flags = mouse.Anonymous.Anonymous.usButtonFlags;
            for button in 0..5 {
                for (bit, down) in [(button * 2, true), (button * 2 + 1, false)] {
                    if flags & (1 << bit) != 0 {
                        push(NativeEvent::Raw {
                            device,
                            generation,
                            control: Control::Button(button),
                            down,
                        });
                    }
                }
            }
            if mouse.lLastX != 0 || mouse.lLastY != 0 || flags & 0xc00 != 0 {
                push(NativeEvent::Activity);
            }
        }
    }
}
unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_INPUT => {
            // RAW may arrive before LL. Fence at receipt, before kernel packet
            // reads or device queries; decoding never reserves an earlier stamp
            // and appends it after later events.
            raw_received(lparam as HRAWINPUT, read_raw);
        }
        WM_INPUT_DEVICE_CHANGE => {
            let raw = lparam as usize;
            if wparam == GIDC_REMOVAL as usize {
                let removed =
                    DEVICES.with(|d| d.try_borrow_mut().ok().and_then(|mut d| d.1.remove(&raw)));
                if let Some((generation, _)) = removed {
                    push(NativeEvent::Removed {
                        device: raw,
                        generation,
                    });
                } else {
                    uncertain();
                }
            } else {
                uncertain();
            } // a new device never silently acquires neutrality
        }
        WM_DESTROY => unsafe { PostQuitMessage(0) },
        _ => (),
    }
    // Required RAWINPUT cleanup, including foreground RIM_INPUT.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

#[cfg(test)]
mod tests;
