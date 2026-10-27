//! A locally confirmed enrollment tuple, never deserialized or inferred from a
//! grant/page. It is not proof of service registration or session permission.
use super::{identity::Identity, issuer::TrustedIssuer};
use crate::{
    grant::{mxid, token},
    stream::{Binding, SignedStreamGrant},
};
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};
pub struct Enrollment {
    issuer: String,
    endpoint: String,
    account: String,
    device: String,
    public_key: String,
    revoked: AtomicBool,
    sessions: Mutex<Vec<Weak<AtomicBool>>>,
}
pub struct Offer<'a> {
    pub endpoint: &'a str,
    pub account: &'a str,
    pub device: &'a str,
    pub challenge: &'a str,
}
pub struct PublishedSource<'a> {
    pub room: &'a str,
    pub stream: &'a str,
}
pub struct EnrolledIdentity<'a> {
    pub issuer: &'a TrustedIssuer,
    pub identity: Arc<Identity>,
    pub enrollment: &'a Enrollment,
}
impl Enrollment {
    fn prepare(
        issuer: &TrustedIssuer,
        identity: &Identity,
        offer: &Offer<'_>,
    ) -> Result<Self, &'static str> {
        if !mxid(offer.account)
            || !token(offer.device, 1, 64)
            || !token(offer.endpoint, 8, 128)
            || !super::consent::safe_identity(offer.account)
        {
            return Err("invalid-enrollment");
        }
        crate::statements::enrollment(
            issuer.fingerprint(),
            offer.endpoint,
            offer.account,
            offer.device,
            offer.challenge,
        )?;
        Ok(Self {
            issuer: issuer.fingerprint().into(),
            endpoint: offer.endpoint.into(),
            account: offer.account.into(),
            device: offer.device.into(),
            public_key: identity.public_key(),
            revoked: AtomicBool::new(false),
            sessions: Mutex::new(Vec::new()),
        })
    }
    pub fn request(
        issuer: &TrustedIssuer,
        identity: &Identity,
        offer: &Offer<'_>,
        cancel: Arc<AtomicBool>,
    ) -> Result<(Self, String), &'static str> {
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled");
        }
        let record = Self::prepare(issuer, identity, offer)?;
        let text = format!(
            "Enroll this Palisade installation for Assist?\r\n\r\nOrigin: {}\r\nAccount: {}\r\nDevice: {}\r\nEndpoint: {}\r\nIssuer SHA-256: {}\r\nInstallation public key: {}\r\n\r\nConfirm the account and device you are using. This records only this installation's identity; it grants no remote access. Every session capability still requires separate local approval. Restarting or re-enrolling requires a new confirmation.",
            crate::issuer::ORIGIN,
            record.account,
            record.device,
            record.endpoint,
            record.issuer,
            record.public_key
        );
        let signature = record.prove_after(
            identity,
            offer.challenge,
            &cancel,
            || super::consent::confirm_enrollment(&text, cancel.clone()),
            || {
                let current = TrustedIssuer::load()?;
                super::input::ordinary_desktop()?;
                record.verify_context(&current, identity)
            },
        )?;
        Ok((record, signature))
    }
    fn prove_after(
        &self,
        identity: &Identity,
        challenge: &str,
        cancel: &AtomicBool,
        confirm: impl FnOnce() -> Result<bool, &'static str>,
        recheck: impl FnOnce() -> Result<(), &'static str>,
    ) -> Result<String, &'static str> {
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled");
        }
        if !confirm()? {
            return Err("consent-denied");
        }
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled");
        }
        recheck()?;
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled");
        }
        let signature = identity.prove_enrollment(self, challenge)?;
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled");
        }
        Ok(signature)
    }
    pub fn revoke(&self) {
        self.revoked.store(true, Ordering::SeqCst);
        let mut sessions = self
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for session in sessions.drain(..).filter_map(|weak| weak.upgrade()) {
            session.store(true, Ordering::SeqCst);
        }
    }
    pub(crate) fn bind_session(&self, stop: &Arc<AtomicBool>) -> Result<(), &'static str> {
        let mut sessions = self.sessions.lock().map_err(|_| "enrollment-failed")?;
        if self.revoked.load(Ordering::SeqCst) || stop.load(Ordering::SeqCst) {
            return Err("enrollment-revoked");
        }
        sessions.retain(|weak| weak.strong_count() > 0);
        let weak = Arc::downgrade(stop);
        if !sessions.iter().any(|saved| saved.ptr_eq(&weak)) {
            if sessions.len() >= 16 {
                return Err("enrollment-busy");
            }
            sessions.push(weak);
        }
        Ok(())
    }
    pub(crate) fn statement(
        &self,
        identity: &Identity,
        challenge: &str,
    ) -> Result<Vec<u8>, &'static str> {
        if self.revoked.load(Ordering::SeqCst) || identity.public_key() != self.public_key {
            return Err("enrollment-identity-mismatch");
        }
        crate::statements::enrollment(
            &self.issuer,
            &self.endpoint,
            &self.account,
            &self.device,
            challenge,
        )
    }
    pub(crate) fn verify_context(
        &self,
        issuer: &TrustedIssuer,
        identity: &Identity,
    ) -> Result<(), &'static str> {
        if self.revoked.load(Ordering::SeqCst)
            || self.issuer != issuer.fingerprint()
            || self.public_key != identity.public_key()
        {
            return Err("enrollment-identity-mismatch");
        }
        Ok(())
    }
    pub(crate) fn verify_binding(
        &self,
        issuer: &TrustedIssuer,
        identity: &Identity,
        binding: &Binding,
    ) -> Result<(), &'static str> {
        self.verify_context(issuer, identity)?;
        if binding.publisher_account != self.account
            || binding.publisher_device != self.device
            || binding.endpoint != self.endpoint
        {
            return Err("binding-mismatch");
        }
        Ok(())
    }
    // Source room/SID must come from the native-owned capture operation. The
    // SID remains a tentative publication claim; it does not grant OS scope.
    pub fn native_binding(
        &self,
        issuer: &TrustedIssuer,
        identity: &Identity,
        session: &str,
        source: PublishedSource<'_>,
        signed: &SignedStreamGrant,
        now: u64,
    ) -> Result<Binding, &'static str> {
        let grant = &signed.grant;
        let binding = Binding {
            session: session.into(),
            room: source.room.into(),
            stream: source.stream.into(),
            publisher_account: self.account.clone(),
            publisher_device: self.device.clone(),
            endpoint: self.endpoint.clone(),
            helper_account: grant.helper_account.clone(),
            helper_device: grant.helper_device.clone(),
            helper_transport: grant.helper_transport.clone(),
        };
        self.verify_binding(issuer, identity, &binding)?;
        if grant.endpoint.as_deref() != Some(self.endpoint.as_str()) {
            return Err("binding-mismatch");
        }
        crate::stream::verify_grant(signed, &issuer.key(), &binding, now, None, 0)?;
        Ok(binding)
    }
}
impl Drop for Enrollment {
    fn drop(&mut self) {
        self.revoke();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{grant::decode, issuer::Candidate};
    use ed25519_dalek::{Signature, VerifyingKey};
    struct Fixture {
        issuer: TrustedIssuer,
        identity: Identity,
        other: Identity,
        signed: SignedStreamGrant,
        path: std::path::PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let f: serde_json::Value = serde_json::from_str(include_str!(
                "../../../../tests/fixtures/assist-native-v1.json"
            ))
            .unwrap();
            let candidate = Candidate::parse(&serde_json::to_vec(&serde_json::json!({"protocol":"palisade.assist.control","version":1,"publicKey":f["issuerPublicKey"],"fingerprint":f["issuerFingerprint"]})).unwrap()).unwrap();
            let path = std::env::temp_dir().join(format!(
                "palisade-enrollment-fixture-{}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            Self {
                issuer: TrustedIssuer::fixture(candidate),
                identity: Identity::fixture(&path.join("one.dpapi")).unwrap(),
                other: Identity::fixture(&path.join("two.dpapi")).unwrap(),
                signed: serde_json::from_value(f["nativeGrant"].clone()).unwrap(),
                path,
            }
        }
        fn offer(&self) -> Offer<'_> {
            let g = &self.signed.grant;
            Offer {
                endpoint: g.endpoint.as_deref().unwrap(),
                account: &g.publisher_account,
                device: &g.publisher_device,
                challenge: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            }
        }
        fn record(&self) -> Enrollment {
            Enrollment::prepare(&self.issuer, &self.identity, &self.offer()).unwrap()
        }
        fn binding(
            &self,
            record: &Enrollment,
            room: &str,
            stream: &str,
        ) -> Result<Binding, &'static str> {
            record.native_binding(
                &self.issuer,
                &self.identity,
                &self.signed.grant.session,
                PublishedSource { room, stream },
                &self.signed,
                self.signed.grant.issued_at,
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            for name in ["one.dpapi", "two.dpapi"] {
                std::fs::remove_file(self.path.join(name)).unwrap();
            }
            std::fs::remove_dir(&self.path).unwrap();
        }
    }
    #[test]
    fn proof_uses_only_confirmed_tuple_own_identity_and_labelled_challenge() {
        let f = Fixture::new();
        let record = f.record();
        let challenge = f.offer().challenge;
        let proof = f.identity.prove_enrollment(&record, challenge).unwrap();
        let public =
            VerifyingKey::from_bytes(&decode::<32>(&f.identity.public_key()).unwrap()).unwrap();
        let signature = Signature::from_bytes(&decode::<64>(&proof).unwrap());
        public
            .verify_strict(
                &record.statement(&f.identity, challenge).unwrap(),
                &signature,
            )
            .unwrap();
        assert!(
            public
                .verify_strict(&decode::<32>(challenge).unwrap(), &signature)
                .is_err()
        );
        assert!(
            public
                .verify_strict(
                    &crate::statements::acknowledgement(&f.signed.grant).unwrap(),
                    &signature
                )
                .is_err()
        );
        assert!(f.other.prove_enrollment(&record, challenge).is_err());
        assert!(
            f.identity
                .prove_enrollment(&record, "invalid challenge")
                .is_err()
        );
    }
    #[test]
    fn publisher_expectations_come_from_local_enrollment_and_owned_publication() {
        let f = Fixture::new();
        let g = &f.signed.grant;
        let record = f.record();
        assert!(f.binding(&record, &g.room, &g.stream).is_ok());
        assert!(
            f.binding(&record, "!another:palisade.test", &g.stream)
                .is_err()
        );
        assert!(f.binding(&record, &g.room, "TR_another01").is_err());
        for field in ["endpoint", "account", "device"] {
            let mut changed = f.record();
            match field {
                "endpoint" => changed.endpoint = "endpoint_other_01".into(),
                "account" => changed.account = "@other:palisade.test".into(),
                _ => changed.device = "OTHER".into(),
            }
            assert!(f.binding(&changed, &g.room, &g.stream).is_err());
        }
        assert!(record.verify_context(&f.issuer, &f.other).is_err());
        let mut changed = f.record();
        changed.issuer = "a".repeat(64);
        assert!(changed.verify_context(&f.issuer, &f.identity).is_err());
        assert!(
            record
                .native_binding(
                    &f.issuer,
                    &f.identity,
                    "session_another01",
                    PublishedSource {
                        room: &g.room,
                        stream: &g.stream
                    },
                    &f.signed,
                    g.issued_at
                )
                .is_err()
        );
    }
    #[test]
    fn malformed_or_display_spoofing_offers_cannot_prepare_a_signer() {
        let f = Fixture::new();
        for account in [
            "not-an-account",
            "@user\nforged:palisade.test",
            "@user\u{202e}forged:palisade.test",
            "@user\u{061c}forged:palisade.test",
        ] {
            let offer = Offer {
                account,
                ..f.offer()
            };
            assert!(Enrollment::prepare(&f.issuer, &f.identity, &offer).is_err());
        }
        for device in ["", "DEVICE\n", "device/other"] {
            let offer = Offer {
                device,
                ..f.offer()
            };
            assert!(Enrollment::prepare(&f.issuer, &f.identity, &offer).is_err());
        }
        for endpoint in ["short", "endpoint\nother", "endpoint/other"] {
            let offer = Offer {
                endpoint,
                ..f.offer()
            };
            assert!(Enrollment::prepare(&f.issuer, &f.identity, &offer).is_err());
        }
    }
    #[test]
    fn pre_cancelled_enrollment_does_not_open_native_confirmation_or_fetch_issuer() {
        let f = Fixture::new();
        let cancelled = Arc::new(AtomicBool::new(true));
        assert!(matches!(
            Enrollment::request(&f.issuer, &f.identity, &f.offer(), cancelled),
            Err("cancelled")
        ));
    }
    #[test]
    fn denial_cancellation_and_failed_issuer_revalidation_return_no_proof() {
        let f = Fixture::new();
        let record = f.record();
        let cancel = AtomicBool::new(false);
        let challenge = f.offer().challenge;
        assert!(matches!(
            record.prove_after(
                &f.identity,
                challenge,
                &cancel,
                || Ok(false),
                || panic!("No issuer fetch after denial")
            ),
            Err("consent-denied")
        ));
        assert!(matches!(
            record.prove_after(
                &f.identity,
                challenge,
                &cancel,
                || {
                    cancel.store(true, Ordering::SeqCst);
                    Ok(true)
                },
                || panic!("No issuer fetch after cancel")
            ),
            Err("cancelled")
        ));
        cancel.store(false, Ordering::SeqCst);
        assert!(matches!(
            record.prove_after(
                &f.identity,
                challenge,
                &cancel,
                || Ok(true),
                || Err("issuer-changed")
            ),
            Err("issuer-changed")
        ));
        assert!(matches!(
            record.prove_after(
                &f.identity,
                challenge,
                &cancel,
                || Ok(true),
                || {
                    cancel.store(true, Ordering::SeqCst);
                    Ok(())
                }
            ),
            Err("cancelled")
        ));
    }
    #[test]
    fn replacement_or_drop_revokes_every_old_session_without_affecting_a_successor() {
        let f = Fixture::new();
        let old = f.record();
        let fresh = f.record();
        let old_stop = Arc::new(AtomicBool::new(false));
        let other_old = Arc::new(AtomicBool::new(false));
        let new_stop = Arc::new(AtomicBool::new(false));
        old.bind_session(&old_stop).unwrap();
        old.bind_session(&other_old).unwrap();
        fresh.bind_session(&new_stop).unwrap();
        old.revoke();
        assert!(old_stop.load(Ordering::SeqCst));
        assert!(other_old.load(Ordering::SeqCst));
        assert!(!new_stop.load(Ordering::SeqCst));
        assert!(old.bind_session(&new_stop).is_err());
        assert!(
            f.identity
                .prove_enrollment(&old, f.offer().challenge)
                .is_err()
        );
        assert!(
            f.binding(&old, &f.signed.grant.room, &f.signed.grant.stream)
                .is_err()
        );
        drop(fresh);
        assert!(new_stop.load(Ordering::SeqCst));
    }
    #[test]
    fn concurrent_session_registration_cannot_escape_enrollment_revocation() {
        let f = Fixture::new();
        let record = Arc::new(f.record());
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let stop = Arc::new(AtomicBool::new(false));
        let (held, ready, flag) = (record.clone(), barrier.clone(), stop.clone());
        let worker = std::thread::spawn(move || {
            ready.wait();
            held.bind_session(&flag)
        });
        barrier.wait();
        record.revoke();
        let registered = worker.join().unwrap();
        if registered.is_ok() {
            assert!(stop.load(Ordering::SeqCst));
        }
        assert!(
            record
                .bind_session(&Arc::new(AtomicBool::new(false)))
                .is_err()
        );
    }
}
