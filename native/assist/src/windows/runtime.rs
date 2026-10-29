//! Dormant stream-session coordinator. Only trusted native IPC/gateway code may
//! call it; no renderer assertion can set consent or replace its issuer.
use super::{
    consent::{Decision, Prompt},
    enrollment::EnrolledIdentity,
    files::SessionFiles,
    identity::Identity,
    input::{Source, ordinary_desktop},
    input_owner::InputOwner,
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
/// Prepared before authority and cloned into urgent readers/watchdogs. Calling
/// either method touches only atomics, never registry/UI/network/join locks.
#[derive(Clone)]
pub struct RuntimeRevoker {
    stop: Arc<AtomicBool>,
    input_stop: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    lease_cancel: Arc<AtomicBool>,
    files: Option<super::files::FileRevoker>,
}
impl RuntimeRevoker {
    pub fn suspend_input(&self) {
        self.input_stop.store(true, Ordering::SeqCst);
        self.ready.store(false, Ordering::SeqCst);
    }
    pub fn revoke(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.suspend_input();
        self.lease_cancel.store(true, Ordering::SeqCst);
        if let Some(files) = &self.files {
            files.revoke();
        }
    }
}
pub struct StreamRuntime {
    gate: Arc<Mutex<SessionGate>>,
    source: Option<Arc<Mutex<Source>>>,
    stop: Arc<AtomicBool>,
    input_stop: Arc<AtomicBool>,
    identity: Arc<Identity>,
    prompts: BTreeMap<String, Prompt>,
    poller: Option<LeasePoller>,
    work: mpsc::SyncSender<Work>,
    worker: crate::cleanup::RetainedWorker,
    ready: Arc<AtomicBool>,
    files: Option<Arc<SessionFiles>>,
    lease_cancel: Arc<AtomicBool>,
    cleanup_failed: bool,
}
impl StreamRuntime {
    pub fn begin(
        enrolled: EnrolledIdentity<'_>,
        binding: Binding,
        grants: &[SignedStreamGrant],
        source: Option<Source>,
        stop_surface: &NativeStop,
        manifest: super::image::ImageManifest,
    ) -> Result<Self, &'static str> {
        if !cfg!(any(feature = "native-input", feature = "native-resources")) {
            return Err("endpoint-disabled");
        }
        let EnrolledIdentity {
            issuer,
            identity,
            enrollment,
        } = enrolled;
        enrollment.verify_binding(issuer, &identity, &binding)?;
        let stop = stop_surface.session_flag()?;
        enrollment.bind_session(&stop)?;
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
        let files = if cfg!(feature = "native-resources") {
            Some(Arc::new(SessionFiles::publisher(
                gate.clone(),
                stop.clone(),
            )?))
        } else {
            None
        };
        let source = source.map(|s| Arc::new(Mutex::new(s)));
        let lost = stop.clone();
        let poller = LeasePoller::start(
            identity.clone(),
            gate.clone(),
            Arc::new(move || lost.store(true, Ordering::SeqCst)),
        );
        let lease_cancel = poller.cancel_flag();
        let (work, receive) = mpsc::sync_channel(1);
        let ready = Arc::new(AtomicBool::new(false));
        let input_stop = Arc::new(AtomicBool::new(false));
        let worker_input_stop = input_stop.clone();
        let worker_ready = ready.clone();
        let (worker_gate, worker_source, worker_stop) =
            (gate.clone(), source.clone(), stop.clone());
        let worker = thread::spawn(move || {
            let mut owner = None;
            // Reverse drop order retains the slot through Executor's fenced
            // mirror cleanup even if the worker unwinds.
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
                        worker_input_stop.store(true, Ordering::SeqCst);
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
                        && !worker_input_stop.load(Ordering::SeqCst)
                        && input_active
                        && quiet_since.elapsed() >= Duration::from_millis(250)
                    {
                        let result = (|| {
                            // The slot remains held until the child is fenced and
                            // its owned input is released, across all streams.
                            owner = Some(InputOwner::acquire()?);
                            let mut tag = [0u8; 8];
                            if unsafe {
                                windows_sys::Win32::Security::Cryptography::BCryptGenRandom(std::ptr::null_mut(),tag.as_mut_ptr(),8,windows_sys::Win32::Security::Cryptography::BCRYPT_USE_SYSTEM_PREFERRED_RNG)
                            } < 0
                            {
                                return Err("random-failed");
                            }
                            let child = ProcessInput::spawn(
                                usize::from_le_bytes(tag),
                                false,
                                &manifest,
                                owner.as_ref().ok_or("input-owner-required")?,
                            )?;
                            executor = Some(
                                Executor::new(child, worker_input_stop.clone(), true)
                                    .with_session_stop(worker_stop.clone()),
                            );
                            active_epoch = Some(
                                worker_gate
                                    .lock()
                                    .map_err(|_| "endpoint-failed")?
                                    .input_epoch(),
                            );
                            worker_ready.store(true, Ordering::SeqCst);
                            Ok::<(), &'static str>(())
                        })();
                        if result.is_err() {
                            worker_input_stop.store(true, Ordering::SeqCst);
                        }
                    }
                    if let Some(ref mut executor) = executor {
                        let epoch = worker_gate.lock().map(|g| g.input_epoch()).unwrap_or(0);
                        // Input revocation fences held input. Independent resource
                        // revocation does not change this input epoch.
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
                            worker_input_stop.store(true, Ordering::SeqCst);
                        }
                        if last_pulse.elapsed() >= Duration::from_millis(250) {
                            last_pulse = Instant::now();
                        }
                    }
                    if worker_input_stop.load(Ordering::SeqCst) {
                        worker_ready.store(false, Ordering::SeqCst);
                        if let Ok(mut gate) = worker_gate.lock() {
                            gate.suspend_input();
                        }
                        if let Some(mut child) = executor.take() {
                            while child.release_owned(&source).is_err() {
                                if ordinary_desktop().is_err() {
                                    worker_stop.store(true, Ordering::SeqCst);
                                }
                                thread::sleep(Duration::from_millis(100));
                            }
                        }
                        owner.take();
                        active_epoch = None;
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
                        || {
                            if worker_stop.load(Ordering::SeqCst)
                                || worker_input_stop.load(Ordering::SeqCst)
                            {
                                return Err("revoked");
                            }
                            Ok((lease::now()?, Instant::now()))
                        },
                    )
                })();
                if result.is_err() && result != Err("input-not-ready") {
                    worker_input_stop.store(true, Ordering::SeqCst);
                }
                let _ = packet.1.send(result);
            }
            worker_stop.store(true, Ordering::SeqCst);
            worker_ready.store(false, Ordering::SeqCst);
            let mut failed = false;
            worker_gate
                .lock()
                .unwrap_or_else(|error| {
                    failed = true;
                    error.into_inner()
                })
                .stop();
            if let (Some(mut executor), Some(source)) = (executor, worker_source) {
                {
                    let source = source.lock().unwrap_or_else(|error| {
                        failed = true;
                        error.into_inner()
                    });
                    // Fence/reap the child before mirror cleanup; protected desktops
                    // keep only a release retry alive, never new remote input.
                    while executor.release_owned(&source).is_err() {
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
            if failed {
                Err("input-cleanup-failed")
            } else {
                Ok(())
            }
        });
        Ok(Self {
            gate,
            source,
            stop,
            input_stop,
            identity,
            prompts: BTreeMap::new(),
            poller: Some(poller),
            work,
            worker: crate::cleanup::RetainedWorker::new(worker),
            ready,
            files,
            lease_cancel,
            cleanup_failed: false,
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
            Prompt::request(
                self.gate.clone(),
                id,
                self.source.clone(),
                self.stop.clone(),
                self.input_stop.clone(),
            )?,
        );
        Ok(())
    }
    pub fn consent_result(&mut self, id: &str) -> Result<Acknowledgement, &'static str> {
        if self.stop.load(Ordering::SeqCst) {
            return Err("revoked");
        }
        let prompt = self.prompts.get_mut(id).ok_or("consent-required")?;
        let decision = prompt.take_decision();
        if !matches!(decision, Decision::Pending) {
            prompt.finish()?;
        }
        let result = match decision {
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
                let input = matches!(
                    grant.capability.as_str(),
                    "control_mouse" | "control_keyboard"
                );
                if self.stop.load(Ordering::SeqCst)
                    || (input && self.input_stop.load(Ordering::SeqCst))
                {
                    return Err("revoked");
                }
                let current = self
                    .gate
                    .lock()
                    .map_err(|_| "endpoint-failed")?
                    .pending_grant(id, lease::now()?)?;
                if current.canonical() != grant.canonical() {
                    return Err("binding-mismatch");
                }
                let acknowledgement = Acknowledgement {
                    grant_id: id.into(),
                    signature: if grant.endpoint.is_some() {
                        Some(self.identity.acknowledge(&grant)?)
                    } else {
                        None
                    },
                    state: "consented",
                };
                if self.stop.load(Ordering::SeqCst)
                    || (input && self.input_stop.load(Ordering::SeqCst))
                {
                    return Err("revoked");
                }
                acknowledgement
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
        if self.input_stop.load(Ordering::SeqCst) || !self.ready.load(Ordering::SeqCst) {
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
        if self.stop.load(Ordering::SeqCst) || ordinary_desktop().is_err() {
            self.stop.store(true, Ordering::SeqCst);
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
    pub fn files(&self) -> Result<Arc<SessionFiles>, &'static str> {
        if self.ended() {
            return Err("revoked");
        }
        self.files.clone().ok_or("native-resources-disabled")
    }
    pub fn input_ready(&self) -> bool {
        !self.ended()
            && !self.input_stop.load(Ordering::SeqCst)
            && self.ready.load(Ordering::SeqCst)
    }
    /// Local suspension invalidates queued permits immediately. The worker fences
    /// and releases input before relinquishing controller ownership. New input
    /// requires a fresh Assist authorization; old consent/leases cannot resume it.
    pub fn suspend_input(&self) {
        self.revoker().suspend_input();
    }
    pub fn revoker(&self) -> RuntimeRevoker {
        RuntimeRevoker {
            stop: self.stop.clone(),
            input_stop: self.input_stop.clone(),
            ready: self.ready.clone(),
            lease_cancel: self.lease_cancel.clone(),
            files: self.files.as_ref().map(|f| f.revoker()),
        }
    }
    /// Blocking retained cleanup, never the urgent path. Attempt every owner
    /// even after an earlier failure; repeated stop cannot erase that failure.
    pub fn stop(&mut self) -> Result<(), &'static str> {
        self.revoker().revoke();
        for prompt in self.prompts.values() {
            prompt.cancel();
        }
        let mut gate = self.gate.lock().unwrap_or_else(|error| {
            self.cleanup_failed = true;
            error.into_inner()
        });
        gate.stop();
        drop(gate);
        let mut completed = Vec::new();
        for (id, prompt) in &mut self.prompts {
            if crate::cleanup::attempt(|| prompt.finish()).is_err() {
                self.cleanup_failed = true;
            } else {
                completed.push(id.clone());
            }
        }
        for id in completed {
            if crate::cleanup::attempt(|| {
                drop(self.prompts.remove(&id));
                Ok(())
            })
            .is_err()
            {
                self.cleanup_failed = true;
            }
        }
        let poller_complete = self
            .poller
            .as_mut()
            .is_none_or(|poller| crate::cleanup::attempt(|| poller.finish()).is_ok());
        if poller_complete {
            if crate::cleanup::attempt(|| {
                drop(self.poller.take());
                Ok(())
            })
            .is_err()
            {
                self.cleanup_failed = true;
            }
        } else {
            self.cleanup_failed = true;
        }
        if self.worker.join().is_err() {
            self.cleanup_failed = true;
        }
        if let Some(files) = &self.files {
            if crate::cleanup::attempt(|| files.wait_cleanup()).is_err() {
                self.cleanup_failed = true;
            }
        }
        if self.cleanup_failed {
            Err("runtime-cleanup-failed")
        } else {
            Ok(())
        }
    }
}
impl Drop for StreamRuntime {
    fn drop(&mut self) {
        let _ = crate::cleanup::attempt(|| self.stop());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_only_revocation_preserves_resource_and_poller_flags() {
        let revoker = RuntimeRevoker {
            stop: Arc::new(AtomicBool::new(false)),
            input_stop: Arc::new(AtomicBool::new(false)),
            ready: Arc::new(AtomicBool::new(true)),
            lease_cancel: Arc::new(AtomicBool::new(false)),
            files: None,
        };
        revoker.suspend_input();
        assert!(revoker.input_stop.load(Ordering::SeqCst));
        assert!(!revoker.ready.load(Ordering::SeqCst));
        assert!(!revoker.stop.load(Ordering::SeqCst));
        assert!(!revoker.lease_cancel.load(Ordering::SeqCst));
        revoker.revoke();
        assert!(revoker.stop.load(Ordering::SeqCst));
        assert!(revoker.lease_cancel.load(Ordering::SeqCst));
    }
}
