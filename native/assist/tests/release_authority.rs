//! Real native authorization/executor, published fixtures and fake input only.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signer, SigningKey};
use palisade_assist_guardian::{
    cooperative::{Barrier, LocalEvent, LocalYield, State},
    executor::{Backend, Executor, Source, YieldDisposition},
    input::{Action, InputProfile, Ledger},
    stream::{Binding, SessionGate, SignedStreamGrant, SignedStreamLease},
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

#[derive(Default)]
struct Trace {
    applied: Vec<Action>,
    fail_release: bool,
    fenced: usize,
    quiesced: usize,
    cooperative: bool,
    fail_yield: bool,
    pause_on_down: Option<Arc<palisade_assist_guardian::cooperative::InputFence>>,
    pending_observation: Option<palisade_assist_guardian::cooperative::LocalObservation>,
    fail_empty_intents: usize,
    intent_calls: usize,
}
struct Fake(Rc<RefCell<Trace>>);
impl Backend for Fake {
    fn cooperative_ready(&self) -> bool {
        self.0.borrow().cooperative
    }
    fn prepare(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
    fn validate(&mut self, _: &Source, _: &Action, _: &Ledger) -> Result<(), &'static str> {
        Ok(())
    }
    fn intent(&mut self, ledger: &Ledger) -> Result<(), &'static str> {
        let mut trace = self.0.borrow_mut();
        trace.intent_calls += 1;
        if !ledger.held() && trace.fail_empty_intents > 0 {
            trace.fail_empty_intents -= 1;
            return Err("mirror-unconfirmed");
        }
        Ok(())
    }
    fn apply(&mut self, _: &Source, action: &Action) -> Result<(), &'static str> {
        let mut trace = self.0.borrow_mut();
        trace.applied.push(action.clone());
        if matches!(action, Action::Key { down: true, .. }) {
            if let Some(fence) = trace.pause_on_down.take() {
                trace.pending_observation = Some(fence.observation(LocalEvent::Activity).unwrap());
            }
        }
        if trace.fail_release && matches!(action, Action::Button { down: false, .. }) {
            trace.fail_release = false;
            return Err("partial-write");
        }
        Ok(())
    }
    fn fence(&mut self) -> Result<(), &'static str> {
        self.0.borrow_mut().fenced += 1;
        Ok(())
    }
    fn cleanup_allowed(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
    fn quiesce_for_yield(&mut self) -> Result<(), &'static str> {
        let mut trace = self.0.borrow_mut();
        if !trace.cooperative {
            return Err("cooperative-unavailable");
        }
        trace.quiesced += 1;
        Ok(())
    }
    fn yield_release(
        &mut self,
        source: &Source,
        action: &Action,
        local: &palisade_assist_guardian::cooperative::LocalHolds,
    ) -> Result<YieldDisposition, &'static str> {
        if self.0.borrow().fail_yield {
            return Err("delivery-uncertain");
        }
        if local.owns(action) {
            return Ok(YieldDisposition::HandedOver);
        }
        self.apply(source, action)?;
        Ok(YieldDisposition::Released)
    }
    fn finish(&mut self) {}
}
fn fixtures() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/assist-control-v2-release.json"
    ))
    .unwrap()
}
fn source() -> Source {
    Source {
        x: -800,
        y: 0,
        width: 800,
        height: 600,
        generation: 1,
    }
}
fn setup(capabilities: &[&str]) -> (SessionGate, u64) {
    let baseline: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/assist-control-v2.json"
    ))
    .unwrap();
    let now = baseline["now"].as_u64().unwrap();
    let mut binding: Binding = serde_json::from_value(json!({
        "session":fixtures()["session"], "room":baseline["expect"]["room"], "stream":baseline["expect"]["stream"],
        "publisherAccount":baseline["expect"]["publisherAccount"], "publisherDevice":baseline["expect"]["publisherDevice"],
        "helperAccount":baseline["expect"]["helperAccount"], "helperDevice":baseline["expect"]["helperDevice"],
        "helperTransport":baseline["expect"]["helperTransport"], "endpoint":"endpoint_release_fixture"
    })).unwrap();
    binding.session = fixtures()["session"].as_str().unwrap().into();
    let key = SigningKey::from_bytes(&[43; 32]);
    let t = Instant::now();
    let mut gate = SessionGate::new(key.verifying_key().to_bytes(), binding.clone(), now, 0, t);
    let mut lease: SignedStreamLease =
        serde_json::from_value(baseline["leases"]["valid"][0]["signed"].clone()).unwrap();
    lease.lease.session = binding.session.clone();
    lease.lease.generation = 4;
    lease.lease.grants.clear();
    for &capability in capabilities {
        let id = match capability {
            "control_mouse" => "sgrant_release_mouse_0001",
            "control_keyboard" => "sgrant_release_keyboard_01",
            "clipboard_to_host" => "sgrant_release_clipboard1",
            _ => panic!("unknown fixture capability"),
        };
        let mut grant: SignedStreamGrant =
            serde_json::from_value(baseline["grants"]["valid"][1]["signed"].clone()).unwrap();
        grant.grant.session = binding.session.clone();
        grant.grant.grant_id = id.into();
        grant.grant.capability = capability.into();
        grant.grant.generation = 4;
        grant.grant.endpoint = capability
            .starts_with("control_")
            .then(|| binding.endpoint.clone());
        grant.signature = URL_SAFE_NO_PAD.encode(key.sign(&grant.grant.canonical()).to_bytes());
        gate.admit(&grant, now).unwrap();
        gate.consent(id, now).unwrap();
        lease.lease.grants.push(id.into());
    }
    lease.lease.grants.sort();
    lease.signature = URL_SAFE_NO_PAD.encode(
        key.sign(&serde_json::to_vec(&lease.lease).unwrap())
            .to_bytes(),
    );
    gate.verify_peer(&binding.helper_transport).unwrap();
    gate.refresh(&lease, now, t, t).unwrap();
    (gate, now)
}
fn dispatch(
    gate: &mut SessionGate,
    executor: &mut Executor<Fake>,
    message: &Value,
    now: u64,
) -> Result<(), &'static str> {
    let t = Instant::now();
    let (permit, input, _) =
        gate.permit_packet(&message.to_string(), InputProfile::Profile2, 1, now, (t, t))?;
    executor.dispatch(permit, gate, &source(), &input.action, || {
        Ok((now, Instant::now()))
    })
}
fn release(grant: &str, seq: u32) -> Value {
    json!({"p":"palisade.assist.control","v":2,"session":fixtures()["session"],"grant":grant,"gen":4,"seq":seq,"kind":"release-all"})
}
fn releases(actions: &[Action]) -> Value {
    let mut buttons = vec![];
    let mut keys = vec![];
    for action in actions {
        match action {
            Action::Button {
                button,
                down: false,
                ..
            } => buttons.push(button.clone()),
            Action::Key { code, down: false } => keys.push(code.clone()),
            _ => (),
        }
    }
    buttons.sort();
    keys.sort();
    json!({"buttons":buttons,"keys":keys})
}

