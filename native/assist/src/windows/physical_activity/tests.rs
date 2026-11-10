//! Fake numeric packets/queues only. Never install hooks/register RawInput or
//! inspect physical devices on the execution machine.
use super::*;
fn key() -> RAWINPUT {
    let mut packet: RAWINPUT = unsafe { zeroed() };
    packet.header.dwType = RIM_TYPEKEYBOARD;
    packet.header.dwSize = (size_of::<RAWINPUTHEADER>() + size_of::<RAWKEYBOARD>()) as u32;
    packet.header.hDevice = 1usize as HANDLE;
    packet.data.keyboard = RAWKEYBOARD {
        MakeCode: 0x2a,
        Message: WM_KEYDOWN,
        ..Default::default()
    };
    packet
}
#[test]
fn numeric_packets_reject_size_type_device_and_unsupported_keyboard_provenance() {
    let good = key();
    let size = good.header.dwSize as usize;
    assert!(valid_packet(&good, size));
    for badsize in [0, size - 1, size + 1, usize::MAX] {
        assert!(!valid_packet(&good, badsize));
    }
    for mode in 0..8 {
        let mut bad = key();
        match mode {
            0 => bad.header.hDevice = ptr::null_mut(),
            1 => bad.header.dwType = RIM_TYPEHID,
            2 => bad.data.keyboard.MakeCode = 0,
            3 => bad.data.keyboard.Flags = 4,
            4 => bad.data.keyboard.VKey = 255,
            5 => bad.data.keyboard.Reserved = 1,
            6 => bad.header.wParam = 2,
            _ => bad.data.keyboard.Flags = 1,
        }
        assert!(!valid_packet(&bad, size), "case {mode}");
    }
    let mut up = key();
    up.data.keyboard.Flags = 1;
    up.data.keyboard.Message = WM_KEYUP;
    assert!(valid_packet(&up, size));
}
#[test]
fn raw_mouse_coalesced_opposite_transitions_and_attribute_changes_are_not_guessed() {
    let mut mouse: RAWINPUT = unsafe { zeroed() };
    mouse.header.dwType = RIM_TYPEMOUSE;
    mouse.header.hDevice = 1usize as HANDLE;
    mouse.header.dwSize = (size_of::<RAWINPUTHEADER>() + size_of::<RAWMOUSE>()) as u32;
    let size = mouse.header.dwSize as usize;
    assert!(valid_packet(&mouse, size));
    for flag in [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048] {
        mouse.data.mouse.Anonymous.ulButtons = flag;
        assert!(valid_packet(&mouse, size));
    }
    for flag in [3, 12, 48, 192, 768, 4096] {
        mouse.data.mouse.Anonymous.ulButtons = flag;
        assert!(!valid_packet(&mouse, size));
    }
    mouse.data.mouse.Anonymous.ulButtons = 0;
    mouse.data.mouse.usFlags = 4;
    assert!(!valid_packet(&mouse, size));
}
#[test]
fn bounded_hook_sink_fences_before_drain_and_latches_overflow_and_reentry() {
    let fence = Arc::new(InputFence::default());
    let sink = Sink {
        fence: fence.clone(),
        queue: RefCell::new(VecDeque::with_capacity(EVENT_CAPACITY)),
        lost: AtomicBool::new(false),
    };
    for _ in 0..EVENT_CAPACITY + 1 {
        sink.push(NativeEvent::Activity);
    }
    assert_eq!(sink.queue.borrow().len(), EVENT_CAPACITY);
    assert!(sink.lost.load(Ordering::SeqCst));
    assert_eq!(fence.check(0), Err("yielded"));
    let borrow = sink.queue.borrow_mut();
    sink.push(NativeEvent::Activity);
    assert!(sink.lost.load(Ordering::SeqCst));
    drop(borrow);
    // This is only observer ingress; a yield pause does not shut monitoring down.
    sink.queue.borrow_mut().clear();
    sink.push(NativeEvent::Activity);
    assert_eq!(sink.queue.borrow().len(), 1);
}
#[test]
fn monitor_stop_failure_stays_failed_and_legacy_and_cooperative_slots_are_exclusive() {
    let slot = ActivitySlot::acquire().unwrap();
    assert!(ActivitySlot::acquire().is_err());
    let (send, samples) = mpsc::sync_channel(1);
    drop(send);
    let stop = Arc::new(AtomicBool::new(false));
    let fence = Arc::new(InputFence::default());
    let mut monitor = PhysicalMonitor {
        quit: Arc::new(AtomicBool::new(false)),
        input_stop: stop.clone(),
        fence: fence.clone(),
        samples,
        worker: Some(thread::spawn(|| Err("fixture-observer-failure"))),
        failed: false,
        _slot: slot,
    };
    assert!(monitor.stop().is_err());
    assert!(monitor.stop().is_err());
    assert!(stop.load(Ordering::SeqCst));
    assert!(monitor.try_recv().is_err());
    assert_eq!(fence.check(0), Err("revoked"));
    drop(monitor);
    assert!(ActivitySlot::acquire().is_ok());
}
#[test]
fn early_error_and_unwind_invalidate_before_owned_cleanup() {
    struct Check {
        stop: Arc<AtomicBool>,
        fence: Arc<InputFence>,
    }
    impl Drop for Check {
        fn drop(&mut self) {
            assert!(self.stop.load(Ordering::SeqCst));
            assert_eq!(self.fence.check(0), Err("revoked"));
        }
    }
    let stop = Arc::new(AtomicBool::new(false));
    let fence = Arc::new(InputFence::default());
    let failed = std::panic::catch_unwind({
        let stop = stop.clone();
        let fence = fence.clone();
        move || {
            let _cleanup = Check {
                stop: stop.clone(),
                fence: fence.clone(),
            };
            let _invalidate = Invalidate {
                input_stop: stop,
                fence,
            };
            panic!("fixed observation unwind fixture");
        }
    });
    assert!(failed.is_err());
}

