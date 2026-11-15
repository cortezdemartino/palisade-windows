//! One coordinator-owned endpoint, never one signer or enrollment per request.
//! Public builds stay inactive; qualified sessions retain native source, consent and cleanup ownership.
use super::{
    enrollment::{Enrollment, Offer},
    identity::{EndpointIdentity, Identity},
    issuer::TrustedIssuer,
};
use crate::endpoint::{Answer, Error, Operation};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Default)]
struct SafetyState {
    requests: BTreeMap<u32, GrantRequest>,
    close_revisions: BTreeMap<(u32, String), u64>,
    peer_stops: BTreeMap<(u32, String, String), super::runtime::InputRevoker>,
    extension: Option<(u32, u8)>,
    sessions: BTreeMap<(u32, String), super::runtime::RuntimeRevoker>,
    closed: BTreeSet<(u32, String)>,
    peers: BTreeMap<(u32, String), (super::runtime::PeerRuntime, String)>,
}
#[derive(Clone)]
pub(crate) struct GrantRequest {
    document: u32,
    session: String,
    revision: u64,
    cancelled: Arc<AtomicBool>,
    input: Arc<AtomicBool>,
}
impl crate::cancellation::StopWitness for GrantRequest {
    fn stopped(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}
impl GrantRequest {
    fn check(&self, safety: &Safety, document: u32, session: &str) -> Result<(), Error> {
        if self.cancelled.load(Ordering::SeqCst)
            || self.document != document
            || self.session != session
            || (self.input.load(Ordering::SeqCst)
                && safety
                    .revision(document, session)
                    .map_err(|_| Error::Stopped)?
                    != self.revision)
        {
            self.cancelled.store(true, Ordering::SeqCst);
            return Err(Error::Cancelled);
        }
        Ok(())
    }
    pub(crate) fn flag(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}
#[derive(Clone, Default)]
pub(crate) struct Safety(Arc<Mutex<SafetyState>>);
impl Safety {
    pub(crate) fn register_request(
        &self,
        sequence: u32,
        document: u32,
        session: &str,
    ) -> Result<GrantRequest, &'static str> {
        if !crate::grant::token(session, 8, 128) {
            return Err("invalid-session");
        }
        let mut state = self.0.lock().map_err(|_| "endpoint-safety")?;
        if state.requests.len() >= 4096 || state.requests.contains_key(&sequence) {
            return Err("request-scope-full");
        }
        let request = GrantRequest {
            document,
            session: session.into(),
            revision: *state
                .close_revisions
                .get(&(document, session.into()))
                .unwrap_or(&0),
            cancelled: Arc::new(AtomicBool::new(false)),
            input: Arc::new(AtomicBool::new(false)),
        };
        state.requests.insert(sequence, request.clone());
        Ok(request)
    }
    pub(crate) fn cancel_request(&self, target: u32) -> Result<bool, &'static str> {
        let request = self
            .0
            .lock()
            .map_err(|_| "endpoint-safety")?
            .requests
            .get(&target)
            .cloned();
        if let Some(request) = request {
            request.cancelled.store(true, Ordering::SeqCst);
            return Ok(true);
        }
        // Single ordered ingress registers a grant before accepting any later
        // cancellation frame. Unknown/non-grant IDs have no authority to cancel.
        Ok(false)
    }
    fn revision(&self, document: u32, session: &str) -> Result<u64, &'static str> {
        Ok(*self
            .0
            .lock()
            .map_err(|_| "endpoint-safety")?
            .close_revisions
            .get(&(document, session.to_owned()))
            .unwrap_or(&0))
    }
    fn replace_input(
        &self,
        document: u32,
        session: &str,
        runtime: &super::runtime::StreamRuntime,
        source: String,
        expected: u64,
    ) -> Result<(), &'static str> {
        let mut state = self.0.lock().map_err(|_| "endpoint-safety")?;
        let key = (document, session.to_owned());
        if !state.sessions.contains_key(&key)
            || *state.close_revisions.get(&key).unwrap_or(&0) != expected
        {
            runtime.revoker().suspend_input();
            return Err("revoked");
        }
        state
            .peers
            .insert(key.clone(), (runtime.peer_handle()?, source));
        state.sessions.insert(key.clone(), runtime.revoker());
        state.closed.remove(&key);
        Ok(())
    }
    pub(crate) fn close_peer(&self, document: u32, session: &str, attempt: &str) -> Answer {
        let result = (|| {
            if !crate::grant::token(session, 8, 128) || crate::grant::decode::<32>(attempt).is_err()
            {
                return Err(Error::InvalidMessage);
            }
            let revoker = self
                .0
                .lock()
                .map_err(|_| Error::Internal)?
                .peer_stops
                .get(&(document, session.to_owned(), attempt.to_owned()))
                .cloned()
                .ok_or(Error::StaleOperation)?;
            revoker.revoke();
            Ok(json!({}))
        })();
        match result {
            Ok(result) => Answer::ok(result),
            Err(code) => Answer::error(code),
        }
    }
    pub(crate) fn extension_safety(
        &self,
        document: u32,
        version: u8,
        operation: &crate::extension::Operation,
    ) -> crate::extension::Answer {
        use crate::extension::{Answer, Error};
        Answer::from_result((|| {
            operation.validate(version)?;
            if !operation.safety() {
                return Err(Error::InvalidMessage);
            }
            if self.0.lock().map_err(|_| Error::Internal)?.extension != Some((document, version)) {
                return Err(Error::UnsupportedVersion);
            }
            // Resource execution remains unavailable until the native brokers
            // and bound encrypted channels are connected. No activity is implied.
            Err(Error::NotActive)
        })())
    }
    fn attach_peer(
        &self,
        document: u32,
        session: &str,
        runtime: super::runtime::PeerRuntime,
        source_operation: String,
    ) -> Result<(), &'static str> {
        let mut state = self.0.lock().map_err(|_| "endpoint-safety")?;
        let key = (document, session.to_owned());
        if !state.sessions.contains_key(&key)
            || state.peers.len() >= 16
            || state.peers.contains_key(&key)
        {
            return Err("endpoint-safety-full");
        }
        state.peers.insert(key, (runtime, source_operation));
        Ok(())
    }
    pub(crate) fn peer(&self, document: u32, operation: crate::peer::Operation) -> Answer {
        let result = (|| {
            if !cfg!(feature = "native-input") {
                return Err(Error::TransportMismatch);
            }
            let (runtime, source_operation) = self
                .0
                .lock()
                .map_err(|_| Error::Internal)?
                .peers
                .get(&(document, operation.session().to_owned()))
                .cloned()
                .ok_or(Error::BindingMismatch)?;
            use crate::peer::Operation as Peer;
            let result = match operation {
                Peer::Inspect { grant, .. } => runtime.inspect_grant(&grant),
                Peer::GrantState { grant_id, .. } => runtime.grant_state(&grant_id),
                Peer::Prepare { grant_id, session } => {
                    let mut value = runtime.prepare_peer(&grant_id).map_err(map_runtime_error)?;
                    let attempt = value["attempt"].as_str().ok_or(Error::Internal)?.to_owned();
                    let mut state = self.0.lock().map_err(|_| Error::Internal)?;
                    if state.peer_stops.len() >= 128 {
                        runtime.input_revoker().revoke();
                        return Err(Error::Busy);
                    }
                    state
                        .peer_stops
                        .insert((document, session, attempt), runtime.input_revoker());
                    value["sourceOperation"] = source_operation.into();
                    Ok(value)
                }
                Peer::Close { .. } => return Err(Error::InvalidMessage),
                Peer::Seal {
                    attempt,
                    own_fingerprint,
                    ..
                } => runtime.peer_operation(&attempt, Some(&own_fingerprint), None, None),
                Peer::Observed {
                    attempt,
                    fingerprint,
                    ..
                } => runtime.peer_operation(&attempt, None, Some(&fingerprint), None),
                Peer::Check { attempt, .. } => runtime.peer_operation(&attempt, None, None, None),
                Peer::Packet {
                    attempt,
                    text,
                    ingress_ns,
                    ..
                } => runtime.peer_operation(&attempt, None, None, Some((&text, &ingress_ns))),
            };
            result.map_err(|error| match error {
                "stale-peer" | "peer-already-bound" => Error::StaleOperation,
                "peer-unverified" | "invalid-statement" | "invalid-input-clock" => {
                    Error::TransportMismatch
                }
                "input-not-ready" | "consent-pending" => Error::Busy,
                _ => map_runtime_error(error),
            })
        })();
        match result {
            Ok(result) => Answer::ok(result),
            Err(error) => Answer::error(error),
        }
    }
    pub(crate) fn close_input(&self, document: u32, session: &str) -> Result<(), &'static str> {
        if !crate::grant::token(session, 8, 128) {
            return Err("invalid-session");
        }
        let key = (document, session.to_owned());
        let revoker = {
            let mut state = self.0.lock().map_err(|_| "endpoint-safety")?;
            if state.closed.len() >= 128 && !state.closed.contains(&key) {
                return Err("endpoint-safety-full");
            }
            state.closed.insert(key.clone());
            let revision = state.close_revisions.entry(key.clone()).or_insert(0);
            *revision = revision.checked_add(1).ok_or("stop-revision-exhausted")?;
            state.sessions.get(&key).cloned()
        };
        if let Some(revoker) = revoker {
            revoker.suspend_input();
        }
        Ok(())
    }
    fn register(
        &self,
        document: u32,
        session: &str,
        revoker: super::runtime::RuntimeRevoker,
    ) -> Result<(), &'static str> {
        let mut state = self.0.lock().map_err(|_| "endpoint-safety")?;
        let key = (document, session.to_owned());
        if state.closed.contains(&key) {
            revoker.suspend_input();
        }
        if state.sessions.contains_key(&key) {
            return Err("repeated-session");
        }
        state.sessions.insert(key, revoker);
        Ok(())
    }
}
struct Session {
    source: super::capture::Binding,
    binding: crate::stream::Binding,
    runtime: super::runtime::StreamRuntime,
    offered: BTreeSet<String>,
}