#[test]
fn canonical_release_scenarios_reach_native_gate_and_executor() {
    for scenario in fixtures()["scenarios"].as_array().unwrap() {
        let caps: Vec<_> = scenario["grants"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap())
            .collect();
        let (mut gate, now) = setup(&caps);
        let trace = Rc::new(RefCell::new(Trace::default()));
        let mut executor =
            Executor::new(Fake(trace.clone()), Arc::new(AtomicBool::new(false)), true);
        for step in scenario["steps"].as_array().unwrap() {
            let before = trace.borrow().applied.len();
            if let Some(error) = step["error"].as_str() {
                if step["grant"]["active"] == false {
                    // The canonical refusal vector is not a revocation transition.
                    // A separate revoked gate tests it without reviving the active one.
                    let (mut revoked, _) = setup(&caps);
                    revoked.revoke(step["message"]["grant"].as_str().unwrap());
                    assert_eq!(
                        dispatch(&mut revoked, &mut executor, &step["message"], now),
                        Err(error)
                    );
                } else {
                    assert_eq!(
                        dispatch(&mut gate, &mut executor, &step["message"], now),
                        Err(error)
                    );
                }
                assert_eq!(trace.borrow().applied.len(), before);
            } else {
                dispatch(&mut gate, &mut executor, &step["message"], now).unwrap();
                assert_eq!(
                    releases(&trace.borrow().applied[before..]),
                    step["expect"],
                    "{}",
                    step
                );
            }
        }
        if !scenario["teardown"].is_null() {
            gate.stop();
            let before = trace.borrow().applied.len();
            executor.release_owned(&source()).unwrap();
            assert_eq!(
                releases(&trace.borrow().applied[before..]),
                scenario["teardown"]
            );
            assert_eq!(trace.borrow().fenced, 1);
            assert!(!executor.held());
        }
    }
}

