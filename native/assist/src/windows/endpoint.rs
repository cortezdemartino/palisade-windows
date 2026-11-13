//! One coordinator-owned endpoint, never one signer or enrollment per request.
//! No input, capture, peer or resource backend is admitted by this milestone.
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
    sessions: BTreeMap<(u32, String), super::runtime::RuntimeRevoker>,
    closed: BTreeSet<(u32, String)>,
}
#[derive(Clone, Default)]
pub(crate) struct Safety(Arc<Mutex<SafetyState>>);
impl Safety {
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
        let result = self.execute(document, operation);
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
    fn execute(&mut self, document: u32, operation: Operation) -> Result<serde_json::Value, Error> {
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
                json!({"version":1,"features":{"capture":self.capture.is_some(),"clipboard":false,"files":false,"input":false}}),
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
            Operation::GrantOffer { session, grant } => self.offer(document, &session, &grant),
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
    ) -> Result<serde_json::Value, Error> {
        use super::enrollment::{EnrolledIdentity, PublishedSource};
        let enrollment = self.enrollment.as_ref().ok_or(Error::NotEnrolled)?;
        let identity = self.identity.as_ref().ok_or(Error::NotEnrolled)?;
        let issuer = self.issuer.as_ref().ok_or(Error::NotEnrolled)?;
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
        let id = signed.grant.grant_id.clone();
        if let Some(previous) = self.sessions.get(session) {
            if previous.binding != binding
                || previous.source.key != source.key
                || previous.runtime.ended()
                || previous.offered.contains(&id)
            {
                return Err(Error::StaleOperation);
            }
        } else {
            // Retain ended sessions as tombstones; never reconstruct their gate.
            if self.sessions.len() >= 16 {
                return Err(Error::Busy);
            }
            let runtime = super::runtime::StreamRuntime::begin(
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
            )
            .map_err(map_runtime_error)?;
            self.safety
                .register(document, session, runtime.revoker())
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
        let entry = self.sessions.get_mut(session).ok_or(Error::Stopped)?;
        entry.runtime.admit(&signed).map_err(map_runtime_error)?;
        entry.offered.insert(id.clone());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while !entry
            .runtime
            .source_confirmed()
            .map_err(map_runtime_error)?
        {
            if self.stop.is_stopped() || entry.runtime.ended() {
                return Err(Error::Stopped);
            }
            if std::time::Instant::now() >= deadline {
                entry.runtime.revoker().revoke();
                return Err(Error::Timeout);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        entry
            .runtime
            .request_consent(&id)
            .map_err(map_runtime_error)?;
        loop {
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
                    return Ok(json!({"signature": result.signature.ok_or(Error::GrantInvalid)?}));
                }
                "consent-denied" => return Err(Error::ConsentDenied),
                _ if std::time::Instant::now() >= deadline => {
                    entry.runtime.revoker().revoke();
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
        "controller-busy" | "consent-unavailable" => Error::Busy,
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
