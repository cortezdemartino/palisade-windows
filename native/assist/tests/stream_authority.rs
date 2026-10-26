use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signer, SigningKey};
use palisade_assist_guardian::{grant::decode, stream::*};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
fn fixtures() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/assist-control-v2.json"
    ))
    .unwrap()
}
fn binding(f: &Value) -> Binding {
    let e = &f["expect"];
    Binding {
        session: e["session"].as_str().unwrap().into(),
        room: e["room"].as_str().unwrap().into(),
        stream: e["stream"].as_str().unwrap().into(),
        publisher_account: e["publisherAccount"].as_str().unwrap().into(),
        publisher_device: e["publisherDevice"].as_str().unwrap().into(),
        helper_account: e["helperAccount"].as_str().unwrap().into(),
        helper_device: e["helperDevice"].as_str().unwrap().into(),
        helper_transport: e["helperTransport"].as_str().unwrap().into(),
        endpoint: "endpoint_fixture_01".into(),
    }
}
fn sign_grant(grant: StreamGrant, key: &SigningKey) -> SignedStreamGrant {
    let signature = URL_SAFE_NO_PAD.encode(key.sign(&grant.canonical()).to_bytes());
    SignedStreamGrant { grant, signature }
}
fn sign_lease(lease: StreamLease, key: &SigningKey) -> SignedStreamLease {
    let signature =
        URL_SAFE_NO_PAD.encode(key.sign(&serde_json::to_vec(&lease).unwrap()).to_bytes());
    SignedStreamLease { lease, signature }
}
#[test]
fn unchanged_companion_positive_and_negative_grants_and_leases() {
    let f = fixtures();
    let key = decode::<32>(f["issuerPublicKey"].as_str().unwrap()).unwrap();
    let b = binding(&f);
    let now = f["now"].as_u64().unwrap();
    let deadline = f["expect"]["sessionDeadline"].as_u64().unwrap();
    for case in f["grants"]["valid"].as_array().unwrap() {
        let mut expected = b.clone();
        if let Some(room) = case["expect"]["room"].as_str() {
            expected.room = room.into();
        }
        let signed: SignedStreamGrant = serde_json::from_value(case["signed"].clone()).unwrap();
        let d = case["expect"]["sessionDeadline"]
            .as_u64()
            .unwrap_or(deadline);
        verify_grant(&signed, &key, &expected, now, Some(d), 1).unwrap();
    }
    for case in f["grants"]["invalid"].as_array().unwrap() {
        if let Ok(signed) = serde_json::from_value::<SignedStreamGrant>(case["signed"].clone()) {
            let d = case["expect"]["sessionDeadline"]
                .as_u64()
                .unwrap_or(deadline);
            let n = case["expect"]["now"].as_u64().unwrap_or(now);
            let generation = case["expect"]["minimumGeneration"].as_u64().unwrap_or(1);
            assert!(
                verify_grant(&signed, &key, &b, n, Some(d), generation).is_err(),
                "accepted {}",
                case["name"]
            );
        }
    }
    for case in f["leases"]["valid"].as_array().unwrap() {
        let signed: SignedStreamLease = serde_json::from_value(case["signed"].clone()).unwrap();
        verify_lease(&signed, &key, &b, deadline, now).unwrap();
    }
    for case in f["leases"]["invalid"].as_array().unwrap() {
        if let Ok(signed) = serde_json::from_value::<SignedStreamLease>(case["signed"].clone()) {
            assert!(
                verify_lease(&signed, &key, &b, deadline, now).is_err(),
                "accepted {}",
                case["name"]
            );
        }
    }
}
#[test]
fn authority_needs_local_consent_and_fresh_covering_lease() {
    let f = fixtures();
    let key = decode::<32>(f["issuerPublicKey"].as_str().unwrap()).unwrap();
    let now = f["now"].as_u64().unwrap();
    let t = Instant::now();
    let grant: SignedStreamGrant =
        serde_json::from_value(f["grants"]["valid"][1]["signed"].clone()).unwrap();
    let lease: SignedStreamLease =
        serde_json::from_value(f["leases"]["valid"][0]["signed"].clone()).unwrap();
    let id = &grant.grant.grant_id;
    let mut gate = SessionGate::new(key, binding(&f), now, 0, t);
    gate.admit(&grant, now).unwrap();
    gate.verify_peer(&binding(&f).helper_transport).unwrap();
    gate.refresh(&lease, now, t, t).unwrap();
    assert!(gate.authorize(id, "control_mouse", now, t).is_err());
    gate.consent(id, now).unwrap();
    gate.authorize(id, "control_mouse", now, t).unwrap();
    assert!(gate.authorize(id, "control_keyboard", now, t).is_err());
    let message=palisade_assist_guardian::input::parse(&json!({"p":"palisade.assist.control","v":2,"session":grant.grant.session,"grant":id,"gen":3,"seq":1,"kind":"pointer-move","x":0.5,"y":0.5,"sourceGen":1}).to_string()).unwrap();
    let permit = gate.permit(&message, 1, now, t).unwrap();
    gate.validate_permit(&permit, 1, now, t).unwrap();
    assert!(gate.validate_permit(&permit, 2, now, t).is_err());
    assert!(gate.permit(&message, 1, now, t).is_err());
    gate.revoke(id);
    assert!(gate.validate_permit(&permit, 1, now, t).is_err());
    assert!(gate.consent(id, now).is_err());
}
#[test]
fn clock_skew_cannot_extend_a_lease_past_twenty_monotonic_seconds() {
    let f = fixtures();
    let key = SigningKey::from_bytes(&[42; 32]);
    let now = f["now"].as_u64().unwrap();
    let t = Instant::now();
    let grant: SignedStreamGrant =
        serde_json::from_value(f["grants"]["valid"][1]["signed"].clone()).unwrap();
    let grant = sign_grant(grant.grant, &key);
    let mut lease: SignedStreamLease =
        serde_json::from_value(f["leases"]["valid"][0]["signed"].clone()).unwrap();
    lease.lease.issued_at = now + 30_000;
    lease.lease.expires_at = now + 50_000;
    let lease = sign_lease(lease.lease, &key);
    let mut gate = SessionGate::new(key.verifying_key().to_bytes(), binding(&f), now, 0, t);
    gate.admit(&grant, now).unwrap();
    gate.verify_peer(&binding(&f).helper_transport).unwrap();
    gate.consent(&grant.grant.grant_id, now).unwrap();
    gate.refresh(&lease, now, t, t).unwrap();
    gate.authorize(
        &grant.grant.grant_id,
        "control_mouse",
        now + 19_999,
        t + Duration::from_millis(19_999),
    )
    .unwrap();
    assert!(
        gate.authorize(
            &grant.grant.grant_id,
            "control_mouse",
            now + 20_000,
            t + Duration::from_secs(20)
        )
        .is_err()
    );
    assert!(gate.ended());
}
#[test]
fn deadlines_and_controller_generations_are_immutable_without_lease_generation_confusion() {
    let f = fixtures();
    let key = SigningKey::from_bytes(&[42; 32]);
    let now = f["now"].as_u64().unwrap();
    let t = Instant::now();
    let g: SignedStreamGrant =
        serde_json::from_value(f["grants"]["valid"][1]["signed"].clone()).unwrap();
    let g = sign_grant(g.grant, &key);
    let l: SignedStreamLease =
        serde_json::from_value(f["leases"]["valid"][0]["signed"].clone()).unwrap();
    let mut l = l.lease;
    l.generation = 99;
    let mut gate = SessionGate::new(key.verifying_key().to_bytes(), binding(&f), now, 0, t);
    gate.admit(&g, now).unwrap();
    gate.verify_peer(&binding(&f).helper_transport).unwrap();
    gate.consent(&g.grant.grant_id, now).unwrap();
    gate.refresh(&sign_lease(l.clone(), &key), now, t, t)
        .unwrap();
    gate.authorize(&g.grant.grant_id, "control_mouse", now, t)
        .unwrap();
    assert_eq!(gate.highest_generation(), 3);
    let mut changed = g.grant.clone();
    changed.session_deadline -= 1;
    changed.expires_at -= 1;
    assert!(gate.admit(&sign_grant(changed, &key), now).is_err());
    l.issued_at += 1;
    l.grants.clear();
    gate.refresh(&sign_lease(l, &key), now, t, t).unwrap();
    assert!(
        gate.authorize(&g.grant.grant_id, "control_mouse", now, t)
            .is_err()
    );
    assert!(gate.admit(&g, now).is_err());
}
#[test]
fn failed_poll_rollback_stop_and_expiration_cannot_reactivate() {
    let f = fixtures();
    let key = decode::<32>(f["issuerPublicKey"].as_str().unwrap()).unwrap();
    let now = f["now"].as_u64().unwrap();
    let t = Instant::now();
    let g: SignedStreamGrant =
        serde_json::from_value(f["grants"]["valid"][1]["signed"].clone()).unwrap();
    let mut gate = SessionGate::new(key, binding(&f), now, 0, t);
    gate.admit(&g, now).unwrap();
    gate.verify_peer(&binding(&f).helper_transport).unwrap();
    assert!(gate.consent(&g.grant.grant_id, now - 1).is_err());
    assert!(gate.admit(&g, now).is_err());
}
#[test]
fn idle_grant_expiration_invalidates_an_outstanding_permit_under_a_fresh_lease() {
    let f = fixtures();
    let key = SigningKey::from_bytes(&[42; 32]);
    let now = f["now"].as_u64().unwrap();
    let t = Instant::now();
    let mut g: SignedStreamGrant =
        serde_json::from_value(f["grants"]["valid"][1]["signed"].clone()).unwrap();
    g.grant.capability = "control_keyboard".into();
    g.grant.expires_at = now + 25;
    let g = sign_grant(g.grant, &key);
    let mut gate = SessionGate::new(key.verifying_key().to_bytes(), binding(&f), now, 0, t);
    gate.admit(&g, now).unwrap();
    gate.verify_peer(&binding(&f).helper_transport).unwrap();
    gate.consent(&g.grant.grant_id, now).unwrap();
    let l: SignedStreamLease =
        serde_json::from_value(f["leases"]["valid"][0]["signed"].clone()).unwrap();
    gate.refresh(&sign_lease(l.lease, &key), now, t, t).unwrap();
    let packet=palisade_assist_guardian::input::parse(&json!({"p":"palisade.assist.control","v":2,"session":g.grant.session,"grant":g.grant.grant_id,"gen":3,"seq":0,"kind":"key","code":"ControlLeft","down":true}).to_string()).unwrap();
    let permit = gate.permit(&packet, 1, now, t).unwrap();
    gate.tick(now + 25, t + Duration::from_millis(25)).unwrap();
    assert!(
        gate.validate_permit(&permit, 1, now + 25, t + Duration::from_millis(25))
            .is_err()
    );
    assert!(gate.consent(&g.grant.grant_id, now + 25).is_err());
}
