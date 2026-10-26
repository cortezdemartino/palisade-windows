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
                    let response: Response =
                        serde_json::from_slice(&response).map_err(|_| "invalid-service-view")?;
                    let view = response.session;
                    let mut guard = gate.lock().map_err(|_| "endpoint-failed")?;
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
                    for record in view.grants {
                        if record.grant.approved_at != view.approved_at
                            || !["issued", "active", "revoked"].contains(&record.state.as_str())
                        {
                            return Err("binding-mismatch");
                        }
                        if record.state == "revoked" {
                            guard.revoke(&record.grant.grant_id)
                        } else {
                            guard.admit(
                                &SignedStreamGrant {
                                    grant: record.grant,
                                    signature: record.signature,
                                },
                                now()?,
                            )?;
                        }
                    }
                    let lease = view.lease.ok_or("session-ended")?;
                    guard.refresh(&lease, now()?, started, Instant::now())
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
