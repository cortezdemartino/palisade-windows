//! Real native authorization/executor, published fixtures and fake input only.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signer, SigningKey};
use palisade_assist_guardian::{
    executor::{Backend, Executor, Source},
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
}
struct Fake(Rc<RefCell<Trace>>);
impl Backend for Fake {
    fn prepare(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
    fn validate(&mut self, _: &Source, _: &Action, _: &Ledger) -> Result<(), &'static str> {
        Ok(())
    }
    fn intent(&mut self, _: &Ledger) -> Result<(), &'static str> {
        Ok(())
    }
    fn apply(&mut self, _: &Source, action: &Action) -> Result<(), &'static str> {
        let mut trace = self.0.borrow_mut();
        trace.applied.push(action.clone());
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
