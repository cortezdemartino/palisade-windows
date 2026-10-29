//! Canonical docs/48 v2 verification. A verified signature alone is never authority.
use crate::grant::{CAPABILITIES, decode, fingerprint, mxid, required_endpoint, token};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);
const MAX_TIME: u64 = 8_640_000_000_000_000;
const MAX_GENERATION: u64 = 9_007_199_254_740_991;
fn is_input(capability: &str) -> bool {
    matches!(capability, "control_mouse" | "control_keyboard")
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StreamGrant {
    pub approved_at: u64,
    pub capability: String,
    #[serde(deserialize_with = "required_endpoint")]
    pub endpoint: Option<String>,
    pub expires_at: u64,
    pub generation: u64,
    pub grant_id: String,
    pub helper_account: String,
    pub helper_device: String,
    pub helper_transport: String,
    pub issued_at: u64,
    pub issuer: String,
    pub nonce: String,
    pub protocol: String,
    pub publisher_account: String,
    pub publisher_device: String,
    pub room: String,
    pub session: String,
    pub session_deadline: u64,
    pub stream: String,
    pub version: u32,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedStreamGrant {
    pub grant: StreamGrant,
    pub signature: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StreamLease {
    pub expires_at: u64,
    pub generation: u64,
    pub grants: Vec<String>,
    pub issued_at: u64,
    pub issuer: String,
    pub kind: String,
    pub protocol: String,
    pub session: String,
    pub session_deadline: u64,
    pub stream: String,
    pub version: u32,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedStreamLease {
    pub lease: StreamLease,
    pub signature: String,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Binding {
    pub session: String,
    pub room: String,
    pub stream: String,
    pub publisher_account: String,
    pub publisher_device: String,
    pub helper_account: String,
    pub helper_device: String,
    pub helper_transport: String,
    pub endpoint: String,
}
fn track(v: &str) -> bool {
    v.strip_prefix("TR_").is_some_and(|v| {
        (4..=64).contains(&v.len()) && v.bytes().all(|c| c.is_ascii_alphanumeric())
    })
}
fn room(v: &str) -> bool {
    let Some(v) = v.strip_prefix('!') else {
        return false;
    };
    let (local, server) = v.split_once(':').map_or((v, None), |(a, b)| (a, Some(b)));
    (1..=255).contains(&local.len())
        && local
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._~-+/=".contains(&c))
        && server.is_none_or(|s| {
            !s.is_empty()
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b".-:[]".contains(&c))
        })
}
fn header(issuer: &str, protocol: &str, version: u32) -> Result<(), &'static str> {
    if issuer != "palisade" || protocol != "palisade.assist.control" || version != 2 {
        Err("incompatible-protocol")
    } else {
        Ok(())
    }
}
fn signature<T: Serialize>(record: &T, text: &str, issuer: &[u8; 32]) -> Result<(), &'static str> {
    let key = VerifyingKey::from_bytes(issuer).map_err(|_| "untrusted-issuer")?;
    let signature = Signature::from_bytes(&decode::<64>(text)?);
    key.verify_strict(
        &serde_json::to_vec(record).map_err(|_| "invalid-message")?,
        &signature,
    )
    .map_err(|_| "binding-mismatch")
}
impl StreamGrant {
    pub fn validate(&self) -> Result<(), &'static str> {
        header(&self.issuer, &self.protocol, self.version)?;
        let native = matches!(
            self.capability.as_str(),
            "control_mouse" | "control_keyboard"
        );
        if !token(&self.grant_id, 8, 128)
            || !token(&self.session, 8, 128)
            || !token(&self.nonce, 16, 128)
            || !room(&self.room)
            || !track(&self.stream)
            || !CAPABILITIES.contains(&self.capability.as_str())
            || !(1..=MAX_GENERATION).contains(&self.generation)
            || !mxid(&self.publisher_account)
            || !mxid(&self.helper_account)
            || self.publisher_account == self.helper_account
            || !token(&self.publisher_device, 1, 64)
            || !token(&self.helper_device, 1, 64)
            || !fingerprint(&self.helper_transport)
            || native != self.endpoint.is_some()
            || self.endpoint.as_ref().is_some_and(|v| !token(v, 8, 128))
            || [
                self.approved_at,
                self.issued_at,
                self.expires_at,
                self.session_deadline,
            ]
            .iter()
            .any(|&t| t > MAX_TIME)
            || self.session_deadline < self.approved_at
            || !(60_000..=7_200_000).contains(&(self.session_deadline - self.approved_at))
            || self.issued_at < self.approved_at
            || self.expires_at <= self.issued_at
            || self.expires_at > self.session_deadline
        {
            return Err("invalid-message");
        }
        Ok(())
    }
    pub fn canonical(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("bounded grant")
    }
}
pub fn verify_grant(
    signed: &SignedStreamGrant,
    issuer: &[u8; 32],
    binding: &Binding,
    now: u64,
    deadline: Option<u64>,
    minimum_generation: u64,
) -> Result<(), &'static str> {
    let g = &signed.grant;
    g.validate()?;
    signature(g, &signed.signature, issuer)?;
    if g.session != binding.session
        || g.room != binding.room
        || g.stream != binding.stream
        || g.publisher_account != binding.publisher_account
        || g.publisher_device != binding.publisher_device
        || g.helper_account != binding.helper_account
        || g.helper_device != binding.helper_device
        || g.helper_transport != binding.helper_transport
        || g.endpoint.as_ref().is_some_and(|e| *e != binding.endpoint)
        || deadline.is_some_and(|d| d != g.session_deadline)
    {
        return Err("binding-mismatch");
    }
    if now > MAX_TIME || g.expires_at <= now || g.issued_at > now.saturating_add(30_000) {
        return Err("expired");
    }
    if g.generation < minimum_generation {
        return Err("stale-generation");
    }
    Ok(())
}
pub fn verify_lease(
    signed: &SignedStreamLease,
    issuer: &[u8; 32],
    binding: &Binding,
    deadline: u64,
    now: u64,
) -> Result<(), &'static str> {
    let l = &signed.lease;
    header(&l.issuer, &l.protocol, l.version)?;
    if l.kind != "lease"
        || !token(&l.session, 8, 128)
        || !track(&l.stream)
        || !(1..=MAX_GENERATION).contains(&l.generation)
        || l.grants.len() > 8
        || l.grants.iter().any(|id| !token(id, 8, 128))
        || l.grants.windows(2).any(|p| p[0] >= p[1])
        || [l.issued_at, l.expires_at, l.session_deadline]
            .iter()
            .any(|&t| t > MAX_TIME)
        || l.expires_at <= l.issued_at
        || l.expires_at - l.issued_at > 20_000
        || l.expires_at > l.session_deadline
    {
        return Err("invalid-message");
    }
    signature(l, &signed.signature, issuer)?;
    if l.session != binding.session || l.stream != binding.stream || l.session_deadline != deadline
    {
        return Err("binding-mismatch");
    }
    if now > MAX_TIME || l.expires_at <= now || l.issued_at > now.saturating_add(30_000) {
        return Err("expired");
    }
    Ok(())
}