#[test]
fn identical_grant_strings_in_another_gate_cannot_release_or_adopt_a_hold() {
    for repeat in [false, true] {
        let (mut gate, now) = setup(&["control_keyboard"]);
        let (mut other, _) = setup(&["control_keyboard"]);
        let trace = Rc::new(RefCell::new(Trace::default()));
        let mut executor =
            Executor::new(Fake(trace.clone()), Arc::new(AtomicBool::new(false)), true);
        let mut press = release("sgrant_release_keyboard_01", 1);
        press["kind"] = "key".into();
        press["code"] = "ShiftLeft".into();
        press["down"] = true.into();
        dispatch(&mut gate, &mut executor, &press, now).unwrap();
        dispatch(
            &mut other,
            &mut executor,
            &release("sgrant_release_keyboard_01", 1),
            now,
        )
        .unwrap();
        let mut up = press.clone();
        up["down"] = false.into();
        up["seq"] = 2.into();
        dispatch(&mut other, &mut executor, &up, now).unwrap();
        assert!(executor.held());
        assert_eq!(trace.borrow().applied.len(), 1);
        // A foreign repeat/down cannot take ownership. The failure independently
        // fences and clears the original hold; it never injects a foreign DOWN.
        press["seq"] = 3.into();
        if repeat {
            press["kind"] = "input-key".into();
            press["repeat"] = true.into();
            press["t"] = 100.into();
        }
        assert_eq!(
            dispatch(&mut other, &mut executor, &press, now),
            Err("held-input")
        );
        assert_eq!(trace.borrow().fenced, 1);
        assert_eq!(
            releases(&trace.borrow().applied),
            json!({"buttons":[],"keys":["ShiftLeft"]})
        );
        assert!(!executor.held());
    }
}

#[test]
fn carried_grant_refusals_do_not_search_for_another_active_input_grant() {
    let (mut gate, now) = setup(&["control_mouse", "control_keyboard", "clipboard_to_host"]);
    let t = Instant::now();
    let check = |gate: &mut SessionGate, packet: Value, expected| {
        let result =
            gate.permit_packet(&packet.to_string(), InputProfile::Profile2, 1, now, (t, t));
        assert!(matches!(result, Err(error) if error == expected));
    };
    check(
        &mut gate,
        release("sgrant_release_clipboard1", 1),
        "not-granted",
    );
    let mut wrong = release("sgrant_release_mouse_0001", 1);
    wrong["gen"] = 3.into();
    check(&mut gate, wrong, "stale-generation");
    gate.revoke("sgrant_release_mouse_0001");
    check(
        &mut gate,
        release("sgrant_release_mouse_0001", 1),
        "revoked",
    );
    // The unrelated keyboard grant is still active, but cannot rescue mouse authority.
    assert!(
        gate.authorize("sgrant_release_keyboard_01", "control_keyboard", now, t)
            .is_ok()
    );
    let mut wrong = release("sgrant_release_keyboard_01", 1);
    wrong["kind"] = "button".into();
    wrong["button"] = "left".into();
    wrong["down"] = false.into();
    wrong["sourceGen"] = 1.into();
    check(&mut gate, wrong, "not-granted");
}

#[test]
fn release_budget_bypass_preserves_authority_sequence_and_expiry() {
    let (mut gate, now) = setup(&["control_mouse"]);
    let t = Instant::now();
    for seq in 1..=240 {
        let mut value = release("sgrant_release_mouse_0001", seq);
        value["kind"] = "pointer-move".into();
        value["x"] = 0.5.into();
        value["y"] = 0.5.into();
        value["sourceGen"] = 1.into();
        gate.permit_packet(&value.to_string(), InputProfile::Profile2, 1, now, (t, t))
            .unwrap();
    }
    let packet = release("sgrant_release_mouse_0001", 241).to_string();
    assert!(
        gate.permit_packet(&packet, InputProfile::Profile2, 1, now, (t, t))
            .is_ok()
    );
    assert!(matches!(
        gate.permit_packet(&packet, InputProfile::Profile2, 1, now, (t, t)),
        Err("replay")
    ));
    let fresh = release("sgrant_release_mouse_0001", 242).to_string();
    gate.revoke("sgrant_release_mouse_0001");
    assert!(matches!(
        gate.permit_packet(&fresh, InputProfile::Profile2, 1, now, (t, t)),
        Err("revoked")
    ));
    let (mut expired, _) = setup(&["control_mouse"]);
    assert!(
        expired
            .permit_packet(
                &fresh,
                InputProfile::Profile2,
                1,
                now + 20_000,
                (t, t + std::time::Duration::from_secs(20))
            )
            .is_err()
    );
}