pub struct Endpoint {
    extension_version: Option<u8>,
    document: Option<u32>,
    negotiated: bool,
    probe: bool,
    cancelled: Arc<AtomicBool>,
    identity: Option<Arc<EndpointIdentity>>,
    issuer: Option<TrustedIssuer>,
    enrollment: Option<Enrollment>,
    attempted: BTreeSet<String>,
    proof_prepared: bool,
    custody: Option<Arc<super::process_input::custody::Client>>,
    probe_executor: Option<super::process_input::ProcessInput>,
    probe_generation: u64,
    stop: crate::cancellation::SessionStop,
    capture_configured: bool,
    capture: Option<super::capture::Capture>,
    sessions: BTreeMap<String, Session>,
    safety: Safety,
    cleanup_failed: bool,
}
impl Endpoint {
    fn qualified_input(&self) -> bool {
        !self.probe
            && cfg!(feature = "native-input")
            && crate::qualification::namespace().ok().flatten().is_some()
    }
    fn input_profile(&self) -> super::runtime::InputProfile {
        if self.qualified_input() && self.extension_version.is_some_and(|v| v >= 2) {
            super::runtime::InputProfile::Profile2
        } else {
            super::runtime::InputProfile::Legacy
        }
    }
    pub fn new(probe: bool, cancelled: Arc<AtomicBool>) -> Self {
        Self::with_custody(probe, cancelled, None)
    }
    pub(crate) fn with_custody(
        probe: bool,
        cancelled: Arc<AtomicBool>,
        custody: Option<Arc<super::process_input::custody::Client>>,
    ) -> Self {
        let stop = cancelled.clone().into();
        Self {
            extension_version: None,
            document: None,
            negotiated: false,
            probe,
            cancelled,
            identity: None,
            issuer: None,
            enrollment: None,
            attempted: BTreeSet::new(),
            proof_prepared: false,
            custody,
            probe_executor: None,
            probe_generation: 0,
            stop,
            capture_configured: false,
            capture: None,
            sessions: BTreeMap::new(),
            safety: Safety::default(),
            cleanup_failed: false,
        }
    }
    pub(crate) fn monitored(mut self, stop: crate::cancellation::SessionStop) -> Self {
        self.stop = stop;
        self
    }
    pub(crate) fn safety(&self) -> Safety {
        self.safety.clone()
    }
    pub(crate) fn poll(&mut self) {
        if let Some(capture) = &mut self.capture {
            capture.harvest();
        }
        for session in self.sessions.values_mut() {
            session.runtime.harvest_prompts();
        }
    }
    pub(crate) fn configure_capture(&mut self, enabled: bool) -> bool {
        if self.capture_configured || self.document.is_some() || self.live().is_err() {
            return self.capture.is_some();
        }
        self.capture_configured = true;
        if enabled && !self.probe && crate::qualification::namespace().ok().flatten().is_some() {
            self.capture = self
                .custody
                .as_ref()
                .map(|client| super::capture::Capture::new(client.clone(), self.stop.clone()));
        }
        self.capture.is_some()
    }
    pub(crate) fn select_capture(
        &mut self,
        document: u32,
        selection: crate::capture::Selection,
    ) -> bool {
        self.live().is_ok()
            && self.document == Some(document)
            && self
                .capture
                .as_mut()
                .is_some_and(|capture| capture.selected(document, selection).is_ok())
    }
    fn live(&self) -> Result<(), Error> {
        if self.stop.is_stopped() || self.cancelled.load(Ordering::SeqCst) {
            Err(Error::Stopped)
        } else {
            Ok(())
        }
    }
    pub fn dispatch(&mut self, document: u32, operation: Operation) -> Answer {
        self.dispatch_request(document, operation, None)
    }
    pub(crate) fn dispatch_request(
        &mut self,
        document: u32,
        operation: Operation,
        request: Option<&GrantRequest>,
    ) -> Answer {
        let result = self.execute(document, operation, request);
        // A late dialog/network completion cannot commit a current enrollment.
        if self.live().is_err() {
            self.revoke();
            return Answer::error(Error::Stopped);
        }
        match result {
            Ok(value) => Answer::ok(value),
            Err(code) => Answer::error(code),
        }
    }
    pub(crate) fn extension(
        &mut self,
        document: u32,
        version: u8,
        operation: crate::extension::Operation,
    ) -> crate::extension::Answer {
        use crate::extension::{Answer, Error, Operation as Ext};
        Answer::from_result((|| {
            self.live().map_err(|_| Error::Stopped)?;
            if self.document != Some(document) || !self.negotiated {
                return Err(Error::UnsupportedVersion);
            }
            operation.validate(version)?;
            if let Ext::Hello { versions } = operation {
                let chosen = match self.extension_version {
                    Some(current) if versions.contains(&u64::from(current)) => current,
                    Some(_) => return Err(Error::UnsupportedVersion),
                    None if !self.sessions.is_empty() => return Err(Error::Busy),
                    None => [3u8, 2, 1]
                        .into_iter()
                        .find(|v| versions.contains(&u64::from(*v)))
                        .ok_or(Error::UnsupportedVersion)?,
                };
                self.extension_version = Some(chosen);
                self.safety.0.lock().map_err(|_| Error::Internal)?.extension =
                    Some((document, chosen));
                let mut features =
                    json!({"cleanupEvents":false,"clipboardText":false,"files":false});
                if chosen >= 2 {
                    features = json!({"cleanupEvents":false,"clipboardImage":false,"clipboardSync":false,"clipboardText":false,"dropSurface":false,"fileResume":false,"files":false,"input2":false,"multiFile":false});
                    features["input2"] = self.qualified_input().into();
                }
                if chosen == 3 {
                    features["localYield"] = false.into();
                }
                return Ok(json!({"version":chosen,"features":features}));
            }
            if self.extension_version != Some(version) {
                return Err(Error::UnsupportedVersion);
            }
            Err(Error::Unsupported)
        })())
    }
    fn execute(
        &mut self,
        document: u32,
        operation: Operation,
        request: Option<&GrantRequest>,
    ) -> Result<serde_json::Value, Error> {
        self.live()?;
        match self.document {
            None => self.document = Some(document),
            Some(current) if current != document => {
                self.revoke();
                return Err(Error::StaleOperation);
            }
            _ => (),
        }
        if let Operation::Hello { versions } = operation {
            if versions.is_empty()
                || versions.len() > 4
                || versions
                    .iter()
                    .any(|v| *v == 0 || *v > 9_007_199_254_740_991)
            {
                return Err(Error::InvalidMessage);
            }
            if !versions.contains(&1) {
                return Err(Error::UnsupportedVersion);
            }
            // Public hello is descriptive and idempotent for the same owned
            // document. Reconnecting a web client never replaces the signer,
            // enrollment or replay tombstones. The private seal remains once-only.
            self.negotiated = true;
            return Ok(
                json!({"version":1,"features":{"capture":self.capture.is_some(),"clipboard":false,"files":false,"input":self.qualified_input()}}),
            );
        }
        if !self.negotiated {
            return Err(Error::InvalidMessage);
        }
        // Never enter HTTPS or another consent worker while the native source
        // picker owns its exclusion guard. Cancellation can then retire that
        // guard through the idle reap without waiting for enrollment I/O.
        if matches!(
            operation,
            Operation::EnrollKey { .. } | Operation::EnrollProve { .. }
        ) && self
            .capture
            .as_mut()
            .is_some_and(|capture| capture.picking())
        {
            return Err(Error::Busy);
        }
        match operation {
            Operation::EnrollKey { origin } => {
                if origin != crate::issuer::ORIGIN {
                    return Err(Error::BindingMismatch);
                }
                if self.probe {
                    return Err(Error::NotEnrolled);
                }
                if self.issuer.is_none() {
                    let issuer = TrustedIssuer::confirm_current(self.cancelled.clone())
                        .map_err(map_error)?;
                    self.live()?;
                    let installation = Identity::load_current_user().map_err(map_error)?;
                    let identity =
                        EndpointIdentity::for_launch(&installation).map_err(map_error)?;
                    self.live()?;
                    self.identity = Some(Arc::new(identity));
                    self.issuer = Some(issuer);
                }
                let issuer = self.issuer.as_ref().ok_or(Error::NotEnrolled)?;
                let identity = self.identity.as_ref().ok_or(Error::NotEnrolled)?;
                Ok(
                    json!({"publicKey":identity.public_key(),"issuerFingerprint":issuer.fingerprint(),"pin":issuer.method()}),
                )
            }
            Operation::EnrollProve {
                endpoint_id,
                challenge,
                account,
                device,
            } => {
                let issuer = self.issuer.as_ref().ok_or(Error::NotEnrolled)?;
                let identity = self.identity.as_ref().ok_or(Error::NotEnrolled)?;
                // Retain attempted endpoint IDs for the launch, including denial
                // and cancelled requests. A page cannot replace a confirmed tuple.
                if self.enrollment.is_some()
                    || self.attempted.len() >= 32
                    || !self.attempted.insert(endpoint_id.clone())
                {
                    return Err(Error::StaleOperation);
                }
                let (enrollment, signature) = Enrollment::request(
                    issuer,
                    identity,
                    &Offer {
                        endpoint: &endpoint_id,
                        challenge: &challenge,
                        account: &account,
                        device: &device,
                    },
                    self.cancelled.clone(),
                )
                .map_err(map_error)?;
                self.live()?;
                self.enrollment = Some(enrollment);
                self.proof_prepared = true;
                Ok(json!({"signature":signature}))
            }
            Operation::Status {} => Ok(json!({
                // Local confirmation only. Web endpoint.proven independently
                // establishes service acceptance; neither creates authority.
                "endpoint":{"enrolled":self.proof_prepared,"endpointId":self.enrollment.as_ref().map(Enrollment::endpoint),"pin":self.issuer.as_ref().map(|i| match i.method() {crate::issuer::Method::Release => "release",crate::issuer::Method::Confirmed => "confirmed"}).unwrap_or("none")},
                "capture":self.capture.as_mut().map(|capture|capture.status()).unwrap_or_else(||json!({"operation":null,"state":"idle","trackSid":null})),
                "controller":{"session":null,"state":"idle"}
            })),
            Operation::GrantOffer { session, grant } => {
                self.offer(document, &session, &grant, request)
            }
            Operation::PeerOffer { .. } | Operation::PeerCandidate { .. } => {
                Err(Error::TransportMismatch)
            }
            Operation::PeerClose { .. } => Ok(json!({})),
            Operation::CaptureStart { operation, room } => {
                self.capture
                    .as_mut()
                    .ok_or(Error::CaptureUnavailable)?
                    .begin(document, &operation, &room)
                    .map_err(map_capture_error)?;
                Ok(json!({"operation":operation}))
            }
            Operation::CaptureBind {
                operation,
                track_sid,
            } => {
                self.capture
                    .as_mut()
                    .ok_or(Error::CaptureUnavailable)?
                    .bind(document, &operation, &track_sid)
                    .map_err(map_capture_error)?;
                Ok(json!({}))
            }
            Operation::CaptureCancel { operation } | Operation::CaptureStop { operation } => {
                if let Some(capture) = &mut self.capture {
                    capture.stop(document, &operation);
                }
                Ok(json!({}))
            }
            Operation::Hello { .. } => unreachable!(),
        }
    }
    pub fn revoke(&mut self) {
        self.cancelled.store(true, Ordering::SeqCst);
        for session in self.sessions.values() {
            session.runtime.revoker().revoke();
        }
        if let Some(capture) = &mut self.capture {
            capture.end();
        }
        drop(self.probe_executor.take());
        if let Some(custody) = self.custody.take() {
            let _ = custody.seal();
        }
        if let Some(record) = self.enrollment.take() {
            record.revoke();
        }
        self.proof_prepared = false;
        self.identity = None;
        self.issuer = None;
    }
    pub(crate) fn cleanup(&mut self) -> Result<(), &'static str> {
        self.revoke();
        for session in self.sessions.values_mut() {
            if crate::cleanup::attempt(|| session.runtime.stop()).is_err() {
                self.cleanup_failed = true;
            }
        }
        if self.cleanup_failed {
            Err("endpoint-cleanup-failed")
        } else {
            Ok(())
        }
    }
    fn offer(
        &mut self,
        document: u32,
        session: &str,
        encoded: &str,
        request: Option<&GrantRequest>,
    ) -> Result<serde_json::Value, Error> {
        use super::enrollment::{EnrolledIdentity, PublishedSource};
        let enrollment = self.enrollment.as_ref().ok_or(Error::NotEnrolled)?;
        let identity = self.identity.as_ref().ok_or(Error::NotEnrolled)?;
        let issuer = self.issuer.as_ref().ok_or(Error::NotEnrolled)?;
        let request = request.ok_or(Error::InvalidMessage)?;
        if !crate::grant::token(session, 8, 128) || encoded.len() > 4096 {
            return Err(Error::GrantInvalid);
        }
        let signed: crate::stream::SignedStreamGrant =
            serde_json::from_str(encoded).map_err(|_| Error::GrantInvalid)?;
        let source = self
            .capture
            .as_mut()
            .ok_or(Error::CaptureUnavailable)?
            .binding()
            .map_err(map_capture_error)?;
        let binding = enrollment
            .native_binding(
                issuer,
                identity,
                session,
                PublishedSource {
                    room: &source.room,
                    stream: &source.stream,
                },
                &signed,
                super::lease::now().map_err(map_error)?,
            )
            .map_err(|_| Error::GrantInvalid)?;
        if !source.live() {
            return Err(Error::CaptureUnavailable);
        }
        request.input.store(
            matches!(
                signed.grant.capability.as_str(),
                "control_mouse" | "control_keyboard"
            ),
            Ordering::SeqCst,
        );
        let id = signed.grant.grant_id.clone();
        if let Some(previous) = self.sessions.get(session) {
            if previous.binding != binding
                || previous.source.key != source.key
                || previous.runtime.ended()
            {
                return Err(Error::StaleOperation);
            }
        } else {
            // Retain ended sessions as tombstones; never reconstruct their gate.
            if self.sessions.len() >= 16 {
                return Err(Error::Busy);
            }
            let runtime = super::runtime::StreamRuntime::begin_with_profile(
                EnrolledIdentity {
                    issuer,
                    identity: identity.clone(),
                    enrollment,
                },
                binding.clone(),
                std::slice::from_ref(&signed),
                source.geometry.clone(),
                super::runtime::RuntimeOwner {
                    stop: self.stop.clone(),
                    custody: self.custody.as_ref().ok_or(Error::Stopped)?.clone(),
                    source: source.clone(),
                },
                self.input_profile(),
            )
            .map_err(map_runtime_error)?;
            self.safety
                .register(document, session, runtime.revoker())
                .map_err(|_| Error::Stopped)?;
            self.safety
                .attach_peer(
                    document,
                    session,
                    runtime.peer_handle().map_err(map_runtime_error)?,
                    source.key.operation.clone(),
                )
                .map_err(|_| Error::Stopped)?;
            self.sessions.insert(
                session.into(),
                Session {
                    source,
                    binding,
                    runtime,
                    offered: BTreeSet::new(),
                },
            );
        }
        let revision = request.revision;
        let entry = self.sessions.get_mut(session).ok_or(Error::Stopped)?;
        if let Err(error) = request.check(&self.safety, document, session) {
            entry
                .runtime
                .reject_offer(&signed)
                .map_err(map_runtime_error)?;
            return Err(error);
        }
        if entry.offered.contains(&id) {
            // A repeated ID is not sufficient: verify the entire signed record
            // before reusing an acknowledgement for the current native consent.
            if entry.runtime.admit(&signed).map_err(map_runtime_error)? {
                return Err(Error::StaleOperation);
            }
            let signature = entry
                .runtime
                .current_acknowledgement(&id)
                .map_err(map_runtime_error)?
                .ok_or(Error::StaleOperation)?;
            request.check(&self.safety, document, session)?;
            return Ok(json!({"signature":signature}));
        }
        // Bind cancellation before any cleanup wait or admission. A timed-out
        // pre-admission offer cannot later adopt a different RPC's owner flag.
        entry
            .runtime
            .bind_offer(&signed, request.flag())
            .map_err(map_runtime_error)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let replaced = loop {
            if let Err(error) = request.check(&self.safety, document, session) {
                entry
                    .runtime
                    .reject_offer(&signed)
                    .map_err(map_runtime_error)?;
                return Err(error);
            }
            match entry.runtime.admit(&signed) {
                Err("cleanup-pending")
                    if std::time::Instant::now() < deadline
                        && !entry.runtime.ended()
                        && entry.source.live() =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10))
                }
                result => break result.map_err(map_runtime_error)?,
            }
        };
        if replaced {
            self.safety
                .replace_input(
                    document,
                    session,
                    &entry.runtime,
                    entry.source.key.operation.clone(),
                    revision,
                )
                .map_err(|_| Error::Stopped)?;
        }
        entry
            .runtime
            .bind_operation(&id, request.flag())
            .map_err(map_runtime_error)?;
        request.check(&self.safety, document, session)?;
        entry.offered.insert(id.clone());
        while !entry
            .runtime
            .grant_observed(&id)
            .map_err(map_runtime_error)?
        {
            request.check(&self.safety, document, session)?;
            if self.stop.is_stopped() || entry.runtime.ended() {
                return Err(Error::Stopped);
            }
            if std::time::Instant::now() >= deadline {
                request.cancel();
                return Err(Error::Timeout);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        entry
            .runtime
            .request_consent(&id, request)
            .map_err(map_runtime_error)?;
        loop {
            request.check(&self.safety, document, session)?;
            if self.stop.is_stopped()
                || !entry.source.live()
                || !entry
                    .runtime
                    .source_confirmed()
                    .map_err(map_runtime_error)?
            {
                entry.runtime.revoker().revoke();
                return Err(Error::Stopped);
            }
            let result = entry
                .runtime
                .consent_result(&id)
                .map_err(map_runtime_error)?;
            match result.state {
                "consented" => {
                    request.check(&self.safety, document, session)?;
                    return Ok(json!({"signature": result.signature.ok_or(Error::GrantInvalid)?}));
                }
                "consent-denied" => return Err(Error::ConsentDenied),
                _ if std::time::Instant::now() >= deadline => {
                    request.cancel();
                    return Err(Error::Timeout);
                }
                _ => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }
    }
    /// Private no-authority process fixture. No canonical web operation routes
    /// here; the monitor fixes Probe mode independently of this request.
    pub(crate) fn probe_executor(&mut self, release: bool) -> Result<u32, &'static str> {
        use crate::executor::{Backend, Source};
        if !self.probe {
            return Err("operation-unavailable");
        }
        if release {
            let input = self.probe_executor.take().ok_or("probe-not-held")?;
            let pid = input.pid();
            drop(input);
            return Ok(pid);
        }
        if self.probe_executor.is_some() {
            return Err("probe-held");
        }
        let client = self.custody.as_ref().ok_or("custody-required")?;
        if !client.probe() {
            return Err("probe-required");
        }
        self.probe_generation = self
            .probe_generation
            .checked_add(1)
            .ok_or("probe-exhausted")?;
        let source = Source {
            x: -100,
            y: 0,
            width: 100,
            height: 100,
            generation: 1,
        };
        let mut input = super::process_input::ProcessInput::spawn_monitored(
            client,
            super::process_input::custody::Registration {
                session: "session_monitor_probe".into(),
                slot: 1,
                execution: self.probe_generation,
                source: source.clone(),
                source_key: None,
            },
            123,
            0,
            Arc::new(super::process_tree::StopEvent::new()?),
        )?;
        let mut ledger = crate::input::Ledger::default();
        let action = crate::input::Action::Key {
            code: "ShiftLeft".into(),
            down: true,
        };
        ledger.plan(&action)?;
        input.intent(&ledger)?;
        input.apply(&source, &action)?;
        let pid = input.pid();
        self.probe_executor = Some(input);
        Ok(pid)
    }
    pub(crate) fn probe_cancel_executor(&mut self) -> Result<u32, &'static str> {
        if !self.probe || self.probe_executor.is_some() {
            return Err("probe-required");
        }
        self.probe_generation = self
            .probe_generation
            .checked_add(1)
            .ok_or("probe-exhausted")?;
        super::process_input::ProcessInput::probe_cancel_startup(
            self.custody.as_ref().ok_or("custody-required")?,
            super::process_input::custody::Registration {
                session: "session_monitor_probe".into(),
                slot: 1,
                execution: self.probe_generation,
                source: crate::executor::Source {
                    x: -100,
                    y: 0,
                    width: 100,
                    height: 100,
                    generation: 1,
                },
                source_key: None,
            },
        )
    }
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        self.revoke();
    }
}
fn map_runtime_error(error: &'static str) -> Error {
    match error {
        "revoked" | "expired" => Error::Stopped,
        "controller-busy"
        | "consent-unavailable"
        | "operation-pending"
        | "consent-pending"
        | "cleanup-pending" => Error::Busy,
        "consent-denied" => Error::ConsentDenied,
        "endpoint-disabled"
        | "native-input-disabled"
        | "native-resources-disabled"
        | "source-mismatch"
        | "source-unconfirmed" => Error::CaptureUnavailable,
        _ => Error::GrantInvalid,
    }
}
fn map_capture_error(error: &'static str) -> Error {
    match error {
        "source-busy" | "input-busy" => Error::Busy,
        "stale-source" | "source-ended" | "revoked" => Error::StaleOperation,
        "invalid-source" => Error::InvalidMessage,
        _ => Error::CaptureUnavailable,
    }
}
fn map_error(error: &'static str) -> Error {
    match error {
        "cancelled" | "revoked" => Error::Cancelled,
        "issuer-changed" | "issuer-unqualified" | "invalid-issuer" | "issuer-corrupt" => {
            Error::IssuerMismatch
        }
        "issuer-not-approved" | "consent-denied" => Error::ConsentDenied,
        "binding-mismatch" => Error::BindingMismatch,
        "invalid-endpoint" | "invalid-identity" | "unsafe-identity" | "invalid-encoding" => {
            Error::InvalidMessage
        }
        _ => Error::Internal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn endpoint() -> Endpoint {
        Endpoint::new(true, Arc::new(AtomicBool::new(false)))
    }
    #[test]
    fn queued_input_requests_keep_the_ingress_stop_boundary_but_resources_are_independent() {
        let safety = Safety::default();
        let input = safety.register_request(1, 4, "session_fixture_01").unwrap();
        let resource = safety.register_request(2, 4, "session_fixture_01").unwrap();
        input.input.store(true, Ordering::SeqCst);
        safety.close_input(4, "session_fixture_01").unwrap();
        assert!(matches!(
            input.check(&safety, 4, "session_fixture_01"),
            Err(Error::Cancelled)
        ));
        assert!(input.flag().load(Ordering::SeqCst));
        assert!(resource.check(&safety, 4, "session_fixture_01").is_ok());
        let fresh = safety.register_request(3, 4, "session_fixture_01").unwrap();
        fresh.input.store(true, Ordering::SeqCst);
        assert!(fresh.check(&safety, 4, "session_fixture_01").is_ok());
        assert!(safety.cancel_request(2).unwrap());
        assert!(matches!(
            resource.check(&safety, 4, "session_fixture_01"),
            Err(Error::Cancelled)
        ));
        assert!(fresh.check(&safety, 4, "session_fixture_01").is_ok());
        assert!(!safety.cancel_request(999).unwrap());
        assert!(safety.register_request(3, 4, "session_fixture_01").is_err());
        safety.cancel_request(1).unwrap();
        assert!(fresh.check(&safety, 4, "session_fixture_01").is_ok());
    }
    #[test]
    fn retired_transport_can_only_signal_its_own_generation_and_host_stop_still_targets_current() {
        let safety = Safety::default();
        let old = super::super::runtime::RuntimeRevoker::no_authority();
        let current = super::super::runtime::RuntimeRevoker::no_authority();
        let attempt = "A".repeat(43);
        safety
            .register(4, "session_fixture_01", current.clone())
            .unwrap();
        safety.0.lock().unwrap().peer_stops.insert(
            (4, "session_fixture_01".into(), attempt.clone()),
            old.input_revoker(),
        );
        assert!(matches!(
            safety.close_peer(4, "session_fixture_01", &attempt),
            Answer::Ok { .. }
        ));
        assert!(old.input_stopped());
        assert!(!current.input_stopped());
        assert!(!old.session_stopped());
        assert!(matches!(
            safety.close_peer(5, "session_fixture_01", &attempt),
            Answer::Error {
                code: Error::StaleOperation
            }
        ));
        assert_eq!(safety.revision(4, "session_fixture_01").unwrap(), 0);
        safety.close_input(4, "session_fixture_01").unwrap();
        assert!(current.input_stopped());
        assert!(!current.session_stopped());
        assert_eq!(safety.revision(4, "session_fixture_01").unwrap(), 1);
        safety.close_input(4, "session_fixture_01").unwrap();
        assert_eq!(safety.revision(4, "session_fixture_01").unwrap(), 2);
    }
    #[test]
    fn native_extension_negotiation_is_offered_document_bound_and_never_advertises_unqualified_features()
     {
        use crate::extension::{Answer as ExtAnswer, Error as ExtError, Operation as Ext};
        for offered in [vec![1], vec![2, 1], vec![3, 2, 1]] {
            let mut e = endpoint();
            e.dispatch(4, hello());
            let expected = *offered.iter().max().unwrap();
            let answer = e.extension(
                4,
                1,
                Ext::Hello {
                    versions: offered.clone(),
                },
            );
            let ExtAnswer::Ok { result } = answer else {
                panic!("extension negotiation refused");
            };
            assert_eq!(result["version"], expected);
            assert!(
                result["features"]
                    .as_object()
                    .unwrap()
                    .values()
                    .all(|v| *v == false)
            );
            assert!(matches!(
                e.input_profile(),
                super::super::runtime::InputProfile::Legacy
            ));
            assert!(matches!(
                e.extension(4, 1, Ext::Hello { versions: vec![4] }),
                ExtAnswer::Error {
                    code: ExtError::UnsupportedVersion
                }
            ));
            assert!(matches!(
                e.extension(
                    5,
                    1,
                    Ext::Hello {
                        versions: offered.clone()
                    }
                ),
                ExtAnswer::Error {
                    code: ExtError::UnsupportedVersion
                }
            ));
            assert!(matches!(
                e.safety.extension_safety(
                    4,
                    expected as u8,
                    &Ext::Cancel {
                        session: "session_01".into(),
                        transfer: "transfer_001".into()
                    }
                ),
                ExtAnswer::Error {
                    code: ExtError::NotActive
                }
            ));
            e.revoke();
            assert!(matches!(
                e.extension(4, 1, Ext::Hello { versions: offered }),
                ExtAnswer::Error {
                    code: ExtError::Stopped
                }
            ));
        }
    }
    #[test]
    fn urgent_peer_close_before_registration_is_retained_without_ending_resources_or_other_documents()
     {
        let safety = Safety::default();
        safety.close_input(4, "session_fixture_01").unwrap();
        let first = super::super::runtime::RuntimeRevoker::no_authority();
        let other = super::super::runtime::RuntimeRevoker::no_authority();
        safety
            .register(4, "session_fixture_01", first.clone())
            .unwrap();
        safety
            .register(5, "session_fixture_01", other.clone())
            .unwrap();
        assert!(first.input_stopped());
        assert!(!first.session_stopped());
        assert!(!other.input_stopped());
        safety.close_input(5, "session_fixture_01").unwrap();
        assert!(other.input_stopped());
        assert!(!other.session_stopped());
        safety.close_input(4, "session_fixture_01").unwrap();
        assert!(safety.close_input(4, "../bad").is_err());
    }
    #[test]
    fn private_capture_setup_cannot_activate_probe_default_or_late_document_capabilities() {
        let mut e = endpoint();
        assert!(!e.configure_capture(true));
        assert!(!e.configure_capture(true));
        let result = e.dispatch(0, hello());
        match result {
            Answer::Ok { result } => assert_eq!(result["features"]["capture"], false),
            _ => panic!("hello refused"),
        }
        assert!(!e.configure_capture(true));
        assert!(matches!(
            e.dispatch(
                0,
                Operation::GrantOffer {
                    session: "session_fixture_01".into(),
                    grant: "{}".into()
                }
            ),
            Answer::Error {
                code: Error::NotEnrolled
            }
        ));
    }
    fn hello() -> Operation {
        Operation::Hello { versions: vec![1] }
    }
    #[test]
    fn native_version_negotiation_retains_canonical_safe_integer_range() {
        let mut e = endpoint();
        assert!(matches!(
            e.dispatch(
                0,
                Operation::Hello {
                    versions: vec![9_007_199_254_740_991, 1]
                }
            ),
            Answer::Ok { .. }
        ));
        let mut e = endpoint();
        assert!(matches!(
            e.dispatch(
                0,
                Operation::Hello {
                    versions: vec![9_007_199_254_740_992, 1]
                }
            ),
            Answer::Error {
                code: Error::InvalidMessage
            }
        ));
    }
    #[test]
    fn negotiation_and_document_replacement_are_native_enforced() {
        let mut e = endpoint();
        assert!(matches!(
            e.dispatch(0, Operation::Status {}),
            Answer::Error {
                code: Error::InvalidMessage
            }
        ));
        assert!(matches!(e.dispatch(0, hello()), Answer::Ok { .. }));
        assert!(matches!(e.dispatch(0, hello()), Answer::Ok { .. }));
        assert!(matches!(
            e.dispatch(1, Operation::Status {}),
            Answer::Error { .. }
        ));
        assert!(matches!(
            e.dispatch(0, Operation::Status {}),
            Answer::Error {
                code: Error::Stopped
            }
        ));
    }
    #[test]
    fn shutdown_fences_late_operations_and_unqualified_backends() {
        let mut e = endpoint();
        e.dispatch(0, hello());
        assert!(matches!(
            e.dispatch(
                0,
                Operation::EnrollKey {
                    origin: "https://attacker.example".into()
                }
            ),
            Answer::Error {
                code: Error::BindingMismatch
            }
        ));
        assert!(matches!(
            e.dispatch(
                0,
                Operation::EnrollKey {
                    origin: crate::issuer::ORIGIN.into()
                }
            ),
            Answer::Error {
                code: Error::NotEnrolled
            }
        ));
        assert!(matches!(
            e.dispatch(
                0,
                Operation::CaptureStart {
                    operation: "operation".into(),
                    room: "!room:example".into()
                }
            ),
            Answer::Error {
                code: Error::CaptureUnavailable
            }
        ));
        e.revoke();
        assert!(matches!(
            e.dispatch(0, hello()),
            Answer::Error {
                code: Error::Stopped
            }
        ));
    }
}
