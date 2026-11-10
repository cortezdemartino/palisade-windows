use super::*;
const SHIFT: Control = Control::Key {
    scan: 0x2a,
    extended: false,
};
fn emit(s: &mut ObservationState, event: NativeEvent, at: Instant) {
    let entry = StampedEvent::capture(&s.fence, event, at).unwrap();
    s.consume(entry).unwrap();
}
fn device(s: &mut ObservationState, id: usize, at: Instant) {
    emit(
        s,
        NativeEvent::Device {
            device: id,
            generation: 1,
        },
        at,
    );
}
fn ll(down: bool) -> NativeEvent {
    NativeEvent::LowLevel {
        control: SHIFT,
        down,
    }
}
fn raw(id: usize, down: bool) -> NativeEvent {
    NativeEvent::Raw {
        device: id,
        generation: 1,
        control: SHIFT,
        down,
    }
}
#[test]
fn both_release_orders_require_both_channels_and_stamping_does_not_pause_twice() {
    for raw_first in [false, true] {
        let fence = Arc::new(InputFence::default());
        let mut s = ObservationState::new(fence.clone());
        let at = Instant::now();
        device(&mut s, 1, at);
        emit(&mut s, ll(true), at);
        emit(&mut s, raw(1, true), at);
        assert_eq!(s.snapshot().held, 1);
        assert_eq!(s.snapshot().pending, 0);
        emit(
            &mut s,
            if raw_first { raw(1, false) } else { ll(false) },
            at,
        );
        assert_eq!(s.snapshot().held, 1);
        emit(
            &mut s,
            if raw_first { ll(false) } else { raw(1, false) },
            at,
        );
        assert_eq!(s.snapshot().held, 0);
        assert_eq!(s.snapshot().pending, 0);
        assert!(!s.snapshot().uncertain);
        assert_eq!(fence.generation(), 5);
        assert_eq!(s.snapshot().generation, 5);
        assert!(!s.snapshot().baseline_qualified);
    }
}
#[test]
fn overlapping_devices_and_repeat_cannot_release_another_physical_hold() {
    let mut s = ObservationState::new(Arc::default());
    let at = Instant::now();
    device(&mut s, 1, at);
    device(&mut s, 2, at);
    for id in [1, 2, 2] {
        emit(&mut s, ll(true), at);
        emit(&mut s, raw(id, true), at);
    }
    emit(&mut s, ll(false), at);
    emit(&mut s, raw(1, false), at);
    assert_eq!(s.snapshot().held, 1);
    emit(&mut s, raw(2, false), at);
    assert_eq!(s.snapshot().held, 1);
    emit(&mut s, ll(false), at);
    assert_eq!(s.snapshot().held, 0);
    assert!(!s.snapshot().uncertain);
}
#[test]
fn unmatched_transition_never_becomes_neutral_on_an_idle_timer() {
    let mut s = ObservationState::new(Arc::default());
    let at = Instant::now();
    device(&mut s, 1, at);
    emit(&mut s, ll(true), at);
    assert!(!s.expired(at + Duration::from_millis(249)));
    assert!(s.expired(at + Duration::from_millis(250)));
    emit(
        &mut s,
        NativeEvent::Uncertain,
        at + Duration::from_millis(250),
    );
    assert!(s.snapshot().uncertain);
    emit(&mut s, ll(false), at + Duration::from_secs(3));
    emit(&mut s, raw(1, false), at + Duration::from_secs(3));
    assert!(s.snapshot().uncertain);
    assert_eq!(s.snapshot().held, 1);
}
#[test]
fn injection_does_not_clear_host_holds_or_supply_local_consent() {
    let mut s = ObservationState::new(Arc::default());
    let at = Instant::now();
    device(&mut s, 1, at);
    emit(&mut s, ll(true), at);
    emit(&mut s, raw(1, true), at);
    emit(&mut s, NativeEvent::Uncertain, at);
    assert_eq!(s.snapshot().held, 1);
    assert!(s.snapshot().uncertain);
    // Neither a tag nor a synthetic UP exists in the native physical grammar.
    assert!(!s.snapshot().baseline_qualified);
}
#[test]
fn removal_handle_reuse_zero_device_wrong_generation_and_unknown_key_fail_closed() {
    for bad in [
        NativeEvent::Removed {
            device: 1,
            generation: 1,
        },
        NativeEvent::Device {
            device: 1,
            generation: 2,
        },
        NativeEvent::Raw {
            device: 0,
            generation: 1,
            control: SHIFT,
            down: false,
        },
        NativeEvent::Raw {
            device: 1,
            generation: 2,
            control: SHIFT,
            down: false,
        },
        NativeEvent::LowLevel {
            control: Control::Key {
                scan: 0xffff,
                extended: false,
            },
            down: true,
        },
    ] {
        let mut s = ObservationState::new(Arc::default());
        let at = Instant::now();
        device(&mut s, 1, at);
        emit(&mut s, ll(true), at);
        emit(&mut s, raw(1, true), at);
        emit(&mut s, bad, at);
        assert!(s.snapshot().uncertain);
        assert_eq!(s.snapshot().held, 1);
    }
}
#[test]
fn lost_foreign_and_out_of_order_stamps_cannot_repair_uncertainty() {
    let fence = Arc::new(InputFence::default());
    let mut s = ObservationState::new(fence.clone());
    let at = Instant::now();
    drop(StampedEvent::capture(&fence, NativeEvent::Activity, at).unwrap());
    emit(&mut s, NativeEvent::Activity, at);
    assert!(s.snapshot().uncertain);
    let foreign = StampedEvent::capture(&Arc::default(), NativeEvent::Activity, at).unwrap();
    assert!(s.consume(foreign).is_err());
    assert_eq!(fence.check(fence.generation()), Err("revoked"));
    let mut s = ObservationState::new(Arc::default());
    let later = StampedEvent::capture(&s.fence, NativeEvent::Activity, at).unwrap();
    let earlier = StampedEvent::capture(&s.fence, NativeEvent::Activity, at).unwrap();
    s.consume(earlier).unwrap();
    assert!(s.consume(later).is_err());
    assert!(s.snapshot().uncertain);
}

