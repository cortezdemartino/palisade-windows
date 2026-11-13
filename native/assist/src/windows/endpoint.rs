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
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub struct Endpoint {
    document: Option<u32>,
    negotiated: bool,
    probe: bool,
    cancelled: Arc<AtomicBool>,
    identity: Option<EndpointIdentity>,
    issuer: Option<TrustedIssuer>,
    enrollment: Option<Enrollment>,
    attempted: BTreeSet<String>,
    proof_prepared: bool,
    custody: Option<Arc<super::process_input::custody::Client>>,
    probe_executor: Option<super::process_input::ProcessInput>,
    probe_generation: u64,
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
        }
    }
    fn live(&self) -> Result<(), Error> {
        if self.cancelled.load(Ordering::SeqCst) {
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
                json!({"version":1,"features":{"capture":false,"clipboard":false,"files":false,"input":false}}),
            );
        }
        if !self.negotiated {
            return Err(Error::InvalidMessage);
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
                    self.identity = Some(identity);
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
                "capture":{"operation":null,"state":"idle","trackSid":null},
                "controller":{"session":null,"state":"idle"}
            })),
            Operation::GrantOffer { .. } => Err(if self.enrollment.is_none() {
                Error::NotEnrolled
            } else {
                Error::CaptureUnavailable
            }),
            Operation::PeerOffer { .. } | Operation::PeerCandidate { .. } => {
                Err(Error::TransportMismatch)
            }
            Operation::PeerClose { .. } => Ok(json!({})),
            Operation::CaptureStart { .. } | Operation::CaptureBind { .. } => {
                Err(Error::CaptureUnavailable)
            }
            Operation::CaptureCancel { .. } | Operation::CaptureStop { .. } => Ok(json!({})),
            Operation::Hello { .. } => unreachable!(),
        }
    }
    pub fn revoke(&mut self) {
        self.cancelled.store(true, Ordering::SeqCst);
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
            },
        )
    }
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        self.revoke();
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
