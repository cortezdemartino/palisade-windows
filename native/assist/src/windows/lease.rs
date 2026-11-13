//! Independently authenticated polling. No renderer cookies, tokens or supplied URLs.
use crate::{
    stream::{SessionGate, SignedStreamGrant, SignedStreamLease},
    windows::{https, identity::EndpointIdentity},
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Party {
    user_id: String,
    device_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    grant: crate::stream::StreamGrant,
    signature: String,
    state: String,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InputRequest {
    pub id: String,
    pub capabilities: Vec<String>,
    pub created_at: u64,
}
/// Descriptive service facts only. They never substitute for signed authority,
/// native source ownership, consent or an observed peer certificate.
#[derive(Clone)]
pub struct ServiceFacts {
    pub stream_track_name: Option<String>,
    pub approved: Vec<String>,
    pub input_request: Option<InputRequest>,
    pub lease_issued_at: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct View {
    id: String,
    room: String,
    stream: String,
    publisher: Party,
    helper: Party,
    approved_at: u64,
    deadline: u64,
    ended: Option<Value>,
    #[serde(default)]
    stream_track_name: Option<String>,
    #[serde(default)]
    approved: Vec<String>,
    #[serde(default)]
    input_request: Option<InputRequest>,
    grants: Vec<Record>,
    lease: Option<SignedStreamLease>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    session: View,
}
pub fn now() -> Result<u64, &'static str> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "invalid-clock")?
        .as_millis()
        .try_into()
        .map_err(|_| "invalid-clock")
}
pub struct LeasePoller {
    cancel: Arc<AtomicBool>,
    worker: crate::cleanup::RetainedWorker,
    gate: Arc<Mutex<SessionGate>>,
    on_loss: Arc<dyn Fn() + Send + Sync>,
    cleanup_failed: bool,
    facts: Arc<Mutex<Option<ServiceFacts>>>,
}
struct FactsLifetime(Arc<Mutex<Option<ServiceFacts>>>);
impl Drop for FactsLifetime {
    fn drop(&mut self) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
}
impl View {
    fn validate_metadata(&self) -> Result<(), &'static str> {
        let unique: std::collections::BTreeSet<_> = self.approved.iter().collect();
        if self.approved.len() > crate::grant::CAPABILITIES.len()
            || unique.len() != self.approved.len()
            || self
                .approved
                .iter()
                .any(|cap| !crate::grant::CAPABILITIES.contains(&cap.as_str()))
        {
            return Err("invalid-service-view");
        }
        if let Some(request) = &self.input_request {
            let unique: std::collections::BTreeSet<_> = request.capabilities.iter().collect();
            if !crate::grant::token(&request.id, 8, 128)
                || request.capabilities.is_empty()
                || request.capabilities.len() > 2
                || unique.len() != request.capabilities.len()
                || request.created_at < self.approved_at
                || request.created_at >= self.deadline
                || request.capabilities.iter().any(|cap| {
                    !["control_mouse", "control_keyboard"].contains(&cap.as_str())
                        || !self.approved.contains(cap)
                })
            {
                return Err("invalid-service-view");
            }
        }
        Ok(())
    }
}
fn apply_response(
    guard: &mut SessionGate,
    bytes: &[u8],
    now: u64,
    started: Instant,
    instant: Instant,
) -> Result<ServiceFacts, &'static str> {
    let response: Response = serde_json::from_slice(bytes).map_err(|_| "invalid-service-view")?;
    guard.tick(now, instant)?;
    let binding = guard.binding().clone();
    let view = response.session;
    view.validate_metadata()?;
    if view.id != binding.session
        || view.room != binding.room
        || view.stream != binding.stream
        || view.publisher.user_id != binding.publisher_account
        || view.publisher.device_id != binding.publisher_device
        || view.helper.user_id != binding.helper_account
        || view.helper.device_id != binding.helper_device
        || view.ended.is_some()
        || view.grants.len() > 8
        || guard.deadline() != Some(view.deadline)
    {
        return Err("session-ended");
    }
    let mut ids = std::collections::BTreeSet::new();
    for record in view.grants {
        if !ids.insert(record.grant.grant_id.clone())
            || record.grant.approved_at != view.approved_at
            || !["issued", "active", "revoked"].contains(&record.state.as_str())
            || (!view.approved.is_empty() && !view.approved.contains(&record.grant.capability))
        {
            return Err("binding-mismatch");
        }
        let id = record.grant.grant_id.clone();
        // Even revoked service records must match an authenticated known grant.
        guard.observe_grant(
            &SignedStreamGrant {
                grant: record.grant,
                signature: record.signature,
            },
            now,
        )?;
        if record.state == "revoked" {
            guard.revoke(&id);
        }
    }
    let lease = view.lease.ok_or("session-ended")?;
    guard.refresh(&lease, now, started, instant)?;
    Ok(ServiceFacts {
        stream_track_name: view.stream_track_name,
        approved: view.approved,
        input_request: view.input_request,
        lease_issued_at: lease.lease.issued_at,
    })
}
impl LeasePoller {
    pub fn start(
        identity: Arc<EndpointIdentity>,
        gate: Arc<Mutex<SessionGate>>,
        on_loss: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let retained_gate = gate.clone();
        let retained_loss = on_loss.clone();
        let facts = Arc::new(Mutex::new(None));
        let worker_facts = facts.clone();
        let worker = thread::spawn(move || {
            let _facts_lifetime = FactsLifetime(worker_facts.clone());
            let mut previous = 0;
            while !flag.load(Ordering::SeqCst) {
                let started = Instant::now();
                let result = (|| {
                    let binding = gate
                        .lock()
                        .map_err(|_| "endpoint-failed")?
                        .binding()
                        .clone();
                    let issued_at = now()?;
                    if issued_at <= previous {
                        return Err("clock-not-increasing");
                    };
                    previous = issued_at;
                    let signature = identity.sign_lease_request(
                        &binding.endpoint,
                        &binding.session,
                        issued_at,
                    )?;
                    let body=serde_json::to_vec(&serde_json::json!({"endpointId":binding.endpoint,"session":binding.session,"issuedAt":issued_at,"signature":signature})).map_err(|_|"invalid-message")?;
                    let response =
                        https::request("/api/v1/assist/streams/endpoint-lease", Some(&body))?;
                    if flag.load(Ordering::SeqCst) {
                        return Err("revoked");
                    }
                    let mut guard = gate.lock().map_err(|_| "endpoint-failed")?;
                    let observed =
                        apply_response(&mut guard, &response, now()?, started, Instant::now())?;
                    if flag.load(Ordering::SeqCst) {
                        return Err("revoked");
                    }
                    *worker_facts.lock().map_err(|_| "endpoint-failed")? = Some(observed);
                    Ok(())
                })();
                if result.is_err() {
                    if let Ok(mut facts) = worker_facts.lock() {
                        *facts = None;
                    }
                    if let Ok(mut guard) = gate.lock() {
                        guard.stop();
                    }
                    on_loss();
                    return Ok(());
                }
                // Five seconds from request start, not five seconds after a slow response.
                while started.elapsed() < Duration::from_secs(5) {
                    if flag.load(Ordering::SeqCst) {
                        return Ok(());
                    }
                    thread::sleep(Duration::from_millis(25));
                    let invalid = gate
                        .lock()
                        .map(|mut guard| {
                            now()
                                .and_then(|time| guard.tick(time, Instant::now()))
                                .is_err()
                        })
                        .unwrap_or(true);
                    if invalid {
                        if let Ok(mut facts) = worker_facts.lock() {
                            *facts = None;
                        }
                        on_loss();
                        return Ok(());
                    }
                }
            }
            Ok(())
        });
        Self {
            cancel,
            worker: crate::cleanup::RetainedWorker::new(worker),
            gate: retained_gate,
            on_loss: retained_loss,
            cleanup_failed: false,
            facts,
        }
    }
    pub(crate) fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }
    pub(crate) fn facts_reader(&self) -> Arc<Mutex<Option<ServiceFacts>>> {
        self.facts.clone()
    }
    pub fn service_facts(&self) -> Option<ServiceFacts> {
        if self.cancel.load(Ordering::SeqCst) {
            return None;
        }
        let facts = self.facts.lock().ok()?.clone();
        if self.cancel.load(Ordering::SeqCst) {
            return None;
        }
        facts
    }
    pub fn finish(&mut self) -> Result<(), &'static str> {
        self.cancel.store(true, Ordering::SeqCst);
        if let Ok(mut facts) = self.facts.lock() {
            *facts = None;
        }
        let mut guard = self.gate.lock().unwrap_or_else(|error| {
            self.cleanup_failed = true;
            error.into_inner()
        });
        guard.stop();
        drop(guard);
        if crate::cleanup::attempt(|| {
            (self.on_loss)();
            Ok(())
        })
        .is_err()
        {
            self.cleanup_failed = true;
        }
        if self.worker.join().is_err() {
            self.cleanup_failed = true;
        }
        if self.cleanup_failed {
            Err("lease-cleanup-failed")
        } else {
            Ok(())
        }
    }
}
impl Drop for LeasePoller {
    fn drop(&mut self) {
        let _ = crate::cleanup::attempt(|| self.finish());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{Binding, StreamGrant};
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use ed25519_dalek::{Signer, SigningKey};
    fn signed(grant: StreamGrant, key: &SigningKey) -> SignedStreamGrant {
        SignedStreamGrant {
            signature: URL_SAFE_NO_PAD.encode(key.sign(&grant.canonical()).to_bytes()),
            grant,
        }
    }
    fn metadata_view() -> Value {
        serde_json::json!({
            "id":"sassist_fixture_0001", "room":"!voice:palisade.test", "stream":"TR_fixtureStream0001",
            "publisher":{"userId":"@publisher:palisade.test","deviceId":"PUBLISHER"},
            "helper":{"userId":"@helper:palisade.test","deviceId":"HELPER"},
            "approvedAt":1800000000000u64,"deadline":1800007200000u64,"ended":null,
            "streamTrackName":"palisade-capture:fixture_operation_01",
            "approved":["control_mouse","clipboard_to_host"],
            "inputRequest":{"id":"sinput_fixture_01","createdAt":1800000300000u64,"capabilities":["control_mouse"]},
            "grants":[],"lease":null
        })
    }
    #[test]
    fn canonical_service_metadata_and_legacy_absence_parse_without_creating_authority() {
        let mut wire = metadata_view();
        let view: View = serde_json::from_value(wire.clone()).unwrap();
        view.validate_metadata().unwrap();
        assert_eq!(view.input_request.unwrap().capabilities, ["control_mouse"]);
        // This metadata has no grant or lease and is never fed into consent.
        assert!(view.grants.is_empty());
        assert!(view.lease.is_none());
        for name in ["streamTrackName", "approved", "inputRequest"] {
            wire.as_object_mut().unwrap().remove(name);
        }
        let legacy: View = serde_json::from_value(wire).unwrap();
        legacy.validate_metadata().unwrap();
        assert!(legacy.stream_track_name.is_none());
        assert!(legacy.approved.is_empty());
        assert!(legacy.input_request.is_none());
    }
    #[test]
    fn malformed_service_metadata_never_becomes_a_request_or_approval() {
        let base = metadata_view();
        let cases = [
            (
                "/approved",
                serde_json::json!(["control_mouse", "control_mouse"]),
            ),
            ("/approved", serde_json::json!(["arbitrary_filesystem"])),
            ("/approved", serde_json::json!(true)),
            ("/approved", Value::Null),
            ("/streamTrackName", serde_json::json!(true)),
            ("/inputRequest/id", serde_json::json!("bad")),
            ("/inputRequest/createdAt", serde_json::json!(-1)),
            (
                "/inputRequest/createdAt",
                serde_json::json!(1800007200000u64),
            ),
            (
                "/inputRequest/createdAt",
                serde_json::json!(1799999999999u64),
            ),
            ("/inputRequest/capabilities", serde_json::json!([])),
            (
                "/inputRequest/capabilities",
                serde_json::json!(["control_keyboard"]),
            ),
            (
                "/inputRequest/capabilities",
                serde_json::json!(["control_mouse", "control_mouse"]),
            ),
            (
                "/inputRequest/capabilities",
                serde_json::json!(["clipboard_to_host"]),
            ),
            (
                "/inputRequest",
                serde_json::json!({"id":"sinput_fixture_01","createdAt":1800000300000u64,
                "capabilities":["control_mouse"],"approved":true}),
            ),
        ];
        for (path, invalid) in cases {
            let mut wire = base.clone();
            *wire.pointer_mut(path).unwrap() = invalid;
            let refused = serde_json::from_value::<View>(wire)
                .map(|view| view.validate_metadata().is_err())
                .unwrap_or(true);
            assert!(refused, "{path}");
        }
        let mut unknown = base;
        unknown["nativeConsent"] = serde_json::json!(true);
        assert!(serde_json::from_value::<View>(unknown).is_err());
    }
    #[test]
    fn poller_exit_clears_descriptive_facts_even_during_unwind() {
        let facts = Arc::new(Mutex::new(Some(ServiceFacts {
            stream_track_name: Some("palisade-capture:fixture_operation_01".into()),
            approved: vec!["control_mouse".into()],
            input_request: None,
            lease_issued_at: 1,
        })));
        assert!(
            crate::cleanup::attempt(|| {
                let _owner = FactsLifetime(facts.clone());
                panic!("intentional poller fixture");
            })
            .is_err()
        );
        assert!(facts.lock().unwrap().is_none());
    }
    #[test]
    fn full_poll_preserves_resources_after_local_input_suspension_without_restoring_input() {
        let fixtures: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/assist-control-v2.json"
        ))
        .unwrap();
        let now = fixtures["now"].as_u64().unwrap();
        let instant = Instant::now();
        let key = SigningKey::from_bytes(&[42; 32]);
        let input: SignedStreamGrant =
            serde_json::from_value(fixtures["grants"]["valid"][1]["signed"].clone()).unwrap();
        let input = signed(input.grant, &key);
        let mut resource = input.grant.clone();
        resource.capability = "clipboard_to_host".into();
        resource.endpoint = None;
        resource.generation += 1;
        resource.grant_id = "clipboard_fixture_01".into();
        resource.nonce = "clipboard_fixture_nonce_01".into();
        let resource = signed(resource, &key);
        let g = &input.grant;
        let binding = Binding {
            session: g.session.clone(),
            room: g.room.clone(),
            stream: g.stream.clone(),
            publisher_account: g.publisher_account.clone(),
            publisher_device: g.publisher_device.clone(),
            helper_account: g.helper_account.clone(),
            helper_device: g.helper_device.clone(),
            helper_transport: g.helper_transport.clone(),
            endpoint: g.endpoint.clone().unwrap(),
        };
        let mut gate = SessionGate::new(key.verifying_key().to_bytes(), binding, now, 0, instant);
        for grant in [&input, &resource] {
            gate.admit(grant, now).unwrap();
            gate.consent(&grant.grant.grant_id, now).unwrap();
        }
        gate.verify_peer(&g.helper_transport).unwrap();
        let mut lease: SignedStreamLease =
            serde_json::from_value(fixtures["leases"]["valid"][0]["signed"].clone()).unwrap();
        lease.lease.grants = vec![g.grant_id.clone(), resource.grant.grant_id.clone()];
        lease.lease.grants.sort();
        lease.lease.generation = resource.grant.generation;
        lease.signature = URL_SAFE_NO_PAD.encode(
            key.sign(&serde_json::to_vec(&lease.lease).unwrap())
                .to_bytes(),
        );
        gate.refresh(&lease, now, instant, instant).unwrap();
        gate.suspend_input();
        lease.lease.issued_at = now + 5000;
        lease.lease.expires_at = now + 25000;
        lease.signature = URL_SAFE_NO_PAD.encode(
            key.sign(&serde_json::to_vec(&lease.lease).unwrap())
                .to_bytes(),
        );
        let mut response = serde_json::json!({"session": {"id":g.session,"room":g.room,"stream":g.stream,
            "publisher":{"userId":g.publisher_account,"deviceId":g.publisher_device},
            "helper":{"userId":g.helper_account,"deviceId":g.helper_device},
            "approvedAt":g.approved_at,"deadline":g.session_deadline,"ended":null,
            "streamTrackName":"palisade-capture:fixture_operation_01",
            "approved":["control_mouse","clipboard_to_host"],"inputRequest":null,
            "grants":[{"grant":g,"signature":input.signature,"state":"active"},
                {"grant":resource.grant,"signature":resource.signature,"state":"active"}],"lease":{"lease":lease.lease,"signature":lease.signature}}});
        let next = instant + Duration::from_secs(5);
        let facts = apply_response(
            &mut gate,
            &serde_json::to_vec(&response).unwrap(),
            now + 5000,
            next,
            next,
        )
        .unwrap();
        assert_eq!(
            facts.stream_track_name.as_deref(),
            Some("palisade-capture:fixture_operation_01")
        );
        assert_eq!(facts.lease_issued_at, now + 5000);
        assert!(!gate.ended());
        gate.authorize(
            &resource.grant.grant_id,
            "clipboard_to_host",
            now + 5000,
            next,
        )
        .unwrap();
        assert!(
            gate.authorize(&g.grant_id, "control_mouse", now + 5000, next)
                .is_err()
        );
        assert!(gate.consent(&g.grant_id, now + 5000).is_err());
        response["session"]["grants"][0]["grant"]["helperDevice"] = serde_json::json!("OTHER");
        assert!(
            apply_response(
                &mut gate,
                &serde_json::to_vec(&response).unwrap(),
                now + 5000,
                next,
                next
            )
            .is_err()
        );
    }
}
