//! Dormant stream-session coordinator. Only trusted native IPC/gateway code may
//! call it; no renderer assertion can set consent or replace its issuer.
use super::{
    consent::{Decision, Prompt},
    enrollment::EnrolledIdentity,
    files::SessionFiles,
    identity::EndpointIdentity,
    input::{Source, ordinary_desktop},
    input_owner::InputOwner,
    lease::{self, LeasePoller},
    process_input::ProcessInput,
    process_tree::StopEvent,
};
use crate::{
    executor::Executor,
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
    Packet(String, Instant, mpsc::SyncSender<Result<(), &'static str>>),
}
pub use crate::input::InputProfile;
/// Constructed by the launch-owned endpoint from its independently monitored
/// stop witness and private custody connection; never accepted from a page.
pub struct RuntimeOwner {
    pub(crate) stop: crate::cancellation::SessionStop,
    pub(crate) custody: Arc<super::process_input::custody::Client>,
    pub(crate) source: super::capture::Binding,
}
/// Prepared before authority and cloned into urgent readers/watchdogs. Calling
/// either method signals the retained native event and uses atomics, without
/// taking a SessionGate, UI, network or join lock.
#[derive(Clone)]
pub struct RuntimeRevoker {
    stop: crate::cancellation::SessionStop,
    input_stop: Arc<AtomicBool>,
    executor_stop: Option<Arc<StopEvent>>,
    ready: Arc<AtomicBool>,
    lease_cancel: Arc<AtomicBool>,
    files: Option<super::files::FileRevoker>,
}
#[derive(Clone)]
pub(crate) struct InputRevoker {
    input_stop: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    executor_stop: Option<Arc<StopEvent>>,
}
impl InputRevoker {
    pub(crate) fn revoke(&self) {
        self.input_stop.store(true, Ordering::SeqCst);
        self.ready.store(false, Ordering::SeqCst);
        if let Some(event) = &self.executor_stop {
            let _ = event.revoke();
        }
    }
}
impl RuntimeRevoker {
    #[cfg(test)]
    pub(crate) fn no_authority() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(false)).into(),
            input_stop: Arc::new(AtomicBool::new(false)),
            executor_stop: None,
            ready: Arc::new(AtomicBool::new(false)),
            lease_cancel: Arc::new(AtomicBool::new(false)),
            files: None,
        }
    }
    #[cfg(test)]
    pub(crate) fn input_stopped(&self) -> bool {
        self.input_stop.load(Ordering::SeqCst)
    }
    #[cfg(test)]
    pub(crate) fn session_stopped(&self) -> bool {
        self.stop.is_stopped()
    }
    pub fn suspend_input(&self) {
        self.input_revoker().revoke();
    }
    pub(crate) fn input_revoker(&self) -> InputRevoker {
        InputRevoker {
            input_stop: self.input_stop.clone(),
            ready: self.ready.clone(),
            executor_stop: self.executor_stop.clone(),
        }
    }
    pub fn revoke(&self) {
        self.stop.revoke();
        self.suspend_input();
        self.lease_cancel.store(true, Ordering::SeqCst);
        if let Some(files) = &self.files {
            files.revoke();
        }
    }
}
pub struct StreamRuntime {
    generation: String,
    peer: Arc<Mutex<Option<OwnedPeer>>>,
    profile: InputProfile,
    gate: Arc<Mutex<SessionGate>>,
    source: Option<Arc<Mutex<Source>>>,
    stop: crate::cancellation::SessionStop,
    input_stop: Arc<AtomicBool>,
    executor_stop: Option<Arc<StopEvent>>,
    identity: Arc<EndpointIdentity>,
    prompts: BTreeMap<String, Prompt>,
    poller: Option<LeasePoller>,
    work: mpsc::SyncSender<Work>,
    worker: crate::cleanup::RetainedWorker,
    ready: Arc<AtomicBool>,
    files: Option<Arc<SessionFiles>>,
    lease_cancel: Arc<AtomicBool>,
    cleanup_failed: bool,
    expected_track: String,
    worker_binding: InputWorkerBinding,
    executor_stops: Arc<Mutex<Vec<Arc<StopEvent>>>>,
}
struct OwnedPeer {
    attempt: String,
    permit: Option<crate::stream::PeerPermit>,
    resources: Option<crate::stream::PeerPermit>,
    cancelled: Arc<AtomicBool>,
    sealed: bool,
    observed: bool,
    ice_requested: bool,
    deadline: Instant,
}
/// The exact connection and original input generation. Closing an old attempt
/// cannot revoke a successor or unrelated session/resource grants.
#[derive(Clone)]
pub(crate) struct PeerRevoker {
    cancelled: Arc<AtomicBool>,
    input: Option<InputRevoker>,
}
impl PeerRevoker {
    #[cfg(test)]
    pub(crate) fn no_authority(input: InputRevoker) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            input: Some(input),
        }
    }
    pub(crate) fn revoke(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Some(input) = &self.input {
            input.revoke();
        }
    }
}
/// References the existing gate/worker; it owns no second authorization or
/// executor. A dedicated bounded IPC lane keeps consent dialogs from starving
/// an already authenticated peer's health checks and packet admission.
#[derive(Clone)]
pub(crate) struct PeerRuntime {
    generation: String,
    input_revoker: InputRevoker,
    peer: Arc<Mutex<Option<OwnedPeer>>>,
    profile: InputProfile,
    gate: Arc<Mutex<SessionGate>>,
    stop: crate::cancellation::SessionStop,
    input_stop: Arc<AtomicBool>,
    identity: Arc<EndpointIdentity>,
    work: mpsc::SyncSender<Work>,
    ready: Arc<AtomicBool>,
    lease_cancel: Arc<AtomicBool>,
    facts: Arc<Mutex<Option<lease::ServiceFacts>>>,
    expected_track: String,
}
fn with_live_gate<T>(
    gate: &Mutex<SessionGate>,
    stop: &crate::cancellation::SessionStop,
    operation: impl FnOnce(&mut SessionGate) -> Result<T, &'static str>,
) -> Result<T, &'static str> {
    if stop.is_stopped() {
        return Err("revoked");
    }
    let mut gate = gate.lock().map_err(|_| "endpoint-failed")?;
    // A busy gate must not turn an operation queued before Stop into new work.
    if stop.is_stopped() {
        gate.stop();
        return Err("revoked");
    }
    let result = operation(&mut gate);
    if stop.is_stopped() {
        gate.stop();
        return Err("revoked");
    }
    result
}
fn current_view(
    facts: &Mutex<Option<lease::ServiceFacts>>,
    track: &str,
) -> Result<lease::ServiceFacts, &'static str> {
    let facts = facts
        .lock()
        .map_err(|_| "endpoint-failed")?
        .clone()
        .ok_or("source-unconfirmed")?;
    if facts.stream_track_name.as_deref() != Some(track)
        || facts.observed_at.elapsed() >= Duration::from_secs(20)
    {
        return Err("source-unconfirmed");
    }
    Ok(facts)
}
fn release_registered_input(
    gate: &Arc<Mutex<SessionGate>>,
    executor: &mut Executor<ProcessInput>,
    source: &Source,
    stop: &crate::cancellation::SessionStop,
) -> Result<(), &'static str> {
    let requirement = gate
        .lock()
        .map_err(|_| "input-cleanup-gate")?
        .require_input_cleanup()?;
    loop {
        match executor.release_for(&requirement, source) {
            Ok(receipt) => {
                return gate
                    .lock()
                    .map_err(|_| "input-cleanup-gate")?
                    .accept_input_cleanup(receipt);
            }
            // Retain the same requirement, executor and input slot. Protected
            // desktops or incomplete mirror writes cannot become success.
            Err("executor-binding-mismatch" | "cleanup-replayed" | "native-input-disabled") => {
                return Err("input-cleanup-unproven");
            }
            Err(_) => {
                if ordinary_desktop().is_err() {
                    stop.revoke_launch();
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}
#[derive(Clone)]
struct InputWorkerBinding {
    worker_gate: Arc<Mutex<SessionGate>>,
    worker_source: Option<Arc<Mutex<Source>>>,
    worker_stop: crate::cancellation::SessionStop,
    worker_facts: Arc<Mutex<Option<lease::ServiceFacts>>>,
    worker_expected_track: String,
    custody: Arc<super::process_input::custody::Client>,
    source_key: super::source_stop::SourceKey,
    profile: InputProfile,
}
struct InputWorker {
    input_stop: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    work: mpsc::SyncSender<Work>,
    worker: crate::cleanup::RetainedWorker,
}
fn spawn_input_worker(
    binding: InputWorkerBinding,
    executor_stop: Option<Arc<StopEvent>>,
) -> InputWorker {
    let InputWorkerBinding {
        worker_gate,
        worker_source,
        worker_stop,
        worker_facts,
        worker_expected_track,
        custody,
        source_key,
        profile,
    } = binding;
    let (work, receive) = mpsc::sync_channel(1);
    let ready = Arc::new(AtomicBool::new(false));
    let input_stop = Arc::new(AtomicBool::new(false));
    let worker_input_stop = input_stop.clone();
    let worker_executor_stop = executor_stop;
    let worker_ready = ready.clone();
    let worker = thread::spawn(move || {
        let mut owner = None;
        // Reverse drop order retains the slot through Executor's fenced
        // mirror cleanup even if the worker unwinds.
        let mut executor: Option<Executor<ProcessInput>> = None;
        let mut last_pulse = Instant::now();
        let mut active_epoch = None;
        let mut quiet_since = Instant::now();
        loop {
            if worker_facts
                .lock()
                .map(|facts| {
                    facts.as_ref().is_some_and(|facts| {
                        facts.stream_track_name.as_deref() != Some(&worker_expected_track)
                    })
                })
                .unwrap_or(true)
            {
                worker_stop.revoke();
            }
            let invalid = lease::now()
                .and_then(|now| {
                    worker_gate
                        .lock()
                        .map_err(|_| "endpoint-failed")?
                        .tick(now, Instant::now())
                })
                .is_err();
            if invalid {
                worker_stop.revoke();
            }
            if worker_gate
                .lock()
                .map(|gate| gate.input_generation_ended())
                .unwrap_or(true)
            {
                worker_input_stop.store(true, Ordering::SeqCst);
            }
            if ordinary_desktop().is_err() {
                worker_stop.revoke_launch();
            }
            if worker_stop.is_stopped() {
                break;
            }
            if let Some(ref source) = worker_source {
                let source = match source.lock() {
                    Ok(s) => s.clone(),
                    Err(_) => {
                        worker_stop.revoke();
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
                    .map(|mut gate| gate.input_active(lease::now().unwrap_or(0), Instant::now()))
                    .unwrap_or(false);
                if executor.is_none()
                    && !worker_input_stop.load(Ordering::SeqCst)
                    && input_active
                    && quiet_since.elapsed() >= Duration::from_millis(250)
                {
                    let result = (|| {
                        let direct_stop = worker_executor_stop
                            .as_ref()
                            .ok_or("executor-stop-required")?
                            .clone();
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
                        let (reservation, registration, start_epoch) = {
                            let mut gate = worker_gate.lock().map_err(|_| "endpoint-failed")?;
                            let reservation = gate.reserve_executor()?;
                            let registration = reservation.custody(
                                &gate.binding().session,
                                source.clone(),
                                source_key.clone(),
                            );
                            (reservation, registration, gate.input_epoch())
                        };
                        let child = match ProcessInput::spawn_monitored(
                            &custody,
                            registration,
                            usize::from_le_bytes(tag),
                            owner.as_ref().ok_or("input-owner-required")?.raw(),
                            direct_stop,
                        ) {
                            Ok(child) => child,
                            Err(error) => {
                                worker_gate
                                    .lock()
                                    .map_err(|_| "endpoint-failed")?
                                    .cancel_unstarted_executor(reservation)?;
                                return Err(error);
                            }
                        };
                        executor = Some(
                            Executor::registered(
                                child,
                                worker_input_stop.clone(),
                                true,
                                reservation,
                            )
                            .with_session_stop(worker_stop.clone()),
                        );
                        let current = {
                            let mut gate = worker_gate.lock().map_err(|_| "endpoint-failed")?;
                            gate.input_epoch() == start_epoch
                                && gate.input_active(lease::now()?, Instant::now())
                        };
                        if !current
                            || worker_stop.is_stopped()
                            || worker_input_stop.load(Ordering::SeqCst)
                        {
                            return Err("revoked");
                        }
                        active_epoch = Some(start_epoch);
                        worker_ready.store(true, Ordering::SeqCst);
                        Ok::<(), &'static str>(())
                    })();
                    if result.is_err() && result != Err("controller-busy") {
                        worker_input_stop.store(true, Ordering::SeqCst);
                        // No registered executor can consume this local slot.
                        // Failed startup either retained its monitor-owned
                        // copy or positively settled before returning. Do not
                        // pin this coordinator's extra copy until unrelated
                        // clipboard/file permissions also end.
                        if executor.is_none() {
                            owner.take();
                        }
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
            }
            if worker_input_stop.load(Ordering::SeqCst) {
                worker_ready.store(false, Ordering::SeqCst);
                if let Ok(mut gate) = worker_gate.lock() {
                    gate.suspend_input();
                }
                if let Some(mut child) = executor.take() {
                    let source = worker_source
                        .as_ref()
                        .ok_or("capture-required")?
                        .lock()
                        .map_err(|_| "capture-failed")?
                        .clone();
                    if release_registered_input(&worker_gate, &mut child, &source, &worker_stop)
                        .is_err()
                    {
                        worker_stop.revoke();
                        return Err("input-cleanup-unproven");
                    }
                }
                owner.take();
                // A no-child generation also has a positive joined outcome.
                // Input ends here; independent resources retain their authority.
                return Ok(());
            }
            let packet = match receive.recv_timeout(Duration::from_millis(20)) {
                Ok(Work::Packet(p, arrived, reply)) => (p, arrived, reply),
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            };
            let result = (|| {
                let source = worker_source
                    .as_ref()
                    .ok_or("capture-required")?
                    .lock()
                    .map_err(|_| "capture-failed")?
                    .clone();
                let mut gate = worker_gate.lock().map_err(|_| "endpoint-failed")?;
                gate.tick(lease::now()?, Instant::now())?;
                if gate.validate_worker_epoch(active_epoch).is_err() {
                    worker_input_stop.store(true, Ordering::SeqCst);
                    return Err("revoked");
                }
                if executor.is_none() {
                    return Err("input-not-ready");
                }
                let (permit, message, queued_until) = gate.permit_packet(
                    &packet.0,
                    profile,
                    source.generation,
                    lease::now()?,
                    (packet.1, Instant::now()),
                )?;
                executor.as_mut().ok_or("endpoint-failed")?.dispatch(
                    permit,
                    &mut gate,
                    &source,
                    &message.action,
                    || {
                        if worker_stop.is_stopped()
                            || worker_input_stop.load(Ordering::SeqCst)
                            || Instant::now() >= queued_until
                        {
                            return Err("revoked");
                        }
                        Ok((lease::now()?, Instant::now()))
                    },
                )
            })();
            if result.is_err() && result != Err("input-not-ready") && result != Err("stale-input") {
                worker_input_stop.store(true, Ordering::SeqCst);
            }
            let _ = packet.2.send(crate::input::packet_result(result));
        }
        worker_stop.revoke();
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
                let source = source
                    .lock()
                    .unwrap_or_else(|error| {
                        failed = true;
                        error.into_inner()
                    })
                    .clone();
                // Fence/reap the child before mirror cleanup; protected desktops
                // keep only a release retry alive, never new remote input.
                release_registered_input(&worker_gate, &mut executor, &source, &worker_stop)?;
            }
        }
        if failed {
            Err("input-cleanup-failed")
        } else {
            Ok(())
        }
    });
    InputWorker {
        input_stop,
        ready,
        work,
        worker: crate::cleanup::RetainedWorker::new(worker),
    }
}
impl StreamRuntime {
    pub fn begin(
        enrolled: EnrolledIdentity<'_>,
        binding: Binding,
        grants: &[SignedStreamGrant],
        source: Option<Source>,
        owner: RuntimeOwner,
    ) -> Result<Self, &'static str> {
        Self::begin_with_profile(
            enrolled,
            binding,
            grants,
            source,
            owner,
            InputProfile::Legacy,
        )
    }
    pub fn begin_with_profile(
        enrolled: EnrolledIdentity<'_>,
        binding: Binding,
        grants: &[SignedStreamGrant],
        source: Option<Source>,
        owner: RuntimeOwner,
        profile: InputProfile,
    ) -> Result<Self, &'static str> {
        Self::begin_seeded(enrolled, binding, grants, source, owner, profile, None)
    }
    pub(crate) fn begin_seeded(
        enrolled: EnrolledIdentity<'_>,
        binding: Binding,
        grants: &[SignedStreamGrant],
        source: Option<Source>,
        owner: RuntimeOwner,
        profile: InputProfile,
        initial: Option<lease::InitialLease>,
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
        if owner.custody.probe() {
            return Err("native-runtime-unqualified");
        }
        let source_key = owner.source.key.clone();
        let expected_track = format!("palisade-capture:{}", source_key.operation);
        let stop = owner.stop.scoped(Arc::new(owner.source));
        let custody = owner.custody;
        enrollment.bind_session(&stop)?;
        if stop.is_stopped() || grants.is_empty() || grants.len() > 8 {
            return Err("invalid-session");
        }
        ordinary_desktop()?;
        let has_input_grant = grants.iter().any(|g| {
            matches!(
                g.grant.capability.as_str(),
                "control_mouse" | "control_keyboard"
            )
        });
        if has_input_grant {
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
        let initial = initial
            .as_ref()
            .map(|seed| seed.initialize(&mut gate, &expected_track, lease::now()?, Instant::now()))
            .transpose()?;
        if stop.is_stopped() {
            return Err("revoked");
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
        // This runtime can admit at most one executor generation. Register its
        // event with the independent local Stop owner before creating the worker.
        // A source-bearing runtime can receive its first input grant from a
        // later lease update. Reserve Stop ownership for that path as well.
        let executor_stop = (cfg!(feature = "native-input") && source.is_some())
            .then(|| StopEvent::new().map(Arc::new))
            .transpose()?;
        let generation = super::identity::peer_token()?;
        let executor_stops = Arc::new(Mutex::new(
            executor_stop.iter().cloned().collect::<Vec<_>>(),
        ));
        let lease_executor_stops = executor_stops.clone();
        let lost = stop.clone();
        let poller = LeasePoller::start_seeded(
            identity.clone(),
            gate.clone(),
            Arc::new(move || {
                lost.revoke();
                for event in lease_executor_stops
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .iter()
                {
                    let _ = event.revoke();
                }
            }),
            initial,
        );
        let lease_cancel = poller.cancel_flag();
        let worker_binding = InputWorkerBinding {
            worker_gate: gate.clone(),
            worker_source: source.clone(),
            worker_stop: stop.clone(),
            worker_facts: poller.facts_reader(),
            worker_expected_track: expected_track.clone(),
            custody,
            source_key,
            profile,
        };
        let input_worker = spawn_input_worker(worker_binding.clone(), executor_stop.clone());
        Ok(Self {
            generation,
            peer: Arc::new(Mutex::new(None)),
            profile,
            gate,
            source,
            stop,
            input_stop: input_worker.input_stop,
            executor_stop,
            identity,
            prompts: BTreeMap::new(),
            poller: Some(poller),
            work: input_worker.work,
            worker: input_worker.worker,
            ready: input_worker.ready,
            worker_binding,
            executor_stops,
            files,
            lease_cancel,
            cleanup_failed: false,
            expected_track,
        })
    }
    pub(crate) fn source_confirmed(&self) -> Result<bool, &'static str> {
        if self.ended() {
            return Err("revoked");
        }
        match self.poller.as_ref().and_then(LeasePoller::service_facts) {
            None => Ok(false),
            Some(facts) if facts.stream_track_name.as_deref() == Some(&self.expected_track) => {
                Ok(true)
            }
            Some(_) => {
                self.revoker().revoke();
                Err("source-mismatch")
            }
        }
    }
    pub(crate) fn resource_grant(
        &self,
        id: &str,
    ) -> Result<crate::stream::StreamGrant, &'static str> {
        if !self.source_confirmed()? {
            return Err("source-unconfirmed");
        }
        with_live_gate(&self.gate, &self.stop, |gate| {
            let facts = current_view(
                &self.poller.as_ref().ok_or("revoked")?.facts_reader(),
                &self.expected_track,
            )?;
            let grant = gate.resource_transport_grant(id, lease::now()?, Instant::now())?;
            if !facts
                .current_grants
                .iter()
                .any(|g| g.canonical() == grant.canonical())
            {
                return Err("not-granted");
            }
            Ok(grant)
        })
    }
    pub(crate) fn peer_handle(&self) -> Result<PeerRuntime, &'static str> {
        Ok(PeerRuntime {
            generation: self.generation.clone(),
            input_revoker: self.revoker().input_revoker(),
            peer: self.peer.clone(),
            profile: self.profile,
            gate: self.gate.clone(),
            stop: self.stop.clone(),
            input_stop: self.input_stop.clone(),
            identity: self.identity.clone(),
            work: self.work.clone(),
            ready: self.ready.clone(),
            lease_cancel: self.lease_cancel.clone(),
            facts: self.poller.as_ref().ok_or("revoked")?.facts_reader(),
            expected_track: self.expected_track.clone(),
        })
    }
    pub(crate) fn admit(&mut self, signed: &SignedStreamGrant) -> Result<bool, &'static str> {
        if !cfg!(feature = "native-input")
            && matches!(
                signed.grant.capability.as_str(),
                "control_mouse" | "control_keyboard"
            )
        {
            return Err("native-input-disabled");
        }
        if matches!(
            signed.grant.capability.as_str(),
            "control_mouse" | "control_keyboard"
        ) {
            let disposition = with_live_gate(&self.gate, &self.stop, |gate| {
                gate.classify_input_offer(signed, lease::now()?)
            })?;
            if matches!(disposition, crate::stream::InputOffer::Replacement) {
                self.revoker().suspend_input();
            }
        }
        if self.input_stop.load(Ordering::SeqCst)
            && matches!(
                signed.grant.capability.as_str(),
                "control_mouse" | "control_keyboard"
            )
        {
            // Urgent Stop sets this flag before the ordinary worker reaches its
            // gate. Do not race that cleanup into a false duplicate-grant error.
            with_live_gate(&self.gate, &self.stop, |gate| {
                gate.suspend_input();
                Ok(())
            })?;
        }
        let (fresh, extends) = with_live_gate(&self.gate, &self.stop, |gate| {
            Ok((
                gate.needs_input_reacquisition(signed),
                gate.extends_fresh_input_batch(signed),
            ))
        })?;
        if !fresh {
            return with_live_gate(&self.gate, &self.stop, |gate| {
                gate.admit(signed, lease::now()?)
            })
            .map(|_| false);
        }
        if extends {
            return with_live_gate(&self.gate, &self.stop, |gate| {
                gate.reacquire_input(signed, lease::now()?)
            })
            .map(|_| false);
        }
        // A successful ACK/Stop flag is not the worker's positive release proof.
        // Join only its completed result; retained failure can never be retried
        // into success or replaced by a new generation.
        if !self.input_stop.load(Ordering::SeqCst) || !self.worker.is_finished() {
            return Err("cleanup-pending");
        }
        if self.worker.join().is_err() {
            self.cleanup_failed = true;
            return Err("input-cleanup-unproven");
        }
        if !self.source_confirmed()? {
            return Err("source-unconfirmed");
        }
        let event = Arc::new(StopEvent::new()?);
        let generation = super::identity::peer_token()?;
        {
            let mut events = self.executor_stops.lock().map_err(|_| "endpoint-failed")?;
            if events.len() >= 128 {
                return Err("executor-generation-limit");
            }
            events.push(event.clone());
        }
        with_live_gate(&self.gate, &self.stop, |gate| {
            gate.reacquire_input(signed, lease::now()?)
        })?;
        let worker = spawn_input_worker(self.worker_binding.clone(), Some(event.clone()));
        self.peer = Arc::new(Mutex::new(None));
        self.input_stop = worker.input_stop;
        self.ready = worker.ready;
        self.work = worker.work;
        self.worker = worker.worker;
        self.executor_stop = Some(event);
        self.generation = generation;
        if self.stop.is_stopped() {
            self.revoker().suspend_input();
            return Err("revoked");
        }
        Ok(true)
    }
    pub(crate) fn bind_operation(
        &self,
        id: &str,
        flag: Arc<AtomicBool>,
    ) -> Result<(), &'static str> {
        with_live_gate(&self.gate, &self.stop, |gate| {
            gate.bind_operation(id, flag, lease::now()?)
        })
    }
    pub(crate) fn bind_offer(
        &self,
        signed: &SignedStreamGrant,
        flag: Arc<AtomicBool>,
    ) -> Result<(), &'static str> {
        with_live_gate(&self.gate, &self.stop, |gate| {
            gate.bind_offered_operation(signed, flag, lease::now()?)
        })
    }
    pub(crate) fn reject_offer(&self, signed: &SignedStreamGrant) -> Result<(), &'static str> {
        with_live_gate(&self.gate, &self.stop, |gate| {
            gate.reject_offer(signed, lease::now()?)
        })
    }
    pub(crate) fn grant_observed(&self, id: &str) -> Result<bool, &'static str> {
        let reader = self.poller.as_ref().ok_or("revoked")?.facts_reader();
        if reader.lock().map_err(|_| "endpoint-failed")?.is_none() {
            return Ok(false);
        }
        let facts = current_view(&reader, &self.expected_track)?;
        with_live_gate(&self.gate, &self.stop, |gate| {
            let grant = gate.pending_grant(id, lease::now()?)?;
            Ok(facts
                .current_grants
                .iter()
                .any(|record| record.canonical() == grant.canonical()))
        })
    }
    pub(crate) fn current_acknowledgement(&self, id: &str) -> Result<Option<String>, &'static str> {
        with_live_gate(&self.gate, &self.stop, |gate| {
            gate.lease_current(lease::now()?, Instant::now())?;
            let facts = current_view(
                &self.poller.as_ref().ok_or("revoked")?.facts_reader(),
                &self.expected_track,
            )?;
            let Some(grant) = gate.consented_grant(id, lease::now()?)? else {
                return Ok(None);
            };
            if !facts
                .current_grants
                .iter()
                .any(|record| record.canonical() == grant.canonical())
            {
                return Err("not-granted");
            }
            if matches!(
                grant.capability.as_str(),
                "control_mouse" | "control_keyboard"
            ) && self.input_stop.load(Ordering::SeqCst)
            {
                return Err("revoked");
            }
            self.identity.acknowledge(&grant).map(Some)
        })
    }
    pub(crate) fn request_consent(
        &mut self,
        id: &str,
        request: &super::endpoint::GrantRequest,
    ) -> Result<(), &'static str> {
        if !cfg!(feature = "native-input") {
            let grant = self
                .gate
                .lock()
                .map_err(|_| "endpoint-failed")?
                .pending_grant(id, lease::now()?)?;
            if matches!(
                grant.capability.as_str(),
                "control_mouse" | "control_keyboard"
            ) {
                return Err("native-input-disabled");
            }
        }
        if !self.source_confirmed()? {
            return Err("source-unconfirmed");
        }
        if self.stop.is_stopped() || self.prompts.contains_key(id) || self.prompts.len() >= 8 {
            return Err("consent-unavailable");
        }
        self.prompts.insert(
            id.into(),
            Prompt::request(
                self.gate.clone(),
                id,
                self.source.clone(),
                self.stop.scoped(Arc::new(request.clone())),
                self.input_stop.clone(),
            )?,
        );
        Ok(())
    }
    pub fn consent_result(&mut self, id: &str) -> Result<Acknowledgement, &'static str> {
        if self.stop.is_stopped() {
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
                if self.stop.is_stopped() || (input && self.input_stop.load(Ordering::SeqCst)) {
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
                        Some(
                            self.current_acknowledgement(id)?
                                .ok_or("consent-required")?,
                        )
                    } else {
                        None
                    },
                    state: "consented",
                };
                if self.stop.is_stopped() || (input && self.input_stop.load(Ordering::SeqCst)) {
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
    pub(crate) fn harvest_prompts(&mut self) {
        let mut completed = Vec::new();
        for (id, prompt) in &mut self.prompts {
            let revoked = self
                .gate
                .lock()
                .map(|mut gate| gate.pending_grant(id, lease::now().unwrap_or(0)).is_err())
                .unwrap_or(true);
            if revoked || matches!(prompt.decision(), Decision::Denied) {
                prompt.cancel();
                if prompt.finish().is_ok() {
                    completed.push(id.clone());
                } else {
                    self.cleanup_failed = true;
                }
            }
        }
        for id in completed {
            self.prompts.remove(&id);
        }
        if self.cleanup_failed {
            self.revoker().revoke();
        }
    }
    pub fn authorize(&self, id: &str, capability: &str) -> Result<(), &'static str> {
        if self.stop.is_stopped() {
            return Err("revoked");
        }
        if ordinary_desktop().is_err() {
            self.stop.revoke_launch();
            return Err("revoked");
        }
        with_live_gate(&self.gate, &self.stop, |gate| {
            gate.authorize(id, capability, lease::now()?, Instant::now())
                .map(|_| ())
        })
    }
    pub fn ended(&self) -> bool {
        self.stop.is_stopped()
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
            executor_stop: self.executor_stop.clone(),
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
        self.cleanup_failed |= self
            .executor_stop
            .as_ref()
            .is_some_and(|event| event.signal_failed());
        if self.cleanup_failed {
            Err("runtime-cleanup-failed")
        } else {
            Ok(())
        }
    }
}
impl PeerRuntime {
    pub(crate) fn inspect_grant(&self, encoded: &str) -> Result<serde_json::Value, &'static str> {
        if self.stop.is_stopped()
            || self.lease_cancel.load(Ordering::SeqCst)
            || encoded.len() > 4096
        {
            return Err("revoked");
        }
        let signed: SignedStreamGrant =
            serde_json::from_str(encoded).map_err(|_| "invalid-message")?;
        with_live_gate(&self.gate, &self.stop, |gate| {
            let disposition = match gate.classify_input_offer(&signed, lease::now()?)? {
                crate::stream::InputOffer::Existing => "current",
                crate::stream::InputOffer::Additional => "additional",
                crate::stream::InputOffer::Replacement => "replacement",
            };
            let facts = current_view(&self.facts, &self.expected_track)?;
            if !facts
                .current_grants
                .iter()
                .any(|g| g.canonical() == signed.grant.canonical())
            {
                return Err("not-granted");
            }
            Ok(serde_json::json!({"disposition":disposition,"generation":self.generation}))
        })
    }
    pub(crate) fn grant_state(&self, id: &str) -> Result<serde_json::Value, &'static str> {
        self.peer_live()?;
        with_live_gate(&self.gate, &self.stop, |gate| {
            gate.lease_current(lease::now()?, Instant::now())?;
            let grant = gate
                .consented_grant(id, lease::now()?)?
                .ok_or("consent-pending")?;
            let facts = current_view(&self.facts, &self.expected_track)?;
            if !matches!(
                grant.capability.as_str(),
                "control_mouse" | "control_keyboard"
            ) || !facts
                .current_grants
                .iter()
                .any(|record| record.canonical() == grant.canonical())
            {
                return Err("not-granted");
            }
            Ok(serde_json::json!({"generation":self.generation}))
        })
    }
    fn check_cohort(
        &self,
        gate: &mut SessionGate,
        permit: &crate::stream::PeerPermit,
    ) -> Result<(), &'static str> {
        let facts = current_view(&self.facts, &self.expected_track)?;
        gate.validate_peer_cohort(permit, &facts.current_grants, lease::now()?, Instant::now())
    }
    pub(crate) fn peer_revoker(&self, attempt: &str) -> Result<PeerRevoker, &'static str> {
        let peer = self.peer.lock().map_err(|_| "endpoint-failed")?;
        let peer = peer
            .as_ref()
            .filter(|peer| peer.attempt == attempt)
            .ok_or("stale-peer")?;
        Ok(PeerRevoker {
            cancelled: peer.cancelled.clone(),
            input: peer.permit.as_ref().map(|_| self.input_revoker.clone()),
        })
    }
    /// Called exclusively with the DTLS fingerprint observed by the app-owned
    /// gateway, not an SDP string or boolean supplied by the hosted page.
    fn peer_live(&self) -> Result<(), &'static str> {
        if self.stop.is_stopped() || self.lease_cancel.load(Ordering::SeqCst) {
            return Err("revoked");
        }
        if self
            .facts
            .lock()
            .map_err(|_| "endpoint-failed")?
            .as_ref()
            .and_then(|facts| facts.stream_track_name.as_deref())
            != Some(self.expected_track.as_str())
        {
            return Err("source-unconfirmed");
        }
        if ordinary_desktop().is_err() {
            self.stop.revoke_launch();
            return Err("revoked");
        }
        Ok(())
    }
    pub(crate) fn prepare_peer(&self, grant: &str) -> Result<serde_json::Value, &'static str> {
        self.peer_live()?;
        let mut peer = self.peer.lock().map_err(|_| "endpoint-failed")?;
        if peer.is_some() {
            return Err("peer-already-bound");
        }
        let (permit, resources, cancelled, channels) =
            with_live_gate(&self.gate, &self.stop, |gate| {
                let facts = current_view(&self.facts, &self.expected_track)?;
                let selected = gate.pending_grant(grant, lease::now()?)?;
                if !facts
                    .current_grants
                    .iter()
                    .any(|record| record.canonical() == selected.canonical())
                {
                    return Err("not-granted");
                }
                let input = matches!(
                    selected.capability.as_str(),
                    "control_mouse" | "control_keyboard"
                );
                let permit = if input {
                    if !cfg!(feature = "native-input") || self.input_stop.load(Ordering::SeqCst) {
                        return Err("native-input-disabled");
                    }
                    if !gate.input_cohort_decided(&facts.current_grants, grant, lease::now()?)? {
                        return Err("consent-pending");
                    }
                    Some(gate.prepare_peer(grant, lease::now()?, Instant::now())?)
                } else {
                    None
                };
                let cancelled = permit.as_ref().map_or_else(
                    || Arc::new(AtomicBool::new(false)),
                    |permit| permit.cancellation(),
                );
                let resources = if cfg!(feature = "native-resources") {
                    let ids: Vec<_> = if input {
                        facts
                            .current_grants
                            .iter()
                            .map(|g| g.grant_id.clone())
                            .collect()
                    } else {
                        vec![grant.to_owned()]
                    };
                    ids.into_iter().find_map(|id| {
                        gate.prepare_resource_peer(
                            &id,
                            lease::now().ok()?,
                            Instant::now(),
                            cancelled.clone(),
                        )
                        .ok()
                    })
                } else {
                    None
                };
                if permit.is_none() && resources.is_none() {
                    return Err("not-granted");
                }
                let mut channels = Vec::new();
                if resources.is_some() {
                    for record in &facts.current_grants {
                        if gate
                            .resource_transport_grant(
                                &record.grant_id,
                                lease::now()?,
                                Instant::now(),
                            )
                            .is_ok()
                        {
                            let lane = if record.capability.starts_with("clipboard_") {
                                "clipboard"
                            } else {
                                "file"
                            };
                            if !channels.contains(&lane) {
                                channels.push(lane);
                            }
                        }
                    }
                }
                Ok((permit, resources, cancelled, channels))
            })?;
        let attempt = super::identity::peer_token()?;
        let helper = self
            .gate
            .lock()
            .map_err(|_| "endpoint-failed")?
            .binding()
            .helper_transport
            .clone();
        let input = permit.is_some();
        let resource_channels = resources.is_some();
        *peer = Some(OwnedPeer {
            attempt: attempt.clone(),
            permit,
            resources,
            cancelled,
            sealed: false,
            observed: false,
            ice_requested: false,
            deadline: Instant::now() + Duration::from_secs(15),
        });
        self.peer_live()?;
        Ok(
            serde_json::json!({"attempt":attempt,"helperTransport":helper,"generation":self.generation,
            "resources":resource_channels,
            "resourceChannels":channels,
            "profile":if !input {"resources"} else {match self.profile {InputProfile::Legacy=>"legacy",InputProfile::Profile2=>"input2"}}}),
        )
    }
    fn check_connection(
        &self,
        gate: &mut SessionGate,
        peer: &OwnedPeer,
    ) -> Result<(bool, bool), &'static str> {
        if peer.cancelled.load(Ordering::SeqCst) {
            return Err("stale-peer");
        }
        let input = peer.permit.as_ref().is_some_and(|permit| {
            !self.input_stop.load(Ordering::SeqCst) && self.check_cohort(gate, permit).is_ok()
        });
        if peer.permit.is_some() && !input {
            self.input_revoker.revoke();
        }
        let resources = peer.resources.as_ref().is_some_and(|permit| {
            let Ok(grant) = gate.resource_peer_grant(
                permit,
                peer.observed,
                lease::now().unwrap_or(0),
                Instant::now(),
            ) else {
                return false;
            };
            current_view(&self.facts, &self.expected_track).is_ok_and(|facts| {
                facts
                    .current_grants
                    .iter()
                    .any(|g| g.canonical() == grant.canonical())
            })
        });
        if gate.ended() || (!input && !resources) {
            return Err("revoked");
        }
        Ok((input, resources))
    }

    pub(crate) fn request_ice(
        &self,
        attempt: &str,
        cancelled: Arc<AtomicBool>,
    ) -> Result<serde_json::Value, &'static str> {
        self.peer_live()?;
        let endpoint = {
            let mut peer = self.peer.lock().map_err(|_| "endpoint-failed")?;
            let peer = peer.as_mut().ok_or("stale-peer")?;
            if peer.attempt != attempt
                || peer.sealed
                || peer.ice_requested
                || Instant::now() >= peer.deadline
            {
                return Err("stale-peer");
            }
            with_live_gate(&self.gate, &self.stop, |gate| {
                self.check_connection(gate, peer)?;
                Ok(())
            })?;
            peer.ice_requested = true;
            self.gate
                .lock()
                .map_err(|_| "endpoint-failed")?
                .binding()
                .endpoint
                .clone()
        };
        let revoker = self.peer_revoker(attempt)?;
        let config = self.identity.request_ice(&endpoint, || {
            cancelled.load(Ordering::SeqCst)
                || revoker.cancelled.load(Ordering::SeqCst)
                || self.stop.is_stopped()
        })?;
        self.peer_live()?;
        if cancelled.load(Ordering::SeqCst) {
            return Err("revoked");
        }
        let peer = self.peer.lock().map_err(|_| "endpoint-failed")?;
        let peer = peer
            .as_ref()
            .filter(|peer| {
                peer.attempt == attempt && !peer.sealed && Instant::now() < peer.deadline
            })
            .ok_or("stale-peer")?;
        with_live_gate(&self.gate, &self.stop, |gate| {
            self.check_connection(gate, peer)
        })?;
        if Instant::now() >= peer.deadline || cancelled.load(Ordering::SeqCst) {
            return Err("stale-peer");
        }
        config.rtc()
    }
    pub(crate) fn peer_operation(
        &self,
        attempt: &str,
        own: Option<&str>,
        observed: Option<&str>,
        packet: Option<(&str, &str)>,
    ) -> Result<serde_json::Value, &'static str> {
        self.peer_live()?;
        {
            let mut peer = self.peer.lock().map_err(|_| "endpoint-failed")?;
            let peer = peer.as_mut().ok_or("peer-unverified")?;
            if peer.attempt != attempt || (!peer.observed && Instant::now() >= peer.deadline) {
                return Err("stale-peer");
            }
            with_live_gate(&self.gate, &self.stop, |gate| {
                self.check_connection(gate, peer)
            })?;
            if let Some(own) = own {
                if peer.sealed || peer.observed {
                    return Err("stale-peer");
                }
                let signature = with_live_gate(&self.gate, &self.stop, |gate| {
                    self.check_connection(gate, peer)?;
                    self.identity.transport(gate.binding(), own)
                })?;
                peer.sealed = true;
                self.peer_live()?;
                return Ok(serde_json::json!({"transportSignature":signature}));
            }
            if let Some(fingerprint) = observed {
                if !peer.sealed || peer.observed {
                    return Err("stale-peer");
                }
                with_live_gate(&self.gate, &self.stop, |gate| {
                    let (input, resources) = self.check_connection(gate, peer)?;
                    if let Some(permit) = peer.permit.as_ref().filter(|_| input) {
                        gate.admit_peer(permit, fingerprint, lease::now()?, Instant::now())?;
                    }
                    if let Some(permit) = peer.resources.as_ref().filter(|_| resources) {
                        gate.admit_resource_peer(
                            permit,
                            fingerprint,
                            lease::now()?,
                            Instant::now(),
                        )?;
                    }
                    Ok(())
                })?;
                peer.observed = true;
            }
            if packet.is_some() && peer.permit.is_none() {
                return Err("input-not-ready");
            }
            if packet.is_some() && !peer.observed {
                return Err("peer-unverified");
            }
        }
        self.peer_live()?;
        if let Some((text, stamp)) = packet {
            self.packet_at(text.into(), super::packet_clock::ingress(stamp)?)?;
        }
        self.peer_live()?;
        // Revalidate the exact setup witness after preparation/dispatch waits.
        let peer = self.peer.lock().map_err(|_| "endpoint-failed")?;
        let peer = peer.as_ref().ok_or("peer-unverified")?;
        let (input, resources) = with_live_gate(&self.gate, &self.stop, |gate| {
            self.check_connection(gate, peer)
        })?;
        Ok(if packet.is_none() {
            serde_json::json!({"ready":peer.observed && (resources || (input && self.ready.load(Ordering::SeqCst))),
                "inputAllowed":input, "inputReady":peer.observed && input && self.ready.load(Ordering::SeqCst)})
        } else {
            serde_json::json!({})
        })
    }
    fn packet_at(&self, packet: String, ingress: Instant) -> Result<(), &'static str> {
        self.peer_live()?;
        if packet.len() > 16_384 {
            return Err("too-large");
        }
        if self.stop.is_stopped() {
            return Err("revoked");
        }
        if self.input_stop.load(Ordering::SeqCst) || !self.ready.load(Ordering::SeqCst) {
            return Err("input-not-ready");
        }
        let (send, receive) = mpsc::sync_channel(1);
        self.work
            .try_send(Work::Packet(packet, ingress, send))
            .map_err(|_| "rate-limited")?;
        receive
            .recv_timeout(Duration::from_millis(250))
            .map_err(|_| "endpoint-timeout")?
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
    fn fixture_gate() -> SessionGate {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/assist-control-v2.json"
        ))
        .unwrap();
        let mut binding = fixture["expect"].clone();
        binding.as_object_mut().unwrap().remove("sessionDeadline");
        binding.as_object_mut().unwrap().remove("now");
        binding["endpoint"] = fixture["grants"]["valid"][1]["signed"]["grant"]["endpoint"].clone();
        SessionGate::new(
            [42; 32],
            serde_json::from_value(binding).unwrap(),
            1,
            0,
            Instant::now(),
        )
    }
    #[test]
    fn launch_stop_while_authorization_holds_gate_rejects_success() {
        let launch = Arc::new(AtomicBool::new(false));
        let stop: crate::cancellation::SessionStop = launch.clone().into();
        let gate = Arc::new(Mutex::new(fixture_gate()));
        let (entered, inside) = mpsc::sync_channel(0);
        let (release, released) = mpsc::sync_channel(0);
        let worker_gate = gate.clone();
        let worker = thread::spawn(move || {
            with_live_gate(&worker_gate, &stop, |_| {
                entered.send(()).unwrap();
                released.recv().unwrap();
                Ok(())
            })
        });
        inside.recv_timeout(Duration::from_secs(2)).unwrap();
        launch.store(true, Ordering::SeqCst);
        release.send(()).unwrap();
        assert_eq!(worker.join().unwrap(), Err("revoked"));
        assert!(gate.lock().unwrap().ended());
    }
    #[test]
    fn sibling_runtime_end_never_sets_global_emergency_stop() {
        let launch = Arc::new(AtomicBool::new(false));
        let revoker = |stop| RuntimeRevoker {
            stop,
            input_stop: Arc::new(AtomicBool::new(false)),
            executor_stop: Some(Arc::new(StopEvent::new().unwrap())),
            ready: Arc::new(AtomicBool::new(true)),
            lease_cancel: Arc::new(AtomicBool::new(false)),
            files: None,
        };
        let first = revoker(crate::cancellation::SessionStop::child(launch.clone()));
        let sibling = revoker(crate::cancellation::SessionStop::child(launch.clone()));
        first.revoke();
        assert!(first.stop.is_stopped());
        assert!(!sibling.stop.is_stopped());
        assert!(!sibling.input_stop.load(Ordering::SeqCst));
        assert!(!sibling.lease_cancel.load(Ordering::SeqCst));
        assert!(!launch.load(Ordering::SeqCst));
        launch.store(true, Ordering::SeqCst);
        assert!(sibling.stop.is_stopped());
    }
    #[test]
    fn input_only_revocation_preserves_resource_and_poller_flags() {
        let revoker = RuntimeRevoker {
            stop: Arc::new(AtomicBool::new(false)).into(),
            input_stop: Arc::new(AtomicBool::new(false)),
            executor_stop: Some(Arc::new(StopEvent::new().unwrap())),
            ready: Arc::new(AtomicBool::new(true)),
            lease_cancel: Arc::new(AtomicBool::new(false)),
            files: None,
        };
        revoker.suspend_input();
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{
            Foundation::WAIT_OBJECT_0, System::Threading::WaitForSingleObject,
        };
        let reader = revoker.executor_stop.as_ref().unwrap().reader().unwrap();
        assert_eq!(
            unsafe { WaitForSingleObject(reader.as_raw_handle(), 0) },
            WAIT_OBJECT_0
        );
        assert!(revoker.input_stop.load(Ordering::SeqCst));
        assert!(!revoker.ready.load(Ordering::SeqCst));
        assert!(!revoker.stop.is_stopped());
        assert!(!revoker.lease_cancel.load(Ordering::SeqCst));
        revoker.revoke();
        assert!(revoker.stop.is_stopped());
        assert!(revoker.lease_cancel.load(Ordering::SeqCst));
    }
}
