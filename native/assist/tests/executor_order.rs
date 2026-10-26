use palisade_assist_guardian::{
    executor::*,
    grant::decode,
    input::{Action, Ledger, parse},
    stream::*,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
#[derive(Default)]
struct Trace {
    events: Vec<String>,
    fail_at: Option<usize>,
    applied: usize,
    intent_delay: bool,
    clock_delay: u64,
}
struct Fake(Rc<RefCell<Trace>>);
impl Backend for Fake {
    fn prepare(&mut self) -> Result<(), &'static str> {
        self.0.borrow_mut().events.push("prepare".into());
        Ok(())
    }
    fn validate(&mut self, _: &Source, _: &Action, _: &Ledger) -> Result<(), &'static str> {
        Ok(())
    }
    fn intent(&mut self, ledger: &Ledger) -> Result<(), &'static str> {
        let mut t = self.0.borrow_mut();
        t.events.push(format!("intent:{}", ledger.snapshot().len()));
        if t.intent_delay {
            t.clock_delay = 200;
        }
        Ok(())
    }
    fn apply(&mut self, _: &Source, action: &Action) -> Result<(), &'static str> {
        let mut t = self.0.borrow_mut();
        t.applied += 1;
        t.events.push(format!("{action:?}"));
        if t.fail_at == Some(t.applied) {
            return Err("partial-write");
        };
        Ok(())
    }
    fn fence(&mut self) -> Result<(), &'static str> {
        self.0.borrow_mut().events.push("fence".into());
        Ok(())
    }
    fn cleanup_allowed(&mut self) -> Result<(), &'static str> {
        self.0.borrow_mut().events.push("cleanup".into());
        Ok(())
    }
    fn finish(&mut self) {
        self.0.borrow_mut().events.push("finish".into());
    }
}
fn fixture() -> (SessionGate, SignedStreamGrant, u64, Instant) {
    fixture_with_expiry(false)
}
fn fixture_with_expiry(short: bool) -> (SessionGate, SignedStreamGrant, u64, Instant) {
    let f: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/assist-control-v2.json"
    ))
    .unwrap();
    let e = &f["expect"];
    let now = f["now"].as_u64().unwrap();
    let t = Instant::now();
    let binding = Binding {
        session: e["session"].as_str().unwrap().into(),
        room: e["room"].as_str().unwrap().into(),
        stream: e["stream"].as_str().unwrap().into(),
        publisher_account: e["publisherAccount"].as_str().unwrap().into(),
        publisher_device: e["publisherDevice"].as_str().unwrap().into(),
        helper_account: e["helperAccount"].as_str().unwrap().into(),
        helper_device: e["helperDevice"].as_str().unwrap().into(),
        helper_transport: e["helperTransport"].as_str().unwrap().into(),
        endpoint: "endpoint_fixture_01".into(),
    };
    // Re-sign a keyboard grant with a test-only key. No production pin is accepted.
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use ed25519_dalek::{Signer, SigningKey};
    let key = SigningKey::from_bytes(&[42; 32]);
    let mut grant: SignedStreamGrant =
        serde_json::from_value(f["grants"]["valid"][1]["signed"].clone()).unwrap();
    grant.grant.capability = "control_keyboard".into();
    if short {
        grant.grant.expires_at = now + 25;
    }
    grant.signature = URL_SAFE_NO_PAD.encode(key.sign(&grant.grant.canonical()).to_bytes());
    let mut lease: SignedStreamLease =
        serde_json::from_value(f["leases"]["valid"][0]["signed"].clone()).unwrap();
    lease.signature = URL_SAFE_NO_PAD.encode(
        key.sign(&serde_json::to_vec(&lease.lease).unwrap())
            .to_bytes(),
    );
    let mut gate = SessionGate::new(key.verifying_key().to_bytes(), binding.clone(), now, 0, t);
    gate.admit(&grant, now).unwrap();
    gate.consent(&grant.grant.grant_id, now).unwrap();
    gate.verify_peer(&binding.helper_transport).unwrap();
    gate.refresh(&lease, now, t, t).unwrap();
    let _ = decode::<32>(f["issuerPublicKey"].as_str().unwrap()).unwrap();
    (gate, grant, now, t)
}
fn source() -> Source {
    Source {
        x: 0,
        y: 0,
        width: 800,
        height: 600,
        generation: 1,
    }
}
fn message(
    g: &SignedStreamGrant,
    kind: &str,
    fields: Value,
) -> palisade_assist_guardian::input::Message {
    let mut m = json!({"p":"palisade.assist.control","v":2,"session":g.grant.session,"grant":g.grant.grant_id,"gen":g.grant.generation,"seq":0,"kind":kind});
    for (k, v) in fields.as_object().unwrap() {
        m[k] = v.clone()
    }
    parse(&m.to_string()).unwrap()
}
#[test]
fn intent_is_acknowledged_before_down_and_failure_fences_before_release() {
    let (mut gate, g, now, t) = fixture();
    let m = message(&g, "shortcut", json!({"name":"copy"}));
    let permit = gate.permit(&m, 1, now, t).unwrap();
    let state = Rc::new(RefCell::new(Trace {
        fail_at: Some(2),
        ..Trace::default()
    }));
    let mut executor = Executor::new(Fake(state.clone()), Arc::new(AtomicBool::new(false)), true);
    assert!(
        executor
            .dispatch(permit, &mut gate, &source(), &m.action, || Ok((now, t)))
            .is_err()
    );
    assert!(gate.ended());
    assert!(!executor.held());
    let trace = &state.borrow().events;
    let down = trace.iter().position(|s| s.contains("down: true")).unwrap();
    assert!(trace[..down].iter().any(|s| s == "intent:1"));
    let fence = trace.iter().position(|s| s == "fence").unwrap();
    let up = trace
        .iter()
        .position(|s| s.contains("down: false"))
        .unwrap();
    assert!(fence < up);
    assert_eq!(trace.last().unwrap(), "finish");
}
#[test]
fn idle_keyboard_expiry_fences_and_releases_held_keys_without_another_packet() {
    let (mut gate, g, now, t) = fixture_with_expiry(true);
    let m = message(&g, "key", json!({"code":"ControlLeft","down":true}));
    let permit = gate.permit(&m, 1, now, t).unwrap();
    let trace = Rc::new(RefCell::new(Trace::default()));
    let mut executor = Executor::new(Fake(trace.clone()), Arc::new(AtomicBool::new(false)), true);
    executor
        .dispatch(permit, &mut gate, &source(), &m.action, || Ok((now, t)))
        .unwrap();
    assert!(executor.held());
    assert!(
        executor
            .heartbeat(
                &mut gate,
                &source(),
                now + 25,
                t + Duration::from_millis(25)
            )
            .is_err()
    );
    assert!(!executor.held());
    let events = &trace.borrow().events;
    let fence = events.iter().position(|e| e == "fence").unwrap();
    let up = events
        .iter()
        .position(|e| e.contains("down: false"))
        .unwrap();
    assert!(fence < up);
}
#[test]
fn dropping_an_active_executor_fences_before_releasing_its_mirrored_input() {
    let (mut gate, g, now, t) = fixture();
    let m = message(&g, "key", json!({"code":"ControlLeft","down":true}));
    let permit = gate.permit(&m, 1, now, t).unwrap();
    let trace = Rc::new(RefCell::new(Trace::default()));
    let mut executor = Executor::new(Fake(trace.clone()), Arc::new(AtomicBool::new(false)), true);
    executor
        .dispatch(permit, &mut gate, &source(), &m.action, || Ok((now, t)))
        .unwrap();
    drop(executor);
    let events = &trace.borrow().events;
    let fence = events.iter().position(|e| e == "fence").unwrap();
    let up = events
        .iter()
        .position(|e| e.contains("down: false"))
        .unwrap();
    assert!(fence < up);
}
#[test]
fn blocking_intent_cannot_outlive_the_one_shot_permit() {
    let (mut gate, g, now, t) = fixture();
    let m = message(&g, "key", json!({"code":"ControlLeft","down":true}));
    let permit = gate.permit(&m, 1, now, t).unwrap();
    let state = Rc::new(RefCell::new(Trace {
        intent_delay: true,
        ..Trace::default()
    }));
    let mut executor = Executor::new(Fake(state.clone()), Arc::new(AtomicBool::new(false)), true);
    assert!(
        executor
            .dispatch(permit, &mut gate, &source(), &m.action, || {
                let delta = state.borrow().clock_delay;
                Ok((now + delta, t + Duration::from_millis(delta)))
            })
            .is_err()
    );
    assert!(
        !state
            .borrow()
            .events
            .iter()
            .any(|s| s.contains("down: true"))
    );
    assert!(gate.ended());
}
#[test]
fn an_unowned_up_cannot_release_host_input() {
    let (mut gate, g, now, t) = fixture();
    let m = message(&g, "key", json!({"code":"ControlLeft","down":false}));
    let permit = gate.permit(&m, 1, now, t).unwrap();
    let state = Rc::new(RefCell::new(Trace::default()));
    let mut executor = Executor::new(Fake(state.clone()), Arc::new(AtomicBool::new(false)), true);
    executor
        .dispatch(permit, &mut gate, &source(), &m.action, || Ok((now, t)))
        .unwrap();
    assert_eq!(state.borrow().applied, 0);
}
#[test]
fn default_disabled_executor_never_calls_the_sink() {
    let (mut gate, g, now, t) = fixture();
    let m = message(&g, "key", json!({"code":"KeyA","down":true}));
    let permit = gate.permit(&m, 1, now, t).unwrap();
    let state = Rc::new(RefCell::new(Trace::default()));
    let mut executor = Executor::new(Fake(state.clone()), Arc::new(AtomicBool::new(false)), false);
    assert!(
        executor
            .dispatch(permit, &mut gate, &source(), &m.action, || Ok((now, t)))
            .is_err()
    );
    assert_eq!(state.borrow().applied, 0);
}
#[test]
fn permits_are_bound_to_the_original_gate_and_action() {
    let (mut first, g, now, t) = fixture();
    let m = message(&g, "key", json!({"code":"KeyA","down":true}));
    let permit = first.permit(&m, 1, now, t).unwrap();
    let (mut second, _, _, _) = fixture();
    assert!(second.validate_permit(&permit, 1, now, t).is_err());
    assert!(!permit.matches(&Action::Key {
        code: "KeyB".into(),
        down: true
    }));
}