#[test]
fn matching_application_tag_and_cleanup_keyup_are_never_physical_input() {
    let mut event: KBDLLHOOKSTRUCT = unsafe { zeroed() };
    event.scanCode = 0x2a;
    event.dwExtraInfo = 73371;
    assert!(matches!(
        keyboard_event(&event, WM_KEYDOWN),
        NativeEvent::LowLevel { down: true, .. }
    ));
    for flags in [
        LLKHF_INJECTED,
        LLKHF_LOWER_IL_INJECTED,
        LLKHF_INJECTED | LLKHF_UP,
    ] {
        event.flags = flags;
        assert!(matches!(
            keyboard_event(&event, WM_KEYUP),
            NativeEvent::Uncertain
        ));
    }
}

#[test]
fn default_build_refuses_observer_start_before_any_native_registration() {
    if cfg!(feature = "native-input") {
        return;
    }
    let stop: SessionStop = Arc::new(AtomicBool::new(false)).into();
    assert!(matches!(
        PhysicalMonitor::start(stop, Arc::new(AtomicBool::new(false)), Arc::default()),
        Err("native-input-disabled")
    ));
}

#[test]
fn raw_first_receipt_fences_before_packet_reader_and_preserves_stamp_order() {
    thread::spawn(|| {
        let fence = Arc::new(InputFence::default());
        let sink = Rc::new(Sink {
            fence: fence.clone(),
            queue: RefCell::new(VecDeque::with_capacity(EVENT_CAPACITY)),
            lost: AtomicBool::new(false),
        });
        SINK.with(|s| assert!(s.set(sink.clone()).is_ok()));
        raw_received(ptr::null_mut(), |_| {
            assert_eq!(fence.check(0), Err("yielded"));
            assert_eq!(sink.queue.borrow().len(), 1);
            push(NativeEvent::Uncertain);
        });
        let mut state = ObservationState::new(fence.clone());
        for entry in sink.queue.borrow_mut().drain(..) {
            state.consume(entry).unwrap();
        }
        assert_eq!(state.snapshot().generation, 2);
        assert!(state.snapshot().uncertain);
        assert_eq!(fence.generation(), 2);
    })
    .join()
    .unwrap();
}
