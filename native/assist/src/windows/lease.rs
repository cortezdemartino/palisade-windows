//! Independently authenticated polling. No renderer cookies, tokens or supplied URLs.
use crate::{
    stream::{SessionGate, SignedStreamGrant, SignedStreamLease},
    windows::{https, identity::Identity},
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
    worker: Option<thread::JoinHandle<()>>,
    gate: Arc<Mutex<SessionGate>>,
    on_loss: Arc<dyn Fn() + Send + Sync>,
}
fn apply_response(
    guard: &mut SessionGate,
    bytes: &[u8],
    now: u64,
    started: Instant,
    instant: Instant,
) -> Result<(), &'static str> {
    let response: Response = serde_json::from_slice(bytes).map_err(|_| "invalid-service-view")?;
    guard.tick(now, instant)?;
    let binding = guard.binding().clone();
    let view = response.session;
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
    guard.refresh(&view.lease.ok_or("session-ended")?, now, started, instant)
}
impl LeasePoller {
    pub fn start(
        identity: Arc<Identity>,
        gate: Arc<Mutex<SessionGate>>,
        on_loss: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let retained_gate = gate.clone();
        let retained_loss = on_loss.clone();
        let worker = thread::spawn(move || {
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
                    apply_response(&mut guard, &response, now()?, started, Instant::now())
                })();
                if result.is_err() {
                    if let Ok(mut guard) = gate.lock() {
                        guard.stop();
                    }
                    on_loss();
                    return;
                }
                // Five seconds from request start, not five seconds after a slow response.
                while started.elapsed() < Duration::from_secs(5) {
                    if flag.load(Ordering::SeqCst) {
                        return;
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
                        on_loss();
                        return;
                    }
                }
            }
        });
        Self {
            cancel,
            worker: Some(worker),
            gate: retained_gate,
            on_loss: retained_loss,
        }
    }
}
impl Drop for LeasePoller {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.gate.lock() {
            guard.stop();
        }
        (self.on_loss)();
        if let Some(worker) = self.worker.take() {
            if worker.thread().id() != thread::current().id() {
                let _ = worker.join();
            }
        }
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
            "grants":[{"grant":g,"signature":input.signature,"state":"active"},
                {"grant":resource.grant,"signature":resource.signature,"state":"active"}],"lease":{"lease":lease.lease,"signature":lease.signature}}});
        let next = instant + Duration::from_secs(5);
        apply_response(
            &mut gate,
            &serde_json::to_vec(&response).unwrap(),
            now + 5000,
            next,
            next,
        )
        .unwrap();
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