#[test]
fn international_mapping_reuses_remote_allowlist_without_expanding_it() {
    for code in [
        "KanaMode",
        "Lang1",
        "Lang2",
        "Convert",
        "NonConvert",
        "IntlYen",
        "NumpadEnter",
        "ControlRight",
    ] {
        let (scan, extended) = crate::input::scan_code2(code).unwrap();
        assert_eq!(key_name(scan, extended).as_deref(), Some(code));
    }
    for scan in [0x5b, 0x5c] {
        assert!(key_name(scan, true).is_none());
    }
}
#[test]
fn empty_observation_is_not_a_qualified_baseline_and_expiry_does_not_overtake_ingress() {
    let fence = Arc::new(InputFence::default());
    let mut s = ObservationState::new(fence.clone());
    let at = Instant::now();
    assert_eq!(
        s.snapshot().require_neutral(),
        Err("native-baseline-unqualified")
    );
    emit(&mut s, ll(true), at);
    let queued = StampedEvent::capture(
        &fence,
        NativeEvent::Activity,
        at + Duration::from_millis(10),
    )
    .unwrap();
    assert!(s.expired(at + Duration::from_millis(250)));
    assert_eq!(fence.generation(), 2);
    s.consume(queued).unwrap();
    emit(
        &mut s,
        NativeEvent::Uncertain,
        at + Duration::from_millis(250),
    );
    assert_eq!(s.snapshot().generation, 3);
    assert!(s.snapshot().uncertain);
}
#[test]
fn raw_first_and_button_overlap_use_the_same_native_ownership_rules() {
    let mut s = ObservationState::new(Arc::default());
    let at = Instant::now();
    device(&mut s, 1, at);
    let button = Control::Button(0);
    emit(
        &mut s,
        NativeEvent::Raw {
            device: 1,
            generation: 1,
            control: button,
            down: true,
        },
        at,
    );
    assert_eq!(s.snapshot().held, 1);
    emit(
        &mut s,
        NativeEvent::LowLevel {
            control: button,
            down: true,
        },
        at,
    );
    emit(
        &mut s,
        NativeEvent::LowLevel {
            control: button,
            down: false,
        },
        at,
    );
    assert_eq!(s.snapshot().held, 1);
    emit(
        &mut s,
        NativeEvent::Raw {
            device: 1,
            generation: 1,
            control: button,
            down: false,
        },
        at,
    );
    assert_eq!(s.snapshot().held, 0);
    assert!(!s.snapshot().uncertain);
}
#[test]
fn capacity_and_transition_disagreement_are_sticky_without_unbounded_growth() {
    let mut s = ObservationState::new(Arc::default());
    let at = Instant::now();
    for id in 1..=DEVICE_CAPACITY + 1 {
        device(&mut s, id, at);
    }
    assert!(s.snapshot().uncertain);
    assert_eq!(s.devices.len(), DEVICE_CAPACITY);
    let mut s = ObservationState::new(Arc::default());
    for _ in 0..EVENT_CAPACITY + 1 {
        emit(&mut s, ll(true), at);
    }
    assert!(s.snapshot().uncertain);
    assert_eq!(s.pending.len(), EVENT_CAPACITY);
    let mut s = ObservationState::new(Arc::default());
    device(&mut s, 1, at);
    emit(&mut s, ll(true), at);
    emit(&mut s, raw(1, false), at);
    assert!(s.snapshot().uncertain);
}