#[test]
fn failed_scoped_release_fences_and_cleans_every_owner() {
    let (mut gate, now) = setup(&["control_mouse", "control_keyboard"]);
    let trace = Rc::new(RefCell::new(Trace::default()));
    let mut executor = Executor::new(Fake(trace.clone()), Arc::new(AtomicBool::new(false)), true);
    let fixture = fixtures();
    let steps = fixture["scenarios"][1]["steps"].as_array().unwrap();
    for step in &steps[..2] {
        dispatch(&mut gate, &mut executor, &step["message"], now).unwrap();
    }
    trace.borrow_mut().fail_release = true;
    assert_eq!(
        dispatch(
            &mut gate,
            &mut executor,
            &release("sgrant_release_mouse_0001", 5),
            now
        ),
        Err("partial-write")
    );
    assert_eq!(trace.borrow().fenced, 1);
    assert!(!executor.held());
    assert!(trace.borrow().applied.contains(&Action::Key {
        code: "ShiftLeft".into(),
        down: false
    }));
    assert!(
        gate.authorize(
            "sgrant_release_keyboard_01",
            "control_keyboard",
            now,
            Instant::now()
        )
        .is_err()
    );
}

fn barrier_packet(grant: &str, seq: u32, epoch: u32, sender: u64) -> Barrier {
    Barrier::parse(
        &json!({"p":"palisade.assist.control","v":2,"session":fixtures()["session"],
        "grant":grant,"gen":4,"seq":seq,"kind":"input-barrier","epoch":epoch,"t":sender})
        .to_string(),
        true,
    )
    .unwrap()
}

#[test]
fn cooperative_barriers_use_signed_input_authority_replay_and_release_like_age_checks() {
    for capability in ["control_mouse", "control_keyboard"] {
        let id = if capability == "control_mouse" {
            "sgrant_release_mouse_0001"
        } else {
            "sgrant_release_keyboard_01"
        };
        let (mut gate, now) = setup(&[capability]);
        let t = Instant::now();
        let authority = gate.input_authority(&source(), now, t).unwrap();
        let mut scheduler = LocalYield::new(authority.clone());
        let packet = barrier_packet(id, 1, 0, 0);
        let verified = gate.verify_barrier(&packet, now, (t, t)).unwrap();
        scheduler.barrier(verified, 0, t, &authority).unwrap();
        assert_eq!(scheduler.state(), State::Active);
        assert!(matches!(
            gate.verify_barrier(&packet, now, (t, t)),
            Err("replay")
        ));
        scheduler
            .observe(LocalEvent::Activity, 1, &authority, false)
            .unwrap();
        let later = t + std::time::Duration::from_secs(3);
        let current = gate.input_authority(&source(), now + 3_000, later).unwrap();
        scheduler.tick_at(3_000, later, &current, true).unwrap();
        // Sender timestamp repeats zero; queue arrival is fresh. A barrier is
        // release-like for age, never stale merely because the host was busy.
        let next = barrier_packet(id, 2, 1, 0);
        let verified = gate
            .verify_barrier(&next, now + 3_000, (later, later))
            .unwrap();
        scheduler.barrier(verified, 3_000, later, &current).unwrap();
        assert_eq!(scheduler.state(), State::Active);
        gate.revoke(id);
        assert!(
            gate.verify_barrier(&barrier_packet(id, 3, 1, 0), now + 3_000, (later, later))
                .is_err()
        );
        assert!(gate.input_authority(&source(), now + 3_000, later).is_err());
    }
    let (mut resources, now) = setup(&["clipboard_to_host"]);
    let t = Instant::now();
    assert!(resources.input_authority(&source(), now, t).is_err());
    assert!(matches!(
        resources.verify_barrier(
            &barrier_packet("sgrant_release_clipboard1", 1, 0, 0),
            now,
            (t, t)
        ),
        Err("not-granted")
    ));
}

