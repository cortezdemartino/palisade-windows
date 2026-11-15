//! Canonical docs/48 v2 verification. A verified signature alone is never authority.
use crate::grant::{CAPABILITIES, decode, fingerprint, mxid, required_endpoint, token};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
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
pub(crate) fn track(v: &str) -> bool {
    v.strip_prefix("TR_").is_some_and(|v| {
        (4..=64).contains(&v.len()) && v.bytes().all(|c| c.is_ascii_alphanumeric())
    })
}
pub(crate) fn room(v: &str) -> bool {
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
struct GrantOperation {
    cancelled: Arc<AtomicBool>,
    canonical: Vec<u8>,
    generation: u64,
    nonce: String,
}
pub struct SessionGate {
    issuer: [u8; 32],
    binding: Binding,
    approved_at: Option<u64>,
    deadline: Option<u64>,
    minimum_generation: u64,
    highest_grant: u64,
    new_input_blocked: bool,
    fresh_input_generation: Option<u64>,
    staged_input: BTreeMap<String, StreamGrant>,
    grant_operations: BTreeMap<String, GrantOperation>,
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
    resource_peer_verified: bool,
    input_age: Option<(crate::input_age::InputAgeGuard, Instant)>,
    executor: Option<(crate::executor::ExecutorIdentity, bool)>,
    cleanup_nonce: u64,
    cleanup_required: Option<u64>,
}
pub struct ExecutionPermit {
    instance: u64,
    epoch: u64,
    grant: String,
    capability: String,
    source_generation: u32,
    until: Instant,
    action: crate::input::Action,
    executor: Option<crate::executor::ExecutorIdentity>,
}
/// Native-owned setup witness. It never authorizes an input operation and cannot
/// be constructed or deserialized from a page or a transport packet.
pub struct PeerPermit {
    instance: u64,
    input_epoch: u64,
    grant: String,
    generation: u64,
    capability: String,
}
pub(crate) enum InputOffer {
    Existing,
    Additional,
    Replacement,
}
impl ExecutionPermit {
    pub(crate) fn executor_registration(&self) -> Option<(crate::executor::ExecutorIdentity, u64)> {
        self.executor.map(|id| (id, self.instance))
    }
    pub fn matches(&self, action: &crate::input::Action) -> bool {
        self.action == *action
    }
    /// Created only after native authorization; never supplied by a packet.
    pub(crate) fn owner(&self) -> (u64, String) {
        (self.instance, self.grant.clone())
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
            minimum_generation,
            highest_grant: minimum_generation,
            new_input_blocked: false,
            fresh_input_generation: None,
            staged_input: BTreeMap::new(),
            grant_operations: BTreeMap::new(),
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
            resource_peer_verified: false,
            input_age: None,
            executor: None,
            cleanup_nonce: 0,
            cleanup_required: None,
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
    fn is_revoked(&self, id: &str) -> bool {
        self.revoked.contains(id)
            || self
                .grant_operations
                .get(id)
                .is_some_and(|operation| operation.cancelled.load(Ordering::SeqCst))
    }
    fn observed_generation(&self) -> u64 {
        self.grant_operations
            .values()
            .filter(|op| op.cancelled.load(Ordering::SeqCst))
            .fold(self.highest_grant, |highest, op| highest.max(op.generation))
    }
    pub(crate) fn bind_operation(
        &mut self,
        id: &str,
        cancelled: Arc<AtomicBool>,
        now: u64,
    ) -> Result<(), &'static str> {
        let grant = self.pending_grant(id, now)?;
        self.bind_verified_offer(&grant, cancelled)
    }
    pub(crate) fn bind_offered_operation(
        &mut self,
        signed: &SignedStreamGrant,
        cancelled: Arc<AtomicBool>,
        now: u64,
    ) -> Result<(), &'static str> {
        self.clock(now)?;
        verify_grant(
            signed,
            &self.issuer,
            &self.binding,
            now,
            self.deadline,
            self.minimum_generation,
        )?;
        let grant = &signed.grant;
        if self.approved_at.is_some_and(|a| a != grant.approved_at)
            || self.is_revoked(&grant.grant_id)
        {
            return Err("revoked");
        }
        if self
            .grants
            .get(&grant.grant_id)
            .is_some_and(|known| known.canonical() != grant.canonical())
        {
            return Err("binding-mismatch");
        }
        if !self.grants.contains_key(&grant.grant_id) && is_input(&grant.capability) {
            self.classify_input_offer(signed, now)?;
        }
        if self
            .grants
            .values()
            .any(|old| old.grant_id != grant.grant_id && old.nonce == grant.nonce)
            || self
                .grant_operations
                .iter()
                .any(|(id, op)| id != &grant.grant_id && op.nonce == grant.nonce)
        {
            return Err("revoked");
        }
        self.bind_verified_offer(grant, cancelled)
    }
    fn bind_verified_offer(
        &mut self,
        grant: &StreamGrant,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), &'static str> {
        let id = &grant.grant_id;
        // A retry of a completed consent cannot replace the permission's owner
        // with a later RPC's cancellation flag.
        if self.consent.contains(id) {
            return Ok(());
        }
        if let Some(existing) = self.grant_operations.get(id) {
            if existing.canonical != grant.canonical() {
                return Err("binding-mismatch");
            }
            return if Arc::ptr_eq(&existing.cancelled, &cancelled) {
                if existing.cancelled.load(Ordering::SeqCst) {
                    Err("revoked")
                } else {
                    Ok(())
                }
            } else {
                Err("operation-pending")
            };
        }
        if self.grant_operations.len() >= 128 {
            return Err("not-granted");
        }
        let stopped = cancelled.load(Ordering::SeqCst);
        self.grant_operations.insert(
            id.to_owned(),
            GrantOperation {
                cancelled,
                canonical: grant.canonical(),
                generation: grant.generation,
                nonce: grant.nonce.clone(),
            },
        );
        if stopped { Err("revoked") } else { Ok(()) }
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
                // Capabilities in one current service view may have been
                // issued at different times. The initial trusted floor is
                // independent of iteration order and the reporting high-water.
                self.minimum_generation
            },
        )?;
        let g = &signed.grant;
        if self.is_revoked(&g.grant_id) || self.approved_at.is_some_and(|a| a != g.approved_at) {
            return Err("revoked");
        }
        if let Some(previous) = self.grants.get(&g.grant_id) {
            if previous.canonical() != g.canonical() {
                return Err("binding-mismatch");
            }
            return Ok(());
        }
        // A previously unseen input capability is not a way around terminal
        // Stop. Fresh acquisition needs its own cleanup/consent/peer workflow;
        // this initial-view compatibility fix deliberately does not supply it.
        if is_input(&g.capability) && self.new_input_blocked {
            return Err("revoked");
        }
        if self.grants.len() >= 128
            || self
                .grants
                .values()
                .filter(|v| !self.is_revoked(&v.grant_id))
                .count()
                >= 8
            || self.grants.values().any(|v| v.capability == g.capability)
        {
            return Err("not-granted");
        }
        self.approved_at = Some(g.approved_at);
        self.deadline = Some(g.session_deadline);
        self.highest_grant = self.highest_grant.max(g.generation);
        self.grants.insert(g.grant_id.clone(), g.clone());
        Ok(())
    }
    /// Retain a verified canceled offer as inactive history. Cancellation cannot
    /// be erased by resubmitting the same signed ID under a new request.
    pub(crate) fn reject_offer(
        &mut self,
        signed: &SignedStreamGrant,
        now: u64,
    ) -> Result<(), &'static str> {
        self.clock(now)?;
        verify_grant(
            signed,
            &self.issuer,
            &self.binding,
            now,
            self.deadline,
            self.minimum_generation,
        )?;
        let g = &signed.grant;
        if self.approved_at.is_some_and(|a| a != g.approved_at) {
            return Err("binding-mismatch");
        }
        if let Some(known) = self.grants.get(&g.grant_id) {
            if known.canonical() != g.canonical() {
                return Err("binding-mismatch");
            }
            // A canceled ACK retry cannot revoke an earlier completed consent,
            // nor can a duplicate request take ownership of another prompt.
            if !self.consent.contains(&g.grant_id)
                && !self.grant_operations.contains_key(&g.grant_id)
            {
                self.revoke(&g.grant_id);
            }
            return Ok(());
        }
        if self.grants.len() >= 128 || self.grants.values().any(|old| old.nonce == g.nonce) {
            return Err("not-granted");
        }
        self.approved_at = Some(g.approved_at);
        self.deadline = Some(g.session_deadline);
        self.highest_grant = self.highest_grant.max(g.generation);
        self.grants.insert(g.grant_id.clone(), g.clone());
        self.revoked.insert(g.grant_id.clone());
        self.staged_input.remove(&g.grant_id);
        Ok(())
    }
    /// A fresh signed input record can be observed while cleanup/consent is
    /// pending, without becoming an admitted grant. This keeps valid independent
    /// resource leases alive while the service publishes a replacement request.
    fn stage_input(&mut self, signed: &SignedStreamGrant, now: u64) -> Result<(), &'static str> {
        self.clock(now)?;
        let grant = &signed.grant;
        verify_grant(
            signed,
            &self.issuer,
            &self.binding,
            now,
            self.deadline,
            self.minimum_generation,
        )?;
        if !is_input(&grant.capability)
            || self.approved_at != Some(grant.approved_at)
            || self.is_revoked(&grant.grant_id)
            || self.grants.contains_key(&grant.grant_id)
            || (grant.generation <= self.observed_generation()
                && self.fresh_input_generation != Some(grant.generation))
            || self.grants.values().any(|known| known.nonce == grant.nonce)
            || self
                .grant_operations
                .iter()
                .any(|(id, op)| id != &grant.grant_id && op.nonce == grant.nonce)
            || self.grants.values().any(|known| {
                known.capability == grant.capability && known.generation == grant.generation
            })
        {
            return Err("revoked");
        }
        if let Some(previous) = self.staged_input.get(&grant.grant_id) {
            return if previous.canonical() == grant.canonical() {
                Ok(())
            } else {
                Err("binding-mismatch")
            };
        }
        if let Some(previous) = self
            .staged_input
            .values()
            .find(|known| known.capability == grant.capability)
        {
            if previous.generation >= grant.generation {
                return Err("stale-generation");
            }
            let old_id = previous.grant_id.clone();
            self.revoke(&old_id);
            if self.ended {
                return Err("not-granted");
            }
        }
        if self.staged_input.len() >= 2 {
            return Err("not-granted");
        }
        self.staged_input
            .insert(grant.grant_id.clone(), grant.clone());
        Ok(())
    }
    /// Trusted runtime only: call after the old input worker has positively
    /// joined. A signature, new generation and settled gate are required; fresh
    /// native consent, lease coverage and certificate admission still follow.
    pub fn reacquire_input(
        &mut self,
        signed: &SignedStreamGrant,
        now: u64,
    ) -> Result<(), &'static str> {
        self.stage_input(signed, now)?;
        let grant = &signed.grant;
        let same_cohort = self.fresh_input_generation == Some(grant.generation);
        if !self.new_input_blocked
            || (!same_cohort
                && (self.executor.is_some()
                    || self.cleanup_required.is_some()
                    || self.peer_verified))
            || self.grants.len() >= 128
            || self.grants.values().any(|known| {
                known.capability == grant.capability && !self.is_revoked(&known.grant_id)
            })
        {
            return Err("cleanup-pending");
        }
        if self
            .fresh_input_generation
            .is_some_and(|generation| generation != grant.generation)
        {
            return Err("stale-generation");
        }
        if !same_cohort {
            self.input_epoch = self
                .input_epoch
                .checked_add(1)
                .ok_or("input-epoch-exhausted")?;
            self.input_age = None;
        }
        self.fresh_input_generation = Some(grant.generation);
        self.highest_grant = self.highest_grant.max(grant.generation);
        self.staged_input.remove(&grant.grant_id);
        self.grants.insert(grant.grant_id.clone(), grant.clone());
        Ok(())
    }
    pub(crate) fn needs_input_reacquisition(&self, signed: &SignedStreamGrant) -> bool {
        self.new_input_blocked
            && is_input(&signed.grant.capability)
            && !self.grants.contains_key(&signed.grant.grant_id)
    }
    pub(crate) fn classify_input_offer(
        &mut self,
        signed: &SignedStreamGrant,
        now: u64,
    ) -> Result<InputOffer, &'static str> {
        self.clock(now)?;
        verify_grant(
            signed,
            &self.issuer,
            &self.binding,
            now,
            self.deadline,
            self.minimum_generation,
        )?;
        let g = &signed.grant;
        if !is_input(&g.capability)
            || self.is_revoked(&g.grant_id)
            || self.approved_at.is_some_and(|a| a != g.approved_at)
        {
            return Err("revoked");
        }
        if let Some(old) = self.grants.get(&g.grant_id) {
            return if old.canonical() == g.canonical() {
                Ok(InputOffer::Existing)
            } else {
                Err("binding-mismatch")
            };
        }
        if self.grants.values().any(|old| old.nonce == g.nonce) {
            return Err("revoked");
        }
        if self
            .grant_operations
            .iter()
            .any(|(id, op)| id != &g.grant_id && op.nonce == g.nonce)
        {
            return Err("revoked");
        }
        if self.fresh_input_generation == Some(g.generation) {
            return Ok(InputOffer::Additional);
        }
        if self
            .grant_operations
            .values()
            .any(|op| op.cancelled.load(Ordering::SeqCst) && op.generation >= g.generation)
        {
            return Err("stale-generation");
        }
        if g.generation > self.observed_generation()
            && (self.new_input_blocked || self.grants.values().any(|old| is_input(&old.capability)))
        {
            return Ok(InputOffer::Replacement);
        }
        if !self.new_input_blocked
            && self
                .grants
                .values()
                .filter(|old| is_input(&old.capability))
                .map(|old| old.generation)
                .min()
                .is_none_or(|generation| g.generation <= generation)
            && !self
                .grants
                .values()
                .any(|old| old.capability == g.capability)
        {
            return Ok(InputOffer::Additional);
        }
        Err("stale-generation")
    }
    pub(crate) fn consented_grant(
        &mut self,
        id: &str,
        now: u64,
    ) -> Result<Option<StreamGrant>, &'static str> {
        let grant = self.pending_grant(id, now)?;
        Ok(self.consent.contains(id).then_some(grant))
    }
    pub(crate) fn lease_current(&mut self, now: u64, instant: Instant) -> Result<(), &'static str> {
        self.clock(now)?;
        if self.lease_until.is_none() || self.lease.is_none() {
            return Err("lease-required");
        }
        if self.lease_until.is_some_and(|until| instant >= until)
            || self
                .lease
                .as_ref()
                .is_some_and(|lease| now >= lease.expires_at)
        {
            self.stop();
            return Err("expired");
        }
        Ok(())
    }
    /// Service authority for connecting a resource transport only. This cannot
    /// authorize clipboard/filesystem actions: those retain authorize + native
    /// local consent and per-operation confirmation in their resource brokers.
    pub(crate) fn resource_transport_grant(
        &mut self,
        id: &str,
        now: u64,
        instant: Instant,
    ) -> Result<StreamGrant, &'static str> {
        self.tick(now, instant)?;
        self.lease_current(now, instant)?;
        let grant = self.pending_grant(id, now)?;
        let lease = self.lease.as_ref().ok_or("lease-required")?;
        if !matches!(
            grant.capability.as_str(),
            "clipboard_to_host" | "clipboard_to_guest" | "file_to_host" | "file_to_guest"
        ) || grant.endpoint.is_some()
            || !lease.grants.iter().any(|g| g == id)
            || grant.generation > lease.generation
        {
            return Err("not-granted");
        }
        Ok(grant)
    }
    pub(crate) fn input_cohort_decided(
        &mut self,
        current: &[StreamGrant],
        selected: &str,
        now: u64,
    ) -> Result<bool, &'static str> {
        let selected = self.pending_grant(selected, now)?;
        if !current
            .iter()
            .any(|g| g.canonical() == selected.canonical())
        {
            return Err("not-granted");
        }
        for record in current
            .iter()
            .filter(|g| is_input(&g.capability) && g.generation == selected.generation)
        {
            if !is_input(&record.capability) {
                return Err("invalid-message");
            }
            let known = self.grants.get(&record.grant_id).ok_or("consent-pending")?;
            if known.canonical() != record.canonical() {
                return Err("binding-mismatch");
            }
            if !self.consent.contains(&record.grant_id) && !self.is_revoked(&record.grant_id) {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub(crate) fn extends_fresh_input_batch(&self, signed: &SignedStreamGrant) -> bool {
        self.needs_input_reacquisition(signed)
            && self.fresh_input_generation == Some(signed.grant.generation)
    }
    pub(crate) fn input_generation_ended(&self) -> bool {
        self.new_input_blocked && self.fresh_input_generation.is_none()
    }
    pub(crate) fn validate_worker_epoch(&self, epoch: Option<u64>) -> Result<(), &'static str> {
        if epoch != Some(self.input_epoch) || self.input_generation_ended() {
            Err("revoked")
        } else {
            Ok(())
        }
    }
    /// One native execution generation per gate. Reserve before startup/READY;
    /// there is no replacement while a previous generation remains registered.
    pub fn reserve_executor(
        &mut self,
    ) -> Result<crate::executor::ExecutorReservation, &'static str> {
        if self.ended {
            return Err("revoked");
        }
        if self.executor.is_some() || self.cleanup_required.is_some() {
            return Err("cleanup-pending");
        }
        if !self
            .grants
            .values()
            .any(|g| is_input(&g.capability) && !self.is_revoked(&g.grant_id))
        {
            return Err("not-granted");
        }
        let identity = crate::executor::ExecutorIdentity::fresh()?;
        self.executor = Some((identity, false));
        Ok(crate::executor::ExecutorReservation {
            identity,
            instance: self.instance,
        })
    }
    /// The still-linear reservation proves no Executor can have consumed it.
    /// Once a registered permit exists, cancellation is no longer sufficient.
    pub fn cancel_unstarted_executor(
        &mut self,
        reservation: crate::executor::ExecutorReservation,
    ) -> Result<(), &'static str> {
        if reservation.instance != self.instance
            || self.executor != Some((reservation.identity, false))
            || self.cleanup_required.is_some()
        {
            return Err("executor-binding-mismatch");
        }
        let next_epoch = self
            .input_epoch
            .checked_add(1)
            .ok_or("input-epoch-exhausted")?;
        self.executor = None;
        self.input_epoch = next_epoch;
        Ok(())
    }
    pub fn require_input_cleanup(
        &mut self,
    ) -> Result<crate::executor::InputCleanupRequirement, &'static str> {
        if self
            .grants
            .values()
            .any(|g| is_input(&g.capability) && !self.is_revoked(&g.grant_id))
        {
            return Err("input-still-authorized");
        }
        if self.cleanup_required.is_some() {
            return Err("cleanup-pending");
        }
        let (identity, _) = self.executor.ok_or("executor-required")?;
        let nonce = self
            .cleanup_nonce
            .checked_add(1)
            .ok_or("cleanup-epoch-exhausted")?;
        self.cleanup_nonce = nonce;
        self.cleanup_required = Some(nonce);
        Ok(crate::executor::InputCleanupRequirement {
            identity,
            instance: self.instance,
            nonce,
        })
    }
    pub fn accept_input_cleanup(
        &mut self,
        receipt: crate::executor::InputCleanupReceipt,
    ) -> Result<(), &'static str> {
        let (identity, _) = self.executor.ok_or("cleanup-stale")?;
        let nonce = self.cleanup_required.ok_or("cleanup-stale")?;
        if !receipt.matches(identity, self.instance, nonce) {
            return Err("cleanup-stale");
        }
        let next_epoch = self
            .input_epoch
            .checked_add(1)
            .ok_or("input-epoch-exhausted")?;
        self.executor = None;
        self.cleanup_required = None;
        self.input_epoch = next_epoch;
        Ok(())
    }
    pub fn consent(&mut self, grant_id: &str, now: u64) -> Result<(), &'static str> {
        self.clock(now)?;
        if !self.grants.contains_key(grant_id) || self.is_revoked(grant_id) {
            return Err("not-granted");
        }
        if self.cleanup_required.is_some()
            && self
                .grants
                .get(grant_id)
                .is_some_and(|g| is_input(&g.capability))
        {
            return Err("cleanup-pending");
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
        if self.is_revoked(&signed.grant.grant_id) {
            let canonical = self
                .grants
                .get(&signed.grant.grant_id)
                .map(|g| g.canonical())
                .or_else(|| {
                    self.grant_operations
                        .get(&signed.grant.grant_id)
                        .map(|op| op.canonical.clone())
                })
                .ok_or("revoked")?;
            if canonical != signed.grant.canonical() {
                return Err("binding-mismatch");
            }
            signature(&signed.grant, &signed.signature, &self.issuer)
        } else if is_input(&signed.grant.capability)
            && !self.grants.contains_key(&signed.grant.grant_id)
            && self.fresh_input_generation != Some(signed.grant.generation)
            && self.grant_operations.values().any(|op| {
                op.cancelled.load(Ordering::SeqCst) && op.generation >= signed.grant.generation
            })
        {
            // A locally canceled acquisition can remain visible until the
            // service processes revocation. Validate it without restoring it or
            // ending independently valid resources. Only a higher fresh signed
            // generation can enter a new native acquisition.
            self.reject_offer(signed, now)
        } else if self.needs_input_reacquisition(signed)
            || (is_input(&signed.grant.capability)
                && !self.grants.contains_key(&signed.grant.grant_id)
                && self
                    .grants
                    .values()
                    .any(|known| known.capability == signed.grant.capability))
        {
            self.stage_input(signed, now)
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
        if if is_input(capability) {
            !self.peer_verified
        } else {
            !self.resource_peer_verified
        } {
            return Err("peer-unverified");
        }
        self.authorize_grant(grant_id, capability, now, instant)
    }
    fn authorize_grant(
        &mut self,
        grant_id: &str,
        capability: &str,
        now: u64,
        instant: Instant,
    ) -> Result<&StreamGrant, &'static str> {
        self.clock(now)?;
        let until = self.lease_until.ok_or("lease-required")?;
        if instant >= until {
            self.stop();
            return Err("expired");
        }
        let g = self.grants.get(grant_id).ok_or("not-granted")?;
        let l = self.lease.as_ref().ok_or("not-granted")?;
        if self.is_revoked(grant_id) {
            return Err("revoked");
        }
        if !self.consent.contains(grant_id)
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
    /// Consent and a covering signed lease permit connection setup, never input.
    /// In particular, setup must not temporarily set `peer_verified`.
    pub fn prepare_peer(
        &mut self,
        grant_id: &str,
        now: u64,
        instant: Instant,
    ) -> Result<PeerPermit, &'static str> {
        self.tick(now, instant)?;
        if self.input_generation_ended() {
            return Err("revoked");
        }
        if self.peer_verified {
            return Err("peer-already-bound");
        }
        let capability = self
            .grants
            .get(grant_id)
            .ok_or("not-granted")?
            .capability
            .clone();
        if !is_input(&capability) || self.cleanup_required.is_some() {
            return Err("input-setup-required");
        }
        let generation = self
            .authorize_grant(grant_id, &capability, now, instant)?
            .generation;
        Ok(PeerPermit {
            instance: self.instance,
            input_epoch: self.input_epoch,
            grant: grant_id.into(),
            generation,
            capability,
        })
    }
    pub fn validate_peer_setup(
        &mut self,
        permit: &PeerPermit,
        now: u64,
        instant: Instant,
    ) -> Result<(), &'static str> {
        self.tick(now, instant)?;
        if permit.instance != self.instance || permit.input_epoch != self.input_epoch {
            return Err("stale-peer");
        }
        if self
            .authorize_grant(&permit.grant, &permit.capability, now, instant)?
            .generation
            != permit.generation
        {
            return Err("stale-peer");
        }
        Ok(())
    }
    pub(crate) fn validate_peer_cohort(
        &mut self,
        permit: &PeerPermit,
        current: &[StreamGrant],
        now: u64,
        instant: Instant,
    ) -> Result<(), &'static str> {
        self.validate_peer_setup(permit, now, instant)?;
        if !self.input_cohort_decided(current, &permit.grant, now)? {
            return Err("consent-pending");
        }
        Ok(())
    }
    /// Only the owning native peer coordinator supplies an observed certificate.
    pub fn admit_peer(
        &mut self,
        permit: &PeerPermit,
        observed: &str,
        now: u64,
        instant: Instant,
    ) -> Result<(), &'static str> {
        self.validate_peer_setup(permit, now, instant)?;
        self.verify_peer(observed)
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
        self.check_sequence(grant_id, sequence, instant, true)
    }
    fn check_sequence(
        &mut self,
        grant_id: &str,
        sequence: u32,
        instant: Instant,
        rate_limited: bool,
    ) -> Result<(), &'static str> {
        if self.sequences.get(grant_id).is_some_and(|&n| sequence <= n) {
            return Err("replay");
        }
        if rate_limited {
            self.rate.1 = (self.rate.1 + instant.duration_since(self.rate.0).as_secs_f64() * 120.0)
                .min(240.0);
            self.rate.0 = instant;
            if self.rate.1 < 1.0 {
                self.stop();
                return Err("rate-limited");
            }
            self.rate.1 -= 1.0;
        }
        self.sequences.insert(grant_id.into(), sequence);
        Ok(())
    }
    pub fn revoke(&mut self, id: &str) {
        if self.revoked.contains(id) {
            return;
        }
        if let Some(staged) = self.staged_input.remove(id) {
            if self.grants.len() >= 128 {
                self.stop();
                return;
            }
            self.highest_grant = self.highest_grant.max(staged.generation);
            self.grants.insert(id.into(), staged);
        }
        self.epoch = self.epoch.checked_add(1).expect("endpoint epoch exhausted");
        if self
            .grants
            .get(id)
            .is_some_and(|grant| is_input(&grant.capability))
            && self.consent.contains(id)
        {
            self.new_input_blocked = true;
            self.fresh_input_generation = None;
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
        self.new_input_blocked = true;
        self.peer_verified = false;
        self.fresh_input_generation = None;
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
        self.new_input_blocked = true;
        self.epoch = self.epoch.checked_add(1).expect("endpoint epoch exhausted");
        self.input_epoch = self
            .input_epoch
            .checked_add(1)
            .expect("input epoch exhausted");
        self.ended = true;
        self.peer_verified = false;
        self.resource_peer_verified = false;
        self.staged_input.clear();
        self.grant_operations.clear();
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
        self.observed_generation()
    }
    pub(crate) fn input_epoch(&self) -> u64 {
        self.input_epoch
    }
    pub fn verify_peer(&mut self, fingerprint: &str) -> Result<(), &'static str> {
        if self.ended || fingerprint != self.binding.helper_transport {
            self.stop();
            return Err("peer-mismatch");
        }
        if self.input_age.is_none() {
            self.input_age = Some((crate::input_age::InputAgeGuard::new(), Instant::now()));
        }
        self.peer_verified = true;
        self.resource_peer_verified = true;
        Ok(())
    }
    pub fn pending_grant(&mut self, id: &str, now: u64) -> Result<StreamGrant, &'static str> {
        self.clock(now)?;
        if self.is_revoked(id) {
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
        let cancelled: Vec<_> = self
            .grant_operations
            .iter()
            .filter(|(id, operation)| {
                operation.cancelled.load(Ordering::SeqCst) && !self.revoked.contains(*id)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in cancelled {
            self.revoke(&id);
        }
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
        if matches!(message.action, crate::input::Action::Unicode { .. }) {
            return Err("invalid-message");
        }
        if message.session != self.binding.session {
            return Err("binding-mismatch");
        }
        let capability = match message.action {
            crate::input::Action::Move { .. }
            | crate::input::Action::Button { .. }
            | crate::input::Action::Wheel { .. } => "control_mouse",
            crate::input::Action::ReleaseAll => self
                .grants
                .get(&message.grant)
                .filter(|g| is_input(&g.capability))
                .map(|g| g.capability.as_str())
                .ok_or("not-granted")?,
            _ => "control_keyboard",
        }
        .to_owned();
        // Releases keep all authority and replay checks, but cannot be starved
        // by the ordinary input budget. The executor still enforces ownership.
        let release = matches!(
            message.action,
            crate::input::Action::ReleaseAll
                | crate::input::Action::Key { down: false, .. }
                | crate::input::Action::Button { down: false, .. }
        );
        if self
            .authorize(&message.grant, &capability, now, instant)?
            .generation
            != message.generation
        {
            return Err("stale-generation");
        }
        self.check_sequence(&message.grant, message.sequence, instant, !release)?;
        if self.cleanup_required.is_some() {
            return Err("cleanup-pending");
        }
        if let Some((_, admitted)) = self.executor.as_mut() {
            *admitted = true;
        }
        Ok(ExecutionPermit {
            instance: self.instance,
            epoch: self.input_epoch,
            grant: message.grant.clone(),
            capability,
            source_generation,
            until: instant + Duration::from_millis(100),
            action: message.action.clone(),
            executor: self.executor.map(|(id, _)| id),
        })
    }
    /// Profile2 authority and sequence checks precede age observation so forged
    /// traffic cannot poison the peer baseline. Repeated verify_peer never
    /// resets it. This gate supports one peer association for its whole lifetime.
    pub fn permit_profile2(
        &mut self,
        message: &crate::input::Profile2Message,
        source_generation: u32,
        now: u64,
        instant: Instant,
    ) -> Result<ExecutionPermit, &'static str> {
        self.permit_profile2_at(message, source_generation, now, instant, instant)
    }
    fn permit_profile2_at(
        &mut self,
        message: &crate::input::Profile2Message,
        source_generation: u32,
        now: u64,
        instant: Instant,
        ingress: Instant,
    ) -> Result<ExecutionPermit, &'static str> {
        let permit = self.permit(&message.message, source_generation, now, instant)?;
        let (age, start) = self.input_age.as_mut().ok_or("peer-unverified")?;
        let arrival = ingress
            .checked_duration_since(*start)
            .ok_or("invalid-message")?;
        let arrival = u64::try_from(arrival.as_millis()).map_err(|_| "invalid-message")?;
        age.observe(message.sender_ms, arrival, message.release())?;
        Ok(permit)
    }
    /// Exact runtime ingress path, including locally measured queue residence.
    /// A valid stale message consumes sequence/age state, but dispatches nothing.
    /// Owned releases remain possible; they still require current authority.
    pub fn permit_packet(
        &mut self,
        raw: &str,
        profile: crate::input::InputProfile,
        source_generation: u32,
        now: u64,
        timing: (Instant, Instant),
    ) -> Result<(ExecutionPermit, crate::input::Message, Instant), &'static str> {
        let (ingress, instant) = timing;
        let (permit, message) = match crate::input::parse(raw) {
            Ok(message) => (
                self.permit(&message, source_generation, now, instant)?,
                message,
            ),
            Err(error) => match profile {
                crate::input::InputProfile::Legacy => return Err(error),
                crate::input::InputProfile::Profile2 => {
                    let m = crate::input::parse_profile2(raw)?;
                    (
                        self.permit_profile2_at(&m, source_generation, now, instant, ingress)?,
                        m.message,
                    )
                }
            },
        };
        let release = matches!(
            message.action,
            crate::input::Action::Key { down: false, .. }
                | crate::input::Action::Button { down: false, .. }
                | crate::input::Action::ReleaseAll
        );
        let until = if release { instant } else { ingress } + Duration::from_millis(100);
        if instant < ingress || instant >= until {
            return Err("stale-input");
        }
        Ok((permit, message, until))
    }
    /// Semantic input authority for the native scheduler. This cannot be made
    /// by renderer metadata and does not depend on a routine lease's new bytes.
    pub fn input_authority(
        &mut self,
        source: &crate::executor::Source,
        now: u64,
        instant: Instant,
    ) -> Result<crate::cooperative::InputAuthority, &'static str> {
        source.point(0., 0.)?;
        self.tick(now, instant)?;
        if !self.input_active(now, instant) {
            return Err("not-granted");
        }
        let grants = self
            .grants
            .values()
            .filter(|g| is_input(&g.capability))
            .map(|g| {
                (
                    g.grant_id.clone(),
                    g.generation,
                    self.consent.contains(&g.grant_id),
                    self.lease
                        .as_ref()
                        .is_some_and(|l| l.grants.contains(&g.grant_id)),
                )
            })
            .collect();
        Ok(crate::cooperative::InputAuthority {
            instance: self.instance,
            input_epoch: self.input_epoch,
            session: self.binding.session.clone(),
            source: source.clone(),
            grants,
        })
    }
    /// A cooperative barrier uses an already-negotiated data plane and either
    /// active input grant. It never becomes an executable input Action.
    pub fn verify_barrier(
        &mut self,
        message: &crate::cooperative::Barrier,
        now: u64,
        timing: (Instant, Instant),
    ) -> Result<crate::cooperative::VerifiedBarrier, &'static str> {
        let (ingress, instant) = timing;
        message.validate()?;
        if message.session != self.binding.session {
            return Err("binding-mismatch");
        }
        let capability = self
            .grants
            .get(&message.grant)
            .filter(|g| is_input(&g.capability))
            .map(|g| g.capability.clone())
            .ok_or("not-granted")?;
        if self
            .authorize(&message.grant, &capability, now, instant)?
            .generation
            != message.r#gen
        {
            return Err("stale-generation");
        }
        self.check_sequence(&message.grant, message.seq, instant, false)?;
        let (age, start) = self.input_age.as_mut().ok_or("peer-unverified")?;
        let arrival = instant
            .checked_duration_since(*start)
            .ok_or("invalid-message")?;
        age.observe(
            message.t,
            u64::try_from(arrival.as_millis()).map_err(|_| "invalid-message")?,
            true,
        )?;
        // Like an owned release, a barrier gets a fresh dispatch window after
        // current authorization/age checks. Preserve true ingress separately:
        // the scheduler must still reject anything received before readiness.
        if instant < ingress {
            return Err("invalid-message");
        }
        Ok(crate::cooperative::VerifiedBarrier {
            instance: self.instance,
            input_epoch: self.input_epoch,
            session: self.binding.session.clone(),
            epoch: message.epoch,
            ingress,
            until: instant + Duration::from_millis(100),
        })
    }
    pub fn validate_permit(
        &mut self,
        permit: &ExecutionPermit,
        source_generation: u32,
        now: u64,
        instant: Instant,
    ) -> Result<(), &'static str> {
        // Per-request cancellation is lock-free; fold sibling cancellation
        // before every primitive rather than waiting for the worker's next tick.
        self.tick(now, instant)?;
        if permit.instance != self.instance
            || permit.executor != self.executor.map(|(id, _)| id)
            || self.cleanup_required.is_some()
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

