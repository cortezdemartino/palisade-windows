//! Dormant stream-session coordinator. Only trusted native IPC/gateway code may
//! call it; no renderer assertion can set consent or replace its issuer.
use super::{
    consent::{Decision, Prompt},
    identity::Identity,
    input::{Source, ordinary_desktop},
    issuer::TrustedIssuer,
    lease::{self, LeasePoller},
    lifecycle::NativeStop,
    process_input::ProcessInput,
};
use crate::{
    executor::Executor,
    input,
    stream::{Binding, SessionGate, SignedStreamGrant},
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Acknowledgement {
    pub grant_id: String,
    pub signature: Option<String>,
    pub state: &'static str,
}
enum Work {
    Packet(String, mpsc::SyncSender<Result<(), &'static str>>),
}
pub struct StreamRuntime {
    gate: Arc<Mutex<SessionGate>>,
    source: Option<Arc<Mutex<Source>>>,
    stop: Arc<AtomicBool>,
    identity: Arc<Identity>,
    prompts: BTreeMap<String, Prompt>,
    poller: Option<LeasePoller>,
    work: mpsc::SyncSender<Work>,
    worker: Option<thread::JoinHandle<()>>,
    ready: Arc<AtomicBool>,
}
impl StreamRuntime {
    pub fn begin(
        issuer: &TrustedIssuer,
        identity: Arc<Identity>,
        binding: Binding,
        grants: &[SignedStreamGrant],
        source: Option<Source>,
        stop_surface: &NativeStop,
        manifest: super::image::ImageManifest,
    ) -> Result<Self, &'static str> {
        if !cfg!(any(feature = "native-input", feature = "native-resources")) {
            return Err("endpoint-disabled");
        }
        let stop = stop_surface.session_flag()?;
        if stop.load(Ordering::SeqCst) || grants.is_empty() || grants.len() > 8 {
            return Err("invalid-session");
        }
        ordinary_desktop()?;
        if grants.iter().any(|g| {
            matches!(
                g.grant.capability.as_str(),
                "control_mouse" | "control_keyboard"
            )
        }) {
            if !cfg!(feature = "native-input") {
                return Err("native-input-disabled");
            }
            source
                .as_ref()
                .ok_or("capture-required")?
                .current_monitor()?;
        }
        let mut gate = SessionGate::new(issuer.key(), binding, lease::now()?, 0, Instant::now());
        for grant in grants {
            gate.admit(grant, lease::now()?)?;
        }
        let gate = Arc::new(Mutex::new(gate));
        let source = source.map(|s| Arc::new(Mutex::new(s)));
        let lost = stop.clone();
        let poller = LeasePoller::start(
            identity.clone(),
            gate.clone(),
            Arc::new(move || lost.store(true, Ordering::SeqCst)),
        );
        let (work, receive) = mpsc::sync_channel(1);
        let ready = Arc::new(AtomicBool::new(false));
        let worker_ready = ready.clone();
        let (worker_gate, worker_source, worker_stop) =
            (gate.clone(), source.clone(), stop.clone());
        let worker = thread::spawn(move || {
            let mut executor: Option<Executor<ProcessInput>> = None;
            let mut last_pulse = Instant::now();
            let mut active_epoch = None;
            let mut quiet_since = Instant::now();
            loop {
                let invalid = lease::now()
                    .and_then(|now| {
                        worker_gate
                            .lock()
                            .map_err(|_| "endpoint-failed")?
                            .tick(now, Instant::now())
                    })
                    .is_err();
                if invalid || ordinary_desktop().is_err() {
                    worker_stop.store(true, Ordering::SeqCst);
                }
                if worker_stop.load(Ordering::SeqCst) {
                    break;
                }
                if let Some(ref source) = worker_source {
                    let source = match source.lock() {
                        Ok(s) => s.clone(),
                        Err(_) => {
                            worker_stop.store(true, Ordering::SeqCst);
                            break;
                        }
                    };
                    if source.current_monitor().is_err() {
                        worker_stop.store(true, Ordering::SeqCst);
                        break;
                    }
                    if !super::input::local_input_idle() {
                        quiet_since = Instant::now();
                    }
                    let input_active = worker_gate
                        .lock()
                        .map(|mut gate| {
                            gate.input_active(lease::now().unwrap_or(0), Instant::now())
                        })
                        .unwrap_or(false);
                    if executor.is_none()
                        && input_active
                        && quiet_since.elapsed() >= Duration::from_millis(250)
                    {
                        let result = (|| {
                            let mut tag = [0u8; 8];
                            if unsafe {
                                windows_sys::Win32::Security::Cryptography::BCryptGenRandom(std::ptr::null_mut(),tag.as_mut_ptr(),8,windows_sys::Win32::Security::Cryptography::BCRYPT_USE_SYSTEM_PREFERRED_RNG)
                            } < 0
                            {
                                return Err("random-failed");
                            }
                            let child =
                                ProcessInput::spawn(usize::from_le_bytes(tag), false, &manifest)?;
                            executor = Some(Executor::new(child, worker_stop.clone(), true));
                            active_epoch =
                                Some(worker_gate.lock().map_err(|_| "endpoint-failed")?.epoch());
                            worker_ready.store(true, Ordering::SeqCst);
                            Ok::<(), &'static str>(())
                        })();
                        if result.is_err() {
                            worker_stop.store(true, Ordering::SeqCst);
                            break;
                        }
                    }
                    if let Some(ref mut executor) = executor {
                        let epoch = worker_gate.lock().map(|g| g.epoch()).unwrap_or(0);
                        // Any grant-generation/revocation change fences held input.
                        if active_epoch != Some(epoch)
                            || (last_pulse.elapsed() >= Duration::from_millis(250)
                                && worker_gate
                                    .lock()
                                    .map(|mut gate| {
                                        executor
                                            .heartbeat(
                                                &mut gate,
                                                &source,
                                                lease::now().unwrap_or(0),
                                                Instant::now(),
                                            )
                                            .is_err()
                                    })
                                    .unwrap_or(true))
                        {
                            worker_stop.store(true, Ordering::SeqCst);
                            break;
                        }
                        if last_pulse.elapsed() >= Duration::from_millis(250) {
                            last_pulse = Instant::now();
                        }
                    }
                }
                let packet = match receive.recv_timeout(Duration::from_millis(20)) {
                    Ok(Work::Packet(p, reply)) => (p, reply),
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => break,
                };
                let result = (|| {
                    let message = input::parse(&packet.0)?;
                    let source = worker_source
                        .as_ref()
                        .ok_or("capture-required")?
                        .lock()
                        .map_err(|_| "capture-failed")?
                        .clone();
                    let mut gate = worker_gate.lock().map_err(|_| "endpoint-failed")?;
                    if executor.is_none() {
                        return Err("input-not-ready");
                    }
                    let permit =
                        gate.permit(&message, source.generation, lease::now()?, Instant::now())?;
                    executor.as_mut().ok_or("endpoint-failed")?.dispatch(
                        permit,
                        &mut gate,
                        &source,
                        &message.action,
                        || Ok((lease::now()?, Instant::now())),
                    )
                })();
                if result.is_err() && result != Err("input-not-ready") {
                    worker_stop.store(true, Ordering::SeqCst);
                }
                let _ = packet.1.send(result);
            }
            worker_stop.store(true, Ordering::SeqCst);
            worker_ready.store(false, Ordering::SeqCst);
            if let Ok(mut gate) = worker_gate.lock() {
                gate.stop();
            }
            if let (Some(mut executor), Some(source)) = (executor, worker_source) {
                if let Ok(source) = source.lock() {
                    // Fence/reap the child before mirror cleanup; protected desktops
                    // keep only a release retry alive, never new remote input.
                    while executor.release_owned(&source).is_err() {
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        });
        Ok(Self {
            gate,
            source,
            stop,
            identity,
            prompts: BTreeMap::new(),
            poller: Some(poller),
            work,
            worker: Some(worker),
            ready,
        })
    }
    pub fn request_consent(&mut self, id: &str) -> Result<(), &'static str> {
        if self.stop.load(Ordering::SeqCst)
            || self.prompts.contains_key(id)
            || self.prompts.len() >= 8
        {
            return Err("consent-unavailable");
        }
        self.prompts.insert(
            id.into(),
            Prompt::request(self.gate.clone(), id, self.source.clone())?,
        );
        Ok(())
    }
    pub fn consent_result(&mut self, id: &str) -> Result<Acknowledgement, &'static str> {
        if self.stop.load(Ordering::SeqCst) {
            return Err("revoked");
        }
        let prompt = self.prompts.get_mut(id).ok_or("consent-required")?;
        let result = match prompt.take_decision() {
            Decision::Pending => Acknowledgement {
                grant_id: id.into(),
                signature: None,
                state: "consent-pending",
            },
            Decision::Denied => Acknowledgement {
                grant_id: id.into(),
                signature: None,
                state: "consent-denied",
            },
            Decision::Allowed(grant) => {
                let current = self
                    .gate
                    .lock()
                    .map_err(|_| "endpoint-failed")?
                    .pending_grant(id, lease::now()?)?;
                if current.canonical() != grant.canonical() {
                    return Err("binding-mismatch");
                }
                Acknowledgement {
                    grant_id: id.into(),
                    signature: if grant.endpoint.is_some() {
                        Some(self.identity.acknowledge(&grant))
                    } else {
                        None
                    },
                    state: "consented",
                }
            }
        };
        if result.state != "consent-pending" {
            self.prompts.remove(id);
        }
        Ok(result)
    }
    /// Called exclusively with the DTLS fingerprint observed by the app-owned
    /// gateway, not an SDP string or boolean supplied by the hosted page.
    pub fn observed_peer(&self, fingerprint: &str) -> Result<(), &'static str> {
        if self.stop.load(Ordering::SeqCst) {
            return Err("revoked");
        }
        self.gate
            .lock()
            .map_err(|_| "endpoint-failed")?
            .verify_peer(fingerprint)
    }
    pub fn packet(&self, packet: String) -> Result<(), &'static str> {
        if self.stop.load(Ordering::SeqCst) {
            return Err("revoked");
        }
        if !self.ready.load(Ordering::SeqCst) {
            return Err("input-not-ready");
        }
        let (send, receive) = mpsc::sync_channel(1);
        self.work
            .try_send(Work::Packet(packet, send))
            .map_err(|_| "rate-limited")?;
        receive
            .recv_timeout(Duration::from_millis(250))
            .map_err(|_| "endpoint-timeout")?
    }
    pub fn authorize(&self, id: &str, capability: &str) -> Result<(), &'static str> {
        if self.stop.load(Ordering::SeqCst) {
            return Err("revoked");
        }
        self.gate
            .lock()
            .map_err(|_| "endpoint-failed")?
            .authorize(id, capability, lease::now()?, Instant::now())
            .map(|_| ())
    }
    pub fn ended(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }
    pub fn input_ready(&self) -> bool {
        !self.ended() && self.ready.load(Ordering::SeqCst)
    }
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(mut gate) = self.gate.lock() {
            gate.stop();
        }
        self.prompts.clear();
        self.poller.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for StreamRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}