#[test]
fn yield_settles_existing_grant_ledgers_without_releasing_host_holds_or_revoking_grants() {
    let (mut gate, now) = setup(&["control_mouse", "control_keyboard"]);
    let trace = Rc::new(RefCell::new(Trace {
        cooperative: true,
        ..Trace::default()
    }));
    let stop = Arc::new(AtomicBool::new(false));
    let mut executor = Executor::new(Fake(trace.clone()), stop.clone(), true);
    let t = Instant::now();
    let authority = gate.input_authority(&source(), now, t).unwrap();
    let mut scheduler = LocalYield::new(authority.clone());
    let verified = gate
        .verify_barrier(
            &barrier_packet("sgrant_release_mouse_0001", 0, 0, 0),
            now,
            (t, t),
        )
        .unwrap();
    scheduler.barrier(verified, 0, t, &authority).unwrap();
    let fixture = fixtures();
    for step in &fixture["scenarios"][1]["steps"].as_array().unwrap()[..2] {
        dispatch(&mut gate, &mut executor, &step["message"], now).unwrap();
    }
    scheduler
        .observe(
            LocalEvent::Key {
                code: "ShiftLeft".into(),
                down: true,
            },
            1,
            &authority,
            false,
        )
        .unwrap();
    scheduler
        .observe(
            LocalEvent::Button {
                button: "right".into(),
                down: true,
            },
            2,
            &authority,
            false,
        )
        .unwrap();
    let before = trace.borrow().applied.len();
    let completed = executor
        .yield_owned(&source(), scheduler.local_holds())
        .unwrap();
    assert_eq!(completed.len(), 2);
    assert!(
        completed
            .iter()
            .all(|(_, disposition)| *disposition == YieldDisposition::HandedOver)
    );
    assert_eq!(trace.borrow().applied.len(), before);
    assert_eq!(trace.borrow().fenced, 0);
    assert_eq!(trace.borrow().quiesced, 1);
    assert!(!stop.load(std::sync::atomic::Ordering::SeqCst));
    assert!(executor.yield_cleanup_complete());
    assert!(
        gate.authorize("sgrant_release_mouse_0001", "control_mouse", now, t)
            .is_ok()
    );
    assert!(
        gate.authorize("sgrant_release_keyboard_01", "control_keyboard", now, t)
            .is_ok()
    );
    // Neither delayed helper release nor later terminal mirror cleanup may
    // synthesize an UP for the physical person's handed-over holds.
    dispatch(
        &mut gate,
        &mut executor,
        &release("sgrant_release_mouse_0001", 5),
        now,
    )
    .unwrap();
    dispatch(
        &mut gate,
        &mut executor,
        &release("sgrant_release_keyboard_01", 4),
        now,
    )
    .unwrap();
    executor.release_owned(&source()).unwrap();
    assert_eq!(trace.borrow().applied.len(), before);
}

#[test]
fn uncertain_yield_delivery_retains_ledger_and_blocks_cleanup_completion() {
    let (mut gate, now) = setup(&["control_mouse"]);
    let trace = Rc::new(RefCell::new(Trace {
        cooperative: true,
        fail_yield: true,
        ..Trace::default()
    }));
    let mut executor = Executor::new(Fake(trace.clone()), Arc::new(AtomicBool::new(false)), true);
    dispatch(
        &mut gate,
        &mut executor,
        &fixtures()["scenarios"][0]["steps"][0]["message"],
        now,
    )
    .unwrap();
    let t = Instant::now();
    let authority = gate.input_authority(&source(), now, t).unwrap();
    let scheduler = LocalYield::new(authority);
    assert!(matches!(
        executor.yield_owned(&source(), scheduler.local_holds()),
        Err("delivery-uncertain")
    ));
    assert!(executor.held());
    assert!(!executor.yield_cleanup_complete());
    trace.borrow_mut().fail_yield = false;
    executor
        .yield_owned(&source(), scheduler.local_holds())
        .unwrap();
    assert!(executor.yield_cleanup_complete());
    assert_eq!(trace.borrow().fenced, 0);
}

#[test]
fn local_fence_interrupts_an_expanded_shortcut_without_revoking_or_repressing() {
    let (mut gate, now) = setup(&["control_keyboard"]);
    let t = Instant::now();
    let authority = gate.input_authority(&source(), now, t).unwrap();
    let mut scheduler = LocalYield::new(authority.clone());
    let verified = gate
        .verify_barrier(
            &barrier_packet("sgrant_release_keyboard_01", 0, 0, 0),
            now,
            (t, t),
        )
        .unwrap();
    scheduler.barrier(verified, 0, t, &authority).unwrap();
    let trace = Rc::new(RefCell::new(Trace {
        cooperative: true,
        pause_on_down: Some(scheduler.input_fence()),
        ..Trace::default()
    }));
    let stop = Arc::new(AtomicBool::new(false));
    let mut executor = Executor::new(Fake(trace.clone()), stop.clone(), true);
    let packet = json!({"p":"palisade.assist.control","v":2,"session":fixtures()["session"],
        "grant":"sgrant_release_keyboard_01","gen":4,"seq":1,"kind":"input-shortcut","name":"copy","t":1}).to_string();
    let (permit, message, _) = gate
        .permit_packet(&packet, InputProfile::Profile2, 1, now, (t, t))
        .unwrap();
    assert_eq!(
        executor.dispatch_cooperative(
            permit,
            &mut gate,
            &source(),
            &message.action,
            &mut scheduler,
            || Ok((now, Instant::now()))
        ),
        Err("yielded")
    );
    assert_eq!(
        trace.borrow().applied,
        vec![Action::Key {
            code: "ControlLeft".into(),
            down: true
        }]
    );
    assert_eq!(trace.borrow().fenced, 0);
    assert!(!stop.load(std::sync::atomic::Ordering::SeqCst));
    assert!(!executor.yield_cleanup_complete());
    let observation = trace.borrow_mut().pending_observation.take().unwrap();
    scheduler
        .observe_native(observation, 1, &authority, false)
        .unwrap();
    executor
        .yield_owned(&source(), scheduler.local_holds())
        .unwrap();
    assert_eq!(
        trace.borrow().applied[1],
        Action::Key {
            code: "ControlLeft".into(),
            down: false
        }
    );
    assert_eq!(trace.borrow().applied.len(), 2);
    assert!(executor.yield_cleanup_complete());
    assert!(
        gate.authorize(
            "sgrant_release_keyboard_01",
            "control_keyboard",
            now,
            Instant::now()
        )
        .is_ok()
    );
}