#[cfg(test)]
mod request_tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use ed25519_dalek::{Signer, SigningKey};
    fn setup() -> (SessionGate, SigningKey, SignedStreamGrant, u64, Instant) {
        let f: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/assist-control-v2.json"
        ))
        .unwrap();
        let mut grant: StreamGrant =
            serde_json::from_value(f["grants"]["valid"][1]["signed"]["grant"].clone()).unwrap();
        let key = SigningKey::from_bytes(&[73; 32]);
        grant.generation = 10;
        let now = f["now"].as_u64().unwrap();
        let binding = Binding {
            session: grant.session.clone(),
            room: grant.room.clone(),
            stream: grant.stream.clone(),
            publisher_account: grant.publisher_account.clone(),
            publisher_device: grant.publisher_device.clone(),
            helper_account: grant.helper_account.clone(),
            helper_device: grant.helper_device.clone(),
            helper_transport: grant.helper_transport.clone(),
            endpoint: grant.endpoint.clone().unwrap(),
        };
        let t = Instant::now();
        (
            SessionGate::new(key.verifying_key().to_bytes(), binding, now, 3, t),
            key.clone(),
            sign(grant, &key),
            now,
            t,
        )
    }
    fn sign(grant: StreamGrant, key: &SigningKey) -> SignedStreamGrant {
        SignedStreamGrant {
            signature: URL_SAFE_NO_PAD.encode(key.sign(&grant.canonical()).to_bytes()),
            grant,
        }
    }
    #[test]
    fn resource_transport_accepts_only_the_four_signed_directions_without_os_consent() {
        for capability in [
            "clipboard_to_host",
            "clipboard_to_guest",
            "file_to_host",
            "file_to_guest",
            "pointer",
            "annotate",
        ] {
            let (mut gate, key, mouse, now, t) = setup();
            let mut grant = other(&mouse, &key, 10, capability).grant;
            grant.endpoint = None;
            let grant = sign(grant, &key);
            gate.admit(&grant, now).unwrap();
            let fixture: serde_json::Value = serde_json::from_str(include_str!(
                "../../../tests/fixtures/assist-control-v2.json"
            ))
            .unwrap();
            let mut lease: StreamLease =
                serde_json::from_value(fixture["leases"]["valid"][0]["signed"]["lease"].clone())
                    .unwrap();
            lease.generation = 10;
            lease.grants = vec![grant.grant.grant_id.clone()];
            let lease = SignedStreamLease {
                signature: URL_SAFE_NO_PAD
                    .encode(key.sign(&serde_json::to_vec(&lease).unwrap()).to_bytes()),
                lease,
            };
            gate.refresh(&lease, now, t, t).unwrap();
            assert_eq!(
                gate.resource_transport_grant(&grant.grant.grant_id, now, t)
                    .is_ok(),
                !matches!(capability, "pointer" | "annotate")
            );
            assert!(
                gate.consented_grant(&grant.grant.grant_id, now)
                    .unwrap()
                    .is_none()
            );
            assert!(
                gate.authorize(&grant.grant.grant_id, capability, now, t)
                    .is_err()
            );
        }
    }
    fn other(
        g: &SignedStreamGrant,
        key: &SigningKey,
        generation: u64,
        cap: &str,
    ) -> SignedStreamGrant {
        let mut grant = g.grant.clone();
        grant.grant_id = format!("grant_request_{}_{}", generation, cap);
        grant.nonce = format!("nonce_request_{}_{}", generation, cap);
        grant.generation = generation;
        grant.capability = cap.into();
        sign(grant, key)
    }
    #[test]
    fn consent_cohorts_wait_for_both_decisions_and_partial_denial_preserves_the_allowed_capability()
    {
        for fresh in [false, true] {
            for allow_mouse in [false, true] {
                let (mut gate, key, mouse, now, t) = setup();
                gate.admit(&mouse, now).unwrap();
                let (mouse, keyboard) = if fresh {
                    gate.suspend_input();
                    let mouse = other(&mouse, &key, 11, "control_mouse");
                    let keyboard = other(&mouse, &key, 11, "control_keyboard");
                    gate.reacquire_input(&mouse, now).unwrap();
                    let epoch = gate.input_epoch();
                    gate.reacquire_input(&keyboard, now).unwrap();
                    assert_eq!(gate.input_epoch(), epoch);
                    (mouse, keyboard)
                } else {
                    let keyboard = other(&mouse, &key, 10, "control_keyboard");
                    gate.admit(&keyboard, now).unwrap();
                    (mouse, keyboard)
                };
                let current = vec![mouse.grant.clone(), keyboard.grant.clone()];
                let (allowed, denied) = if allow_mouse {
                    (&mouse, &keyboard)
                } else {
                    (&keyboard, &mouse)
                };
                gate.consent(&allowed.grant.grant_id, now).unwrap();
                assert!(
                    !gate
                        .input_cohort_decided(&current, &allowed.grant.grant_id, now)
                        .unwrap()
                );
                let epoch = gate.input_epoch();
                gate.revoke(&denied.grant.grant_id);
                assert_eq!(gate.input_epoch(), epoch);
                assert!(!gate.input_generation_ended());
                assert!(
                    gate.input_cohort_decided(&current, &allowed.grant.grant_id, now)
                        .unwrap()
                );
                let fixture: serde_json::Value = serde_json::from_str(include_str!(
                    "../../../tests/fixtures/assist-control-v2.json"
                ))
                .unwrap();
                let mut lease: StreamLease = serde_json::from_value(
                    fixture["leases"]["valid"][0]["signed"]["lease"].clone(),
                )
                .unwrap();
                lease.generation = allowed.grant.generation;
                lease.grants = vec![allowed.grant.grant_id.clone()];
                let signed = SignedStreamLease {
                    signature: URL_SAFE_NO_PAD
                        .encode(key.sign(&serde_json::to_vec(&lease).unwrap()).to_bytes()),
                    lease,
                };
                gate.refresh(&signed, now, t, t).unwrap();
                let permit = gate.prepare_peer(&allowed.grant.grant_id, now, t).unwrap();
                gate.validate_peer_cohort(&permit, &current, now, t)
                    .unwrap();
                gate.admit_peer(&permit, &allowed.grant.helper_transport, now, t)
                    .unwrap();
                gate.authorize(&allowed.grant.grant_id, &allowed.grant.capability, now, t)
                    .unwrap();
            }
        }
    }
    #[test]
    fn a_second_fresh_member_preserves_a_prepared_permit_and_age_epoch() {
        let (mut gate, key, mouse, now, t) = setup();
        gate.admit(&mouse, now).unwrap();
        gate.suspend_input();
        let mouse = other(&mouse, &key, 11, "control_mouse");
        let keyboard = other(&mouse, &key, 11, "control_keyboard");
        gate.reacquire_input(&mouse, now).unwrap();
        gate.consent(&mouse.grant.grant_id, now).unwrap();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/assist-control-v2.json"
        ))
        .unwrap();
        let mut lease: StreamLease =
            serde_json::from_value(fixture["leases"]["valid"][0]["signed"]["lease"].clone())
                .unwrap();
        lease.generation = 11;
        lease.grants = vec![mouse.grant.grant_id.clone()];
        let lease = SignedStreamLease {
            signature: URL_SAFE_NO_PAD
                .encode(key.sign(&serde_json::to_vec(&lease).unwrap()).to_bytes()),
            lease,
        };
        gate.refresh(&lease, now, t, t).unwrap();
        let permit = gate.prepare_peer(&mouse.grant.grant_id, now, t).unwrap();
        let epoch = gate.input_epoch();
        assert!(matches!(
            gate.classify_input_offer(&keyboard, now).unwrap(),
            InputOffer::Additional
        ));
        gate.reacquire_input(&keyboard, now).unwrap();
        assert_eq!(gate.input_epoch(), epoch);
        gate.validate_peer_setup(&permit, now, t).unwrap();
        let current = vec![mouse.grant.clone(), keyboard.grant.clone()];
        assert_eq!(
            gate.validate_peer_cohort(&permit, &current, now, t),
            Err("consent-pending")
        );
        gate.revoke(&keyboard.grant.grant_id);
        gate.validate_peer_cohort(&permit, &current, now, t)
            .unwrap();
    }
    #[test]
    fn cancellation_binding_is_immutable_and_idle_tick_revokes_consent() {
        let (mut gate, _, mouse, now, t) = setup();
        gate.admit(&mouse, now).unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        gate.bind_operation(&mouse.grant.grant_id, flag.clone(), now)
            .unwrap();
        assert_eq!(
            gate.bind_operation(&mouse.grant.grant_id, Arc::new(AtomicBool::new(false)), now),
            Err("operation-pending")
        );
        gate.consent(&mouse.grant.grant_id, now).unwrap();
        let epoch = gate.input_epoch();
        flag.store(true, Ordering::SeqCst);
        assert!(gate.consented_grant(&mouse.grant.grant_id, now).is_err());
        gate.tick(now, t).unwrap();
        assert!(gate.input_epoch() > epoch);
        assert!(gate.input_generation_ended());
        assert!(gate.pending_grant(&mouse.grant.grant_id, now).is_err());
        assert!(
            gate.bind_operation(&mouse.grant.grant_id, Arc::new(AtomicBool::new(false)), now)
                .is_err()
        );
        let epoch = gate.input_epoch();
        gate.tick(now, t).unwrap();
        assert_eq!(gate.input_epoch(), epoch);
    }
    #[test]
    fn service_revocation_before_peer_ready_requires_positive_retirement_and_a_new_generation() {
        let (mut gate, key, mouse, now, _) = setup();
        gate.admit(&mouse, now).unwrap();
        gate.consent(&mouse.grant.grant_id, now).unwrap();
        assert!(!gate.input_generation_ended());
        gate.revoke(&mouse.grant.grant_id);
        assert!(gate.input_generation_ended());
        let same_generation = other(&mouse, &key, 10, "control_keyboard");
        assert!(gate.classify_input_offer(&same_generation, now).is_err());
        let fresh = other(&mouse, &key, 11, "control_keyboard");
        assert!(matches!(
            gate.classify_input_offer(&fresh, now).unwrap(),
            InputOffer::Replacement
        ));
        gate.reacquire_input(&fresh, now).unwrap();
        assert!(!gate.input_generation_ended());
        assert!(
            gate.consented_grant(&fresh.grant.grant_id, now)
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn pre_admission_cancellation_is_immediate_immutable_and_does_not_restore_lease_observations() {
        let (mut gate, key, mouse, now, t) = setup();
        gate.admit(&mouse, now).unwrap();
        let fresh = other(&mouse, &key, 12, "control_mouse");
        let flag = Arc::new(AtomicBool::new(false));
        gate.bind_offered_operation(&fresh, flag.clone(), now)
            .unwrap();
        assert_eq!(gate.highest_generation(), 10); // binding alone is not admission
        assert!(gate.pending_grant(&fresh.grant.grant_id, now).is_err());
        flag.store(true, Ordering::SeqCst);
        assert_eq!(gate.highest_generation(), 12); // no tick/cleanup scheduling gap
        let lower = other(&mouse, &key, 11, "control_keyboard");
        assert!(gate.classify_input_offer(&lower, now).is_err());
        assert!(
            gate.bind_offered_operation(&fresh, Arc::new(AtomicBool::new(false)), now)
                .is_err()
        );
        let sibling = other(&fresh, &key, 12, "control_keyboard");
        for record in [&fresh, &sibling] {
            gate.observe_grant(record, now).unwrap();
            assert!(gate.pending_grant(&record.grant.grant_id, now).is_err());
        }
        assert!(!gate.ended());
        gate.tick(now, t).unwrap();
        gate.observe_grant(&fresh, now).unwrap();
        let later = other(&fresh, &key, 13, "control_mouse");
        gate.bind_offered_operation(&later, Arc::new(AtomicBool::new(false)), now)
            .unwrap();
        assert!(matches!(
            gate.classify_input_offer(&later, now).unwrap(),
            InputOffer::Replacement
        ));
        // A different ID cannot reuse the canceled operation's nonce.
        let mut replay = later.grant.clone();
        replay.nonce = fresh.grant.nonce.clone();
        assert!(
            gate.bind_offered_operation(&sign(replay, &key), Arc::new(AtomicBool::new(false)), now)
                .is_err()
        );
    }
    #[test]
    fn a_queued_peer_cannot_adopt_a_new_input_epoch_when_another_capability_is_revoked() {
        let (mut gate, key, mouse, now, _) = setup();
        let keyboard = other(&mouse, &key, 10, "control_keyboard");
        for grant in [&mouse, &keyboard] {
            gate.admit(grant, now).unwrap();
            gate.consent(&grant.grant.grant_id, now).unwrap();
        }
        let queued_epoch = gate.input_epoch();
        gate.validate_worker_epoch(Some(queued_epoch)).unwrap();
        // The service update wins the gate lock after ingress but before dequeue.
        gate.revoke(&mouse.grant.grant_id);
        assert!(
            gate.consented_grant(&keyboard.grant.grant_id, now)
                .unwrap()
                .is_some()
        );
        assert_ne!(gate.input_epoch(), queued_epoch);
        assert_eq!(
            gate.validate_worker_epoch(Some(queued_epoch)),
            Err("revoked")
        );
        assert_eq!(
            gate.validate_worker_epoch(Some(gate.input_epoch())),
            Err("revoked")
        );
    }
    #[test]
    fn revoked_staged_records_keep_authenticated_history_without_ending_resources() {
        let (mut gate, key, mouse, now, t) = setup();
        let mut resource = other(&mouse, &key, 11, "clipboard_to_host").grant;
        resource.endpoint = None;
        let resource = sign(resource, &key);
        for grant in [&mouse, &resource] {
            gate.admit(grant, now).unwrap();
            gate.consent(&grant.grant.grant_id, now).unwrap();
        }
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/assist-control-v2.json"
        ))
        .unwrap();
        let mut lease: StreamLease =
            serde_json::from_value(fixture["leases"]["valid"][0]["signed"]["lease"].clone())
                .unwrap();
        lease.generation = 11;
        lease.grants = vec![
            mouse.grant.grant_id.clone(),
            resource.grant.grant_id.clone(),
        ];
        lease.grants.sort();
        let lease = SignedStreamLease {
            signature: URL_SAFE_NO_PAD
                .encode(key.sign(&serde_json::to_vec(&lease).unwrap()).to_bytes()),
            lease,
        };
        gate.refresh(&lease, now, t, t).unwrap();
        gate.verify_peer(&mouse.grant.helper_transport).unwrap();
        gate.suspend_input();
        let fresh = other(&mouse, &key, 12, "control_mouse");
        gate.observe_grant(&fresh, now).unwrap(); // service observation before a local offer
        gate.revoke(&fresh.grant.grant_id);
        for _ in 0..2 {
            gate.observe_grant(&fresh, now).unwrap();
            gate.revoke(&fresh.grant.grant_id);
        }
        let staged = other(&mouse, &key, 13, "control_mouse");
        gate.observe_grant(&staged, now).unwrap();
        let replacement = other(&mouse, &key, 14, "control_mouse");
        gate.observe_grant(&replacement, now).unwrap(); // replaces an unadmitted staged record
        gate.observe_grant(&staged, now).unwrap();
        gate.revoke(&staged.grant.grant_id);
        gate.authorize(&resource.grant.grant_id, "clipboard_to_host", now, t)
            .unwrap();
        assert!(gate.pending_grant(&fresh.grant.grant_id, now).is_err());
        assert!(gate.pending_grant(&staged.grant.grant_id, now).is_err());
        assert!(
            gate.pending_grant(&replacement.grant.grant_id, now)
                .is_err()
        );
        assert!(!gate.ended());
    }
    #[test]
    fn sibling_cancellation_fences_the_next_primitive_before_the_idle_worker_tick() {
        let (mut gate, key, mouse, now, t) = setup();
        let keyboard = other(&mouse, &key, 10, "control_keyboard");
        for grant in [&mouse, &keyboard] {
            gate.admit(grant, now).unwrap();
        }
        let flag = Arc::new(AtomicBool::new(false));
        gate.bind_operation(&keyboard.grant.grant_id, flag.clone(), now)
            .unwrap();
        for grant in [&mouse, &keyboard] {
            gate.consent(&grant.grant.grant_id, now).unwrap();
        }
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/assist-control-v2.json"
        ))
        .unwrap();
        let mut lease: StreamLease =
            serde_json::from_value(fixture["leases"]["valid"][0]["signed"]["lease"].clone())
                .unwrap();
        lease.generation = 10;
        lease.grants = vec![
            mouse.grant.grant_id.clone(),
            keyboard.grant.grant_id.clone(),
        ];
        lease.grants.sort();
        let lease = SignedStreamLease {
            signature: URL_SAFE_NO_PAD
                .encode(key.sign(&serde_json::to_vec(&lease).unwrap()).to_bytes()),
            lease,
        };
        gate.refresh(&lease, now, t, t).unwrap();
        gate.verify_peer(&mouse.grant.helper_transport).unwrap();
        let permit = ExecutionPermit {
            instance: gate.instance,
            epoch: gate.input_epoch(),
            grant: mouse.grant.grant_id.clone(),
            capability: "control_mouse".into(),
            source_generation: 1,
            until: t + Duration::from_millis(100),
            action: crate::input::Action::ReleaseAll,
            executor: None,
        };
        gate.validate_permit(&permit, 1, now, t).unwrap();
        flag.store(true, Ordering::SeqCst);
        assert_eq!(gate.validate_permit(&permit, 1, now, t), Err("revoked"));
        assert!(gate.input_generation_ended());
        assert!(gate.prepare_peer(&mouse.grant.grant_id, now, t).is_err());
    }
    #[test]
    fn canceled_signed_offers_cannot_restore_authority_and_do_not_cancel_an_existing_ack_owner() {
        let (mut gate, key, mouse, now, _) = setup();
        gate.admit(&mouse, now).unwrap();
        gate.reject_offer(&mouse, now).unwrap();
        assert!(gate.admit(&mouse, now).is_err());
        let fresh = other(&mouse, &key, 11, "control_mouse");
        gate.reject_offer(&fresh, now).unwrap();
        assert!(gate.classify_input_offer(&fresh, now).is_err());
        let canceled_sibling = other(&fresh, &key, 11, "control_keyboard");
        assert!(gate.classify_input_offer(&canceled_sibling, now).is_err());
        let sibling = other(&fresh, &key, 12, "control_keyboard");
        assert!(matches!(
            gate.classify_input_offer(&sibling, now).unwrap(),
            InputOffer::Replacement
        ));
        gate.suspend_input();
        gate.reacquire_input(&sibling, now).unwrap();
        gate.consent(&sibling.grant.grant_id, now).unwrap();
        let epoch = gate.input_epoch();
        gate.reject_offer(&sibling, now).unwrap();
        assert_eq!(gate.input_epoch(), epoch);
        assert!(
            gate.consented_grant(&sibling.grant.grant_id, now)
                .unwrap()
                .is_some()
        );
    }
}
