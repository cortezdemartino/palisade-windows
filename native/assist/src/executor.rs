//! One-shot permits, write-ahead intent and fenced cleanup, shared by native and test backends.
use crate::{
    input::{Action, Ledger},
    stream::{ExecutionPermit, SessionGate},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};
static NEXT_EXECUTOR: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExecutorIdentity(u64);
impl ExecutorIdentity {
    pub(crate) fn fresh() -> Result<Self, &'static str> {
        NEXT_EXECUTOR
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_add(1))
            .map(Self)
            .map_err(|_| "executor-identity-exhausted")
    }
}
/// Linear native registration, reserved by SessionGate before child startup.
/// It is not serialized, cloneable, or accepted from a renderer.
pub struct ExecutorReservation {
    pub(crate) identity: ExecutorIdentity,
    pub(crate) instance: u64,
}
#[cfg(windows)]
impl ExecutorReservation {
    pub(crate) fn custody(
        &self,
        session: &str,
        source: Source,
        source_key: crate::windows::source_stop::SourceKey,
    ) -> crate::windows::process_input::custody::Registration {
        crate::windows::process_input::custody::Registration {
            session: session.into(),
            slot: self.instance,
            execution: self.identity.0,
            source,
            source_key: Some(source_key),
        }
    }
}
pub struct InputCleanupRequirement {
    pub(crate) identity: ExecutorIdentity,
    pub(crate) instance: u64,
    pub(crate) nonce: u64,
}
pub struct InputCleanupReceipt {
    identity: ExecutorIdentity,
    instance: u64,
    nonce: u64,
}
impl InputCleanupReceipt {
    pub(crate) fn matches(&self, identity: ExecutorIdentity, instance: u64, nonce: u64) -> bool {
        self.identity == identity && self.instance == instance && self.nonce == nonce
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub generation: u32,
}
impl Source {
    pub fn point(&self, x: f64, y: f64) -> Result<(i32, i32), &'static str> {
        if !x.is_finite()
            || !y.is_finite()
            || !(0.0..=1.0).contains(&x)
            || !(0.0..=1.0).contains(&y)
            || self.width < 1
            || self.height < 1
            || self.width > 32768
            || self.height > 32768
            || self.generation == 0
        {
            return Err("invalid-source");
        }
        let px = i64::from(self.x) + (x * f64::from(self.width - 1)).round() as i64;
        let py = i64::from(self.y) + (y * f64::from(self.height - 1)).round() as i64;
        Ok((
            px.try_into().map_err(|_| "invalid-source")?,
            py.try_into().map_err(|_| "invalid-source")?,
        ))
    }
}
pub trait Backend {
    fn requires_registration(&self) -> bool {
        false
    }
    /// True only for an adapter with delivery-time arbitration in ordinary
    /// input AND retained mirror cleanup. The existing OS adapter stays false.
    fn cooperative_ready(&self) -> bool {
        false
    }
    fn retain_cleanup(&self) -> bool {
        false
    }
    fn heartbeat(&mut self, _source: &Source) -> Result<(), &'static str> {
        Ok(())
    }
    fn prepare(&mut self) -> Result<(), &'static str>;
    fn validate(
        &mut self,
        source: &Source,
        action: &Action,
        owned: &Ledger,
    ) -> Result<(), &'static str>;
    fn intent(&mut self, ledger: &Ledger) -> Result<(), &'static str>;
    fn apply(&mut self, source: &Source, action: &Action) -> Result<(), &'static str>;
    /// Stop/reap any queued executor before releasing mirrored input.
    fn fence(&mut self) -> Result<(), &'static str>;
    fn cleanup_allowed(&mut self) -> Result<(), &'static str>;
    /// Nonterminal input fence: all expanded/in-flight ordinary work must be
    /// quiescent, without destroying the session or resetting its authorization.
    /// Old adapters fail closed until they implement delivery acknowledgements.
    fn quiesce_for_yield(&mut self) -> Result<(), &'static str> {
        Err("cooperative-unavailable")
    }
    /// Positive delivery-time disposition. A bitmap check followed by SendInput
    /// is insufficient: both native child and mirror cleanup must honor host holds.
    fn yield_release(
        &mut self,
        _source: &Source,
        _action: &Action,
        _local: &crate::cooperative::LocalHolds,
    ) -> Result<YieldDisposition, &'static str> {
        Err("cooperative-unavailable")
    }
    fn finish(&mut self);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YieldDisposition {
    Released,
    HandedOver,
}
enum Admission {
    Legacy,
    Cooperative {
        fence: Arc<crate::cooperative::InputFence>,
        generation: u64,
        release: bool,
        interrupted: bool,
    },
}
impl Admission {
    fn check(&mut self) -> Result<(), &'static str> {
        match self {
            Self::Legacy | Self::Cooperative { release: true, .. } => Ok(()),
            Self::Cooperative {
                fence,
                generation,
                interrupted,
                ..
            } => {
                let result = fence.check(*generation);
                if result == Err("yielded") {
                    *interrupted = true;
                }
                result
            }
        }
    }
    fn nonterminal(&self, result: Result<(), &'static str>) -> bool {
        matches!(
            self,
            Self::Cooperative {
                interrupted: true,
                ..
            }
        ) && result == Err("yielded")
    }
}
pub struct Executor<B: Backend> {
    identity: ExecutorIdentity,
    registered_instance: Option<u64>,
    backend: B,
    ledger: Ledger,
    // The combined ledger remains the write-ahead process cleanup mirror.
    // Packet-driven releases use only the native permit's exact owner.
    owners: BTreeMap<(u64, String), Ledger>,
    stop: Arc<AtomicBool>,
    session_stop: Option<crate::cancellation::SessionStop>,
    enabled: bool,
    last_source: Option<Source>,
    epoch: Option<u64>,
    cleanup_pending: bool,
    mirror_failed: bool,
    receipt_issued: bool,
}
impl<B: Backend> Executor<B> {
    pub fn heartbeat(
        &mut self,
        gate: &mut SessionGate,
        source: &Source,
        now: u64,
        instant: Instant,
    ) -> Result<(), &'static str> {
        let result = (|| {
            gate.tick(now, instant)?;
            if self.stopped() || self.epoch.is_some_and(|epoch| epoch != gate.input_epoch()) {
                return Err("revoked");
            }
            self.backend.heartbeat(source)
        })();
        if result.is_err() {
            gate.suspend_input();
            self.stop.store(true, Ordering::SeqCst);
            let _ = self.release_owned(source);
        }
        result
    }
    pub fn new(backend: B, stop: Arc<AtomicBool>, enabled: bool) -> Self {
        Self {
            // Keep zero outside the managed reservation identity space.
            identity: ExecutorIdentity::fresh().unwrap_or(ExecutorIdentity(0)),
            registered_instance: None,
            backend,
            ledger: Ledger::default(),
            owners: BTreeMap::new(),
            stop,
            session_stop: None,
            enabled,
            last_source: None,
            epoch: None,
            cleanup_pending: false,
            mirror_failed: false,
            receipt_issued: false,
        }
    }
    pub fn registered(
        backend: B,
        stop: Arc<AtomicBool>,
        enabled: bool,
        reservation: ExecutorReservation,
    ) -> Self {
        let mut executor = Self::new(backend, stop, enabled);
        executor.identity = reservation.identity;
        executor.registered_instance = Some(reservation.instance);
        executor
    }
    pub fn with_session_stop(mut self, stop: impl Into<crate::cancellation::SessionStop>) -> Self {
        self.session_stop = Some(stop.into());
        self
    }
    fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
            || self
                .session_stop
                .as_ref()
                .is_some_and(|stop| stop.is_stopped())
    }
    pub fn dispatch(
        &mut self,
        permit: ExecutionPermit,
        gate: &mut SessionGate,
        source: &Source,
        action: &Action,
        clock: impl FnMut() -> Result<(u64, Instant), &'static str>,
    ) -> Result<(), &'static str> {
        self.dispatch_checked(permit, gate, source, action, clock, Admission::Legacy)
    }
    pub fn dispatch_cooperative(
        &mut self,
        permit: ExecutionPermit,
        gate: &mut SessionGate,
        source: &Source,
        action: &Action,
        scheduler: &mut crate::cooperative::LocalYield,
        mut clock: impl FnMut() -> Result<(u64, Instant), &'static str>,
    ) -> Result<(), &'static str> {
        if !self.backend.cooperative_ready() {
            return Err("cooperative-unavailable");
        }
        let (now, instant) = clock()?;
        let authority = gate.input_authority(source, now, instant)?;
        let release = matches!(
            action,
            Action::ReleaseAll
                | Action::Key { down: false, .. }
                | Action::Button { down: false, .. }
        );
        scheduler.admit(release, &authority)?;
        let fence = scheduler.input_fence();
        let generation = fence.generation();
        if !release && self.cleanup_pending {
            return Err("yielded");
        }
        self.dispatch_checked(
            permit,
            gate,
            source,
            action,
            clock,
            Admission::Cooperative {
                fence,
                generation,
                release,
                interrupted: false,
            },
        )
    }
    fn dispatch_checked(
        &mut self,
        permit: ExecutionPermit,
        gate: &mut SessionGate,
        source: &Source,
        action: &Action,
        mut clock: impl FnMut() -> Result<(u64, Instant), &'static str>,
        mut admission: Admission,
    ) -> Result<(), &'static str> {
        self.last_source = Some(source.clone());
        let result = (|| {
            if !self.enabled {
                return Err("native-input-disabled");
            }
            if !permit.matches(action) {
                return Err("binding-mismatch");
            }
            let registration = permit.executor_registration();
            if registration
                != self
                    .registered_instance
                    .map(|instance| (self.identity, instance))
                || (self.backend.requires_registration() && registration.is_none())
            {
                return Err("executor-binding-mismatch");
            }
            action.validate()?;
            self.backend.prepare()?;
            let (now, t) = clock()?;
            gate.validate_permit(&permit, source.generation, now, t)?;
            admission.check()?;
            let owner = permit.owner();
            self.epoch = Some(gate.input_epoch());
            match action {
                Action::Move { source_gen, .. }
                | Action::Button { source_gen, .. }
                | Action::Wheel { source_gen, .. }
                    if *source_gen != source.generation =>
                {
                    return Err("source-changed");
                }
                _ => (),
            }
            let owned = self.owners.get(&owner).cloned().unwrap_or_default();
            let plan = match action {
                Action::ReleaseAll => owned.clone().releases(),
                Action::RepeatKey(_) => owned.clone().plan(action)?,
                Action::Shortcut(_) | Action::Text(_) => self.ledger.clone().plan(action)?,
                _ => vec![action.clone()],
            };
            for step in plan {
                if !self
                    .owners
                    .get(&owner)
                    .cloned()
                    .unwrap_or_default()
                    .owns(&step)
                {
                    continue;
                }
                self.backend.validate(source, &step, &self.ledger)?;
                // Target/desktop checks may block; use endpoint-owned clocks afterwards.
                let (now, t) = clock()?;
                gate.validate_permit(&permit, source.generation, now, t)?;
                admission.check()?;
                if self.stopped() {
                    return Err("revoked");
                }
                let down = matches!(
                    step,
                    Action::Key { down: true, .. }
                        | Action::Button { down: true, .. }
                        | Action::Unicode { down: true, .. }
                );
                if down {
                    if self.ledger.is_held(&step)
                        && !self.owners.get(&owner).is_some_and(|l| l.is_held(&step))
                    {
                        return Err("held-input");
                    }
                    self.ledger.plan(&step)?;
                    self.owners.entry(owner.clone()).or_default().plan(&step)?;
                    self.synchronize_mirror()?;
                }
                let (now, t) = clock()?;
                gate.validate_permit(&permit, source.generation, now, t)?;
                admission.check()?;
                if self.stopped() {
                    return Err("revoked");
                }
                self.backend.apply(source, &step)?;
                if !down {
                    self.ledger.plan(&step)?;
                    if let Some(owned) = self.owners.get_mut(&owner) {
                        owned.plan(&step)?;
                        if !owned.held() {
                            self.owners.remove(&owner);
                        }
                    }
                    self.synchronize_mirror()?;
                }
                let (now, t) = clock()?;
                gate.tick(now, t)?;
                admission.check()?;
                if self.stopped() {
                    return Err("revoked");
                }
            }
            Ok(())
        })();
        if admission.nonterminal(result) {
            self.cleanup_pending = true;
        } else if result.is_err() {
            gate.suspend_input();
            self.stop.store(true, Ordering::SeqCst);
            let _ = self.release_owned(source);
        }
        self.backend.finish();
        result
    }
    fn synchronize_mirror(&mut self) -> Result<(), &'static str> {
        let result = self.backend.intent(&self.ledger);
        if result.is_err() && self.backend.requires_registration() {
            self.mirror_failed = true;
        }
        result
    }
    fn confirm_mirror_settlement(&mut self) -> Result<(), &'static str> {
        self.synchronize_mirror()?;
        if self.mirror_failed {
            // A later write can update the mirror. It cannot prove settlement
            // of the earlier failed native generation.
            return Err("mirror-settlement-failed");
        }
        Ok(())
    }
    pub fn release_owned(&mut self, source: &Source) -> Result<(), &'static str> {
        self.cleanup_pending = true;
        let result = (|| {
            if !self.enabled {
                return Err("native-input-disabled");
            }
            self.stop.store(true, Ordering::SeqCst);
            self.backend.fence()?;
            self.backend.prepare()?;
            for action in self.ledger.clone().releases() {
                self.backend.cleanup_allowed()?;
                self.backend.apply(source, &action)?;
                self.ledger.plan(&action)?;
                self.synchronize_mirror()?;
            }
            // Empty local state is not proof that the surviving backend mirror
            // received the last handover/release. Keep the owner until its final
            // empty intent is acknowledged, including terminal cleanup retries.
            self.confirm_mirror_settlement()?;
            self.owners.clear();
            self.cleanup_pending = false;
            Ok(())
        })();
        self.backend.finish();
        result
    }
    /// Settle the existing write-ahead ledgers without revoking grants. The
    /// caller must continue gating input until a native ready/barrier transition.
    /// Unsupported, uncertain or partial delivery retains unresolved ownership.
    pub fn yield_owned(
        &mut self,
        source: &Source,
        local: &crate::cooperative::LocalHolds,
    ) -> Result<Vec<(Action, YieldDisposition)>, &'static str> {
        self.last_source = Some(source.clone());
        self.cleanup_pending = true;
        let result = (|| {
            if !self.enabled || self.stopped() {
                return Err("revoked");
            }
            if !self.backend.cooperative_ready() {
                return Err("cooperative-unavailable");
            }
            self.backend.quiesce_for_yield()?;
            if !local.certain() {
                return Err("yield-uncertain");
            }
            self.backend.prepare()?;
            let mut completed = Vec::new();
            for action in self.ledger.clone().releases() {
                self.backend.cleanup_allowed()?;
                let disposition = self.backend.yield_release(source, &action, local)?;
                // Only a positive native delivery/handover can clear either
                // mirror. A timeout or insertion count is never that proof.
                self.ledger.plan(&action)?;
                for owned in self.owners.values_mut() {
                    if owned.owns(&action) {
                        owned.plan(&action)?;
                    }
                }
                self.owners.retain(|_, owned| owned.held());
                self.synchronize_mirror()?;
                completed.push((action, disposition));
            }
            // Retry a failed final mirror write even if its positive delivery
            // acknowledgement already left the local ledger empty.
            self.confirm_mirror_settlement()?;
            self.cleanup_pending = false;
            Ok(completed)
        })();
        self.backend.finish();
        result
    }
    pub fn held(&self) -> bool {
        self.ledger.held()
    }
    pub fn yield_cleanup_complete(&self) -> bool {
        !self.cleanup_pending && !self.ledger.held()
    }
    /// A terminal receipt belongs to this exact registered executor. Ordinary
    /// yield, a fresh empty replacement and failed mirror sync cannot issue it.
    pub fn release_for(
        &mut self,
        requirement: &InputCleanupRequirement,
        source: &Source,
    ) -> Result<InputCleanupReceipt, &'static str> {
        if requirement.identity != self.identity
            || self.registered_instance != Some(requirement.instance)
        {
            return Err("executor-binding-mismatch");
        }
        if self.receipt_issued {
            return Err("cleanup-replayed");
        }
        self.release_owned(source)?;
        self.receipt_issued = true;
        Ok(InputCleanupReceipt {
            identity: self.identity,
            instance: requirement.instance,
            nonce: requirement.nonce,
        })
    }
}
impl<B: Backend> Drop for Executor<B> {
    fn drop(&mut self) {
        if self.enabled && (self.ledger.held() || self.cleanup_pending) {
            if let Some(source) = self.last_source.clone() {
                loop {
                    if self.release_owned(&source).is_ok() || !self.backend.retain_cleanup() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    }
}