#[test]
fn acknowledged_handover_keeps_mirror_retry_owned_through_terminal_cleanup_and_drop() {
    for drop_cleanup in [false, true] {
        let (mut gate, now) = setup(&["control_mouse"]);
        let trace = Rc::new(RefCell::new(Trace {
            cooperative: true,
            ..Trace::default()
        }));
        let mut executor =
            Executor::new(Fake(trace.clone()), Arc::new(AtomicBool::new(false)), true);
        dispatch(
            &mut gate,
            &mut executor,
            &fixtures()["scenarios"][0]["steps"][0]["message"],
            now,
        )
        .unwrap();
        let t = Instant::now();
        let authority = gate.input_authority(&source(), now, t).unwrap();
        let mut scheduler = LocalYield::new(authority.clone());
        scheduler
            .observe(
                LocalEvent::Button {
                    button: "left".into(),
                    down: true,
                },
                1,
                &authority,
                false,
            )
            .unwrap();
        trace.borrow_mut().fail_empty_intents = 1;
        assert!(matches!(
            executor.yield_owned(&source(), scheduler.local_holds()),
            Err("mirror-unconfirmed")
        ));
        assert!(!executor.held());
        assert!(!executor.yield_cleanup_complete());
        let before = trace.borrow().intent_calls;
        if !drop_cleanup {
            // Failed terminal synchronization must also retain pending proof.
            trace.borrow_mut().fail_empty_intents = 1;
            assert_eq!(executor.release_owned(&source()), Err("mirror-unconfirmed"));
            assert!(!executor.yield_cleanup_complete());
            executor.release_owned(&source()).unwrap();
            assert!(executor.yield_cleanup_complete());
        }
        drop(executor);
        assert!(trace.borrow().intent_calls > before);
        assert_eq!(
            trace.borrow().applied.len(),
            1,
            "the host-owned button never receives an UP"
        );
    }
}

#[test]
fn barrier_received_after_readiness_survives_slow_dispatch_but_preserves_ingress() {
    let (mut gate, now) = setup(&["control_mouse"]);
    let t = Instant::now();
    let authority = gate.input_authority(&source(), now, t).unwrap();
    let mut scheduler = LocalYield::new(authority.clone());
    let verified = gate
        .verify_barrier(
            &barrier_packet("sgrant_release_mouse_0001", 0, 0, 0),
            now,
            (t, t),
        )
        .unwrap();
    scheduler.barrier(verified, 0, t, &authority).unwrap();
    scheduler
        .observe(LocalEvent::Activity, 1, &authority, true)
        .unwrap();
    let ready = t + std::time::Duration::from_secs(3);
    scheduler.tick_at(3_000, ready, &authority, true).unwrap();
    let delayed = ready + std::time::Duration::from_secs(1);
    let verified = gate
        .verify_barrier(
            &barrier_packet("sgrant_release_mouse_0001", 1, 1, 0),
            now + 4_000,
            (ready, delayed),
        )
        .unwrap();
    scheduler
        .barrier(verified, 4_000, delayed, &authority)
        .unwrap();
    assert_eq!(scheduler.state(), State::Active);
}

#[test]
fn first_terminal_release_with_failed_mirror_flush_is_retained_by_drop() {
    let (mut gate, now) = setup(&["control_mouse"]);
    let trace = Rc::new(RefCell::new(Trace::default()));
    let mut executor = Executor::new(Fake(trace.clone()), Arc::new(AtomicBool::new(false)), true);
    dispatch(
        &mut gate,
        &mut executor,
        &fixtures()["scenarios"][0]["steps"][0]["message"],
        now,
    )
    .unwrap();
    trace.borrow_mut().fail_empty_intents = 1;
    assert_eq!(executor.release_owned(&source()), Err("mirror-unconfirmed"));
    assert!(!executor.held());
    assert!(!executor.yield_cleanup_complete());
    let before = trace.borrow().intent_calls;
    drop(executor);
    assert!(trace.borrow().intent_calls > before);
    assert_eq!(
        trace.borrow().applied.len(),
        2,
        "retry only the mirror; never a second UP"
    );
}