/// One endpoint owns one controller. Callers cannot manufacture an Authorized result.
pub struct SessionGate {
    issuer: [u8; 32],
    binding: Binding,
    approved_at: Option<u64>,
    deadline: Option<u64>,
    highest_grant: u64,
    grants: BTreeMap<String, StreamGrant>,
    consent: BTreeSet<String>,
    active_seen: BTreeSet<String>,
    revoked: BTreeSet<String>,
    lease: Option<StreamLease>,
    lease_until: Option<Instant>,
    last_lease: u64,
    last_wall: u64,
    sequences: BTreeMap<String, u32>,
    rate: (Instant, f64),
    ended: bool,
    epoch: u64,
    input_epoch: u64,
    instance: u64,
    peer_verified: bool,
}
pub struct ExecutionPermit {
    instance: u64,
    epoch: u64,
    grant: String,
    capability: String,
    source_generation: u32,
    until: Instant,
    action: crate::input::Action,
}
impl ExecutionPermit {
    pub fn matches(&self, action: &crate::input::Action) -> bool {
        self.action == *action
    }
}
impl SessionGate {
    pub fn new(
        issuer: [u8; 32],
        binding: Binding,
        now: u64,
        minimum_generation: u64,
        instant: Instant,
    ) -> Self {
        Self {
            issuer,
            binding,
            approved_at: None,
            deadline: None,
            highest_grant: minimum_generation,
            grants: BTreeMap::new(),
            consent: BTreeSet::new(),
            active_seen: BTreeSet::new(),
            revoked: BTreeSet::new(),
            lease: None,
            lease_until: None,
            last_lease: 0,
            last_wall: now,
            sequences: BTreeMap::new(),
            rate: (instant, 240.0),
            ended: false,
            epoch: 1,
            input_epoch: 1,
            instance: NEXT_INSTANCE
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| v.checked_add(1))
                .expect("endpoint instance exhausted"),
            peer_verified: false,
        }
    }
    fn clock(&mut self, now: u64) -> Result<(), &'static str> {
        if self.ended
            || now < self.last_wall
            || now > MAX_TIME
            || self.deadline.is_some_and(|d| now >= d)
        {
            self.stop();
            return Err("expired");
        }
        self.last_wall = now;
        Ok(())
    }
    pub fn admit(&mut self, signed: &SignedStreamGrant, now: u64) -> Result<(), &'static str> {
        self.clock(now)?;
        verify_grant(
            signed,
            &self.issuer,
            &self.binding,
            now,
            self.deadline,
            if self.grants.contains_key(&signed.grant.grant_id) {
                0
            } else {
                self.highest_grant
            },
        )?;
        let g = &signed.grant;
        if self.revoked.contains(&g.grant_id)
            || self.approved_at.is_some_and(|a| a != g.approved_at)
        {
            return Err("revoked");
        }
        if let Some(previous) = self.grants.get(&g.grant_id) {
            if previous.canonical() != g.canonical() {
                return Err("binding-mismatch");
            }
            return Ok(());
        }
        if self.grants.len() >= 8 || self.grants.values().any(|v| v.capability == g.capability) {
            return Err("not-granted");
        }
        self.approved_at = Some(g.approved_at);
        self.deadline = Some(g.session_deadline);
        self.highest_grant = self.highest_grant.max(g.generation);
        self.grants.insert(g.grant_id.clone(), g.clone());
        Ok(())
    }
    pub fn consent(&mut self, grant_id: &str, now: u64) -> Result<(), &'static str> {
        self.clock(now)?;
        if !self.grants.contains_key(grant_id) || self.revoked.contains(grant_id) {
            return Err("not-granted");
        }
        self.consent.insert(grant_id.into());
        Ok(())
    }
    /// Polling may still observe a service-active grant that was locally revoked.
    /// Validate its unchanged signed record without restoring any authority.
    pub fn observe_grant(
        &mut self,
        signed: &SignedStreamGrant,
        now: u64,
    ) -> Result<(), &'static str> {
        self.clock(now)?;
        if self.revoked.contains(&signed.grant.grant_id) {
            let previous = self.grants.get(&signed.grant.grant_id).ok_or("revoked")?;
            if previous.canonical() != signed.grant.canonical() {
                return Err("binding-mismatch");
            }
            signature(&signed.grant, &signed.signature, &self.issuer)
        } else {
            self.admit(signed, now)
        }
    }
    pub fn refresh(
        &mut self,
        signed: &SignedStreamLease,
        now: u64,
        request_started: Instant,
        instant: Instant,
    ) -> Result<(), &'static str> {
        let result = (|| {
            self.clock(now)?;
            let deadline = self.deadline.ok_or("not-granted")?;
            verify_lease(signed, &self.issuer, &self.binding, deadline, now)?;
            let l = &signed.lease;
            if l.issued_at <= self.last_lease
                || instant.duration_since(request_started) >= Duration::from_secs(20)
            {
                return Err("replay");
            }
            let ids: Vec<_> = self.consent.iter().cloned().collect();
            for id in ids {
                if !l.grants.contains(&id) && self.active_seen.contains(&id) {
                    self.revoke(&id)
                }
            }
            for id in &l.grants {
                if self.grants.contains_key(id) {
                    self.active_seen.insert(id.clone());
                }
            }
            self.last_lease = l.issued_at;
            // Wall-clock skew must never turn a 20-second lease into 50 seconds.
            self.lease_until = Some(
                (request_started + Duration::from_secs(20))
                    .min(instant + Duration::from_millis(l.expires_at.saturating_sub(now))),
            );
            self.lease = Some(l.clone());
            Ok(())
        })();
        if result.is_err() {
            self.stop()
        };
        result
    }
    pub fn authorize(
        &mut self,
        grant_id: &str,
        capability: &str,
        now: u64,
        instant: Instant,
    ) -> Result<&StreamGrant, &'static str> {
        self.clock(now)?;
        if !self.peer_verified {
            return Err("peer-unverified");
        }
        let until = self.lease_until.ok_or("lease-required")?;
        if instant >= until {
            self.stop();
            return Err("expired");
        }
        let g = self.grants.get(grant_id).ok_or("not-granted")?;
        let l = self.lease.as_ref().ok_or("not-granted")?;
        if self.revoked.contains(grant_id)
            || !self.consent.contains(grant_id)
            || g.capability != capability
            || !l.grants.iter().any(|v| v == grant_id)
            || g.generation > l.generation
            || now >= g.expires_at
            || now >= l.expires_at
        {
            return Err("not-granted");
        }
        Ok(g)
    }
    pub fn check_message(
        &mut self,
        grant_id: &str,
        capability: &str,
        generation: u64,
        sequence: u32,
        now: u64,
        instant: Instant,
    ) -> Result<(), &'static str> {
        if self
            .authorize(grant_id, capability, now, instant)?
            .generation
            != generation
        {
            return Err("stale-generation");
        }
        if self.sequences.get(grant_id).is_some_and(|&n| sequence <= n) {
            return Err("replay");
        }
        self.rate.1 =
            (self.rate.1 + instant.duration_since(self.rate.0).as_secs_f64() * 120.0).min(240.0);
        self.rate.0 = instant;
        if self.rate.1 < 1.0 {
            self.stop();
            return Err("rate-limited");
        }
        self.rate.1 -= 1.0;
        self.sequences.insert(grant_id.into(), sequence);
        Ok(())
    }
    pub fn revoke(&mut self, id: &str) {
        if self.revoked.contains(id) {
            return;
        }
        self.epoch = self.epoch.checked_add(1).expect("endpoint epoch exhausted");
        if self
            .grants
            .get(id)
            .is_some_and(|grant| is_input(&grant.capability))
        {
            self.input_epoch = self
                .input_epoch
                .checked_add(1)
                .expect("input epoch exhausted");
        }
        self.consent.remove(id);
        self.revoked.insert(id.into());
        self.sequences.remove(id);
    }
    /// Terminal for the current input grants. A fresh lease or old consent cannot
    /// restore them. Independently granted clipboard/files keep their own checks.
    pub fn suspend_input(&mut self) {
        let ids: Vec<_> = self
            .grants
            .values()
            .filter(|grant| is_input(&grant.capability))
            .map(|grant| grant.grant_id.clone())
            .collect();
        for id in ids {
            self.revoke(&id);
        }
    }
    pub fn stop(&mut self) {
        self.epoch = self.epoch.checked_add(1).expect("endpoint epoch exhausted");
        self.input_epoch = self
            .input_epoch
            .checked_add(1)
            .expect("input epoch exhausted");
        self.ended = true;
        self.peer_verified = false;
        self.consent.clear();
        self.active_seen.clear();
        self.grants.clear();
        self.lease = None;
        self.lease_until = None;
        self.sequences.clear();
    }
    pub fn ended(&self) -> bool {
        self.ended
    }
    pub(crate) fn input_active(&mut self, now: u64, instant: Instant) -> bool {
        let candidates: Vec<_> = self
            .grants
            .values()
            .filter(|g| matches!(g.capability.as_str(), "control_mouse" | "control_keyboard"))
            .map(|g| (g.grant_id.clone(), g.capability.clone()))
            .collect();
        candidates
            .iter()
            .any(|(id, cap)| self.authorize(id, cap, now, instant).is_ok())
    }
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
    pub fn deadline(&self) -> Option<u64> {
        self.deadline
    }
    pub fn highest_generation(&self) -> u64 {
        self.highest_grant
    }
    pub(crate) fn input_epoch(&self) -> u64 {
        self.input_epoch
    }
    pub fn verify_peer(&mut self, fingerprint: &str) -> Result<(), &'static str> {
        if self.ended || fingerprint != self.binding.helper_transport {
            self.stop();
            return Err("peer-mismatch");
        }
        self.peer_verified = true;
        Ok(())
    }
    pub fn pending_grant(&mut self, id: &str, now: u64) -> Result<StreamGrant, &'static str> {
        self.clock(now)?;
        if self.revoked.contains(id) {
            return Err("revoked");
        }
        self.grants
            .get(id)
            .filter(|g| now < g.expires_at)
            .cloned()
            .ok_or("not-granted")
    }
    pub fn tick(&mut self, now: u64, instant: Instant) -> Result<(), &'static str> {
        self.clock(now)?;
        let expired: Vec<_> = self
            .grants
            .values()
            .filter(|g| g.expires_at <= now)
            .map(|g| g.grant_id.clone())
            .collect();
        for id in expired {
            self.revoke(&id);
        }
        if self.lease_until.is_some_and(|until| instant >= until) {
            self.stop();
            return Err("expired");
        };
        Ok(())
    }
    pub fn permit(
        &mut self,
        message: &crate::input::Message,
        source_generation: u32,
        now: u64,
        instant: Instant,
    ) -> Result<ExecutionPermit, &'static str> {
        message.action.validate()?;
        if message.session != self.binding.session {
            return Err("binding-mismatch");
        }
        let capability = match message.action {
            crate::input::Action::Move { .. }
            | crate::input::Action::Button { .. }
            | crate::input::Action::Wheel { .. } => "control_mouse",
            _ => "control_keyboard",
        };
        self.check_message(
            &message.grant,
            capability,
            message.generation,
            message.sequence,
            now,
            instant,
        )?;
        Ok(ExecutionPermit {
            instance: self.instance,
            epoch: self.input_epoch,
            grant: message.grant.clone(),
            capability: capability.into(),
            source_generation,
            until: instant + Duration::from_millis(100),
            action: message.action.clone(),
        })
    }
    pub fn validate_permit(
        &mut self,
        permit: &ExecutionPermit,
        source_generation: u32,
        now: u64,
        instant: Instant,
    ) -> Result<(), &'static str> {
        if permit.instance != self.instance
            || permit.epoch != self.input_epoch
            || source_generation != permit.source_generation
            || instant >= permit.until
        {
            return Err("revoked");
        }
        self.authorize(&permit.grant, &permit.capability, now, instant)
            .map(|_| ())
    }
}