fn managed(gate: &mut SessionGate, trace: Rc<RefCell<Trace>>) -> Executor<Fake> {
    let reservation = gate.reserve_executor().unwrap();
    Executor::registered(
        Fake(trace),
        Arc::new(AtomicBool::new(false)),
        true,
        reservation,
    )
}

#[test]
fn registered_cleanup_is_one_shot_and_keeps_resource_authority_and_deadline() {
    let (mut gate, now) = setup(&["control_mouse", "clipboard_to_host"]);
    let deadline = gate.deadline();
    let generation = gate.highest_generation();
    let trace = Rc::new(RefCell::new(Trace::default()));
    let mut executor = managed(&mut gate, trace.clone());
    dispatch(
        &mut gate,
        &mut executor,
        &fixtures()["scenarios"][0]["steps"][0]["message"],
        now,
    )
    .unwrap();
    assert!(matches!(
        gate.require_input_cleanup(),
        Err("input-still-authorized")
    ));
    gate.suspend_input();
    let requirement = gate.require_input_cleanup().unwrap();
    assert!(matches!(
        gate.require_input_cleanup(),
        Err("cleanup-pending")
    ));
    assert!(matches!(gate.reserve_executor(), Err("cleanup-pending")));
    trace.borrow_mut().fail_empty_intents = 1;
    assert!(matches!(
        executor.release_for(&requirement, &source()),
        Err("mirror-unconfirmed")
    ));
    assert!(!executor.held());
    assert!(!executor.yield_cleanup_complete());
    assert!(
        gate.authorize(
            "sgrant_release_clipboard1",
            "clipboard_to_host",
            now,
            Instant::now()
        )
        .is_ok()
    );
    let receipt = executor.release_for(&requirement, &source()).unwrap();
    assert!(matches!(
        executor.release_for(&requirement, &source()),
        Err("cleanup-replayed")
    ));
    gate.accept_input_cleanup(receipt).unwrap();
    assert!(
        gate.authorize(
            "sgrant_release_clipboard1",
            "clipboard_to_host",
            now,
            Instant::now()
        )
        .is_ok()
    );
    assert!(
        gate.authorize(
            "sgrant_release_mouse_0001",
            "control_mouse",
            now,
            Instant::now()
        )
        .is_err()
    );
    assert_eq!(gate.deadline(), deadline);
    assert_eq!(gate.highest_generation(), generation);
    assert_eq!(
        trace.borrow().applied.len(),
        2,
        "retry cannot inject another UP"
    );
    assert!(
        matches!(gate.reserve_executor(), Err("not-granted")),
        "cleanup never restores input permission"
    );
}

#[test]
fn empty_or_foreign_executor_cannot_satisfy_a_registered_cleanup_requirement() {
    let (mut gate, _) = setup(&["control_mouse"]);
    let mut executor = managed(&mut gate, Rc::new(RefCell::new(Trace::default())));
    gate.suspend_input();
    let requirement = gate.require_input_cleanup().unwrap();
    let mut empty = Executor::new(
        Fake(Rc::new(RefCell::new(Trace::default()))),
        Arc::new(AtomicBool::new(false)),
        true,
    );
    assert!(matches!(
        empty.release_for(&requirement, &source()),
        Err("executor-binding-mismatch")
    ));
    let (mut other, _) = setup(&["control_mouse"]);
    let mut foreign = managed(&mut other, Rc::new(RefCell::new(Trace::default())));
    assert!(matches!(
        foreign.release_for(&requirement, &source()),
        Err("executor-binding-mismatch")
    ));
    gate.accept_input_cleanup(executor.release_for(&requirement, &source()).unwrap())
        .unwrap();
}

#[test]
fn wrong_gate_consumes_no_matching_cleanup_and_cannot_clear_its_own_executor() {
    let (mut a, _) = setup(&["control_mouse"]);
    let (mut b, _) = setup(&["control_mouse"]);
    let mut executor_a = managed(&mut a, Rc::new(RefCell::new(Trace::default())));
    let mut executor_b = managed(&mut b, Rc::new(RefCell::new(Trace::default())));
    a.suspend_input();
    b.suspend_input();
    let request_a = a.require_input_cleanup().unwrap();
    let request_b = b.require_input_cleanup().unwrap();
    let wrong = executor_a.release_for(&request_a, &source()).unwrap();
    assert_eq!(b.accept_input_cleanup(wrong), Err("cleanup-stale"));
    assert!(matches!(b.reserve_executor(), Err("cleanup-pending")));
    b.accept_input_cleanup(executor_b.release_for(&request_b, &source()).unwrap())
        .unwrap();
    assert!(
        matches!(a.require_input_cleanup(), Err("cleanup-pending")),
        "a wrong-gate attempt cannot complete its original gate either"
    );
}

#[test]
fn unstarted_reservation_is_linear_and_cancellation_invalidates_prior_permits() {
    let (mut gate, now) = setup(&["control_mouse"]);
    let message = palisade_assist_guardian::input::parse(
        &release("sgrant_release_mouse_0001", 1).to_string(),
    )
    .unwrap();
    let t = Instant::now();
    let old = gate.permit(&message, 1, now, t).unwrap();
    let reservation = gate.reserve_executor().unwrap();
    assert!(matches!(gate.reserve_executor(), Err("cleanup-pending")));
    gate.cancel_unstarted_executor(reservation).unwrap();
    assert!(gate.validate_permit(&old, 1, now, t).is_err());
    let reservation = gate.reserve_executor().unwrap();
    let next = palisade_assist_guardian::input::parse(
        &release("sgrant_release_mouse_0001", 2).to_string(),
    )
    .unwrap();
    let _admitted = gate.permit(&next, 1, now, t).unwrap();
    assert_eq!(
        gate.cancel_unstarted_executor(reservation),
        Err("executor-binding-mismatch")
    );
    assert!(matches!(gate.reserve_executor(), Err("cleanup-pending")));
    let (mut resources, _) = setup(&["clipboard_to_host"]);
    assert!(matches!(resources.reserve_executor(), Err("not-granted")));
}

#[test]
fn permit_minted_before_registration_cannot_be_used_by_registered_executor() {
    let (mut gate, now) = setup(&["control_mouse"]);
    let message = palisade_assist_guardian::input::parse(
        &release("sgrant_release_mouse_0001", 1).to_string(),
    )
    .unwrap();
    let t = Instant::now();
    let old = gate.permit(&message, 1, now, t).unwrap();
    let trace = Rc::new(RefCell::new(Trace::default()));
    let mut executor = managed(&mut gate, trace.clone());
    assert_eq!(
        executor.dispatch(old, &mut gate, &source(), &message.action, || Ok((now, t))),
        Err("executor-binding-mismatch")
    );
    assert!(trace.borrow().applied.is_empty());
    let requirement = gate.require_input_cleanup().unwrap();
    gate.accept_input_cleanup(executor.release_for(&requirement, &source()).unwrap())
        .unwrap();
}

#[test]
fn nonterminal_yield_is_not_a_terminal_cleanup_authorization() {
    let (mut gate, now) = setup(&["control_mouse"]);
    let trace = Rc::new(RefCell::new(Trace {
        cooperative: true,
        ..Trace::default()
    }));
    let mut executor = managed(&mut gate, trace);
    dispatch(
        &mut gate,
        &mut executor,
        &fixtures()["scenarios"][0]["steps"][0]["message"],
        now,
    )
    .unwrap();
    let authority = gate
        .input_authority(&source(), now, Instant::now())
        .unwrap();
    let scheduler = LocalYield::new(authority);
    executor
        .yield_owned(&source(), scheduler.local_holds())
        .unwrap();
    assert!(executor.yield_cleanup_complete());
    assert!(matches!(
        gate.require_input_cleanup(),
        Err("input-still-authorized")
    ));
    assert!(matches!(gate.reserve_executor(), Err("cleanup-pending")));
    gate.suspend_input();
    let requirement = gate.require_input_cleanup().unwrap();
    gate.accept_input_cleanup(executor.release_for(&requirement, &source()).unwrap())
        .unwrap();
}

#[test]
fn input_cleanup_receipt_remains_valid_after_resource_revocation_or_session_end() {
    for ended in [false, true] {
        let (mut gate, _) = setup(&["control_mouse", "clipboard_to_host"]);
        let mut executor = managed(&mut gate, Rc::new(RefCell::new(Trace::default())));
        gate.suspend_input();
        let requirement = gate.require_input_cleanup().unwrap();
        if ended {
            gate.stop();
        } else {
            gate.revoke("sgrant_release_clipboard1");
        }
        gate.accept_input_cleanup(executor.release_for(&requirement, &source()).unwrap())
            .unwrap();
        assert_eq!(gate.ended(), ended);
        assert!(gate.reserve_executor().is_err());
    }
}
