//! Native local-yield scheduling. No grant registry, input ledger, clock supplied
//! by a page, or OS effects. The executor retains all cleanup ownership.
use crate::{executor::Source, grant::token, input::Action};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

pub const IDLE_MS: u64 = 2_000;
pub const MAX_EPOCH: u32 = 2_147_483_647;

/// Independent of the renderer and ordinary work queue. Native observation can
/// fence already-minted/expanded input before scheduling its detailed event.
/// This changes execution admission only, never service grants or generations.
#[derive(Default)]
pub struct InputFence {
    word: AtomicU64,
    terminal: AtomicBool,
}
impl InputFence {
    pub fn observation(
        self: &Arc<Self>,
        event: LocalEvent,
    ) -> Result<LocalObservation, &'static str> {
        Ok(self.stamp()?.attach(event))
    }
    /// Stamp the numeric native event immediately, before deferred decoding.
    /// Consuming the stamp later must not pause again or acknowledge later work.
    pub(crate) fn stamp(self: &Arc<Self>) -> Result<FenceStamp, &'static str> {
        Ok(FenceStamp {
            generation: self.pause()?,
            fence: self.clone(),
        })
    }
    pub fn pause(&self) -> Result<u64, &'static str> {
        match self
            .word
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                n.checked_add(2).map(|v| v | 1)
            }) {
            Ok(before) => Ok((before >> 1) + 1),
            Err(_) => {
                self.end();
                Err("epoch-exhausted")
            }
        }
    }
    pub fn generation(&self) -> u64 {
        self.word.load(Ordering::SeqCst) >> 1
    }
    pub fn check(&self, generation: u64) -> Result<(), &'static str> {
        if self.terminal.load(Ordering::SeqCst) {
            return Err("revoked");
        }
        if generation > u64::MAX >> 1 || self.word.load(Ordering::SeqCst) != generation << 1 {
            return Err("yielded");
        }
        Ok(())
    }
    fn resume(&self, generation: u64) -> Result<(), &'static str> {
        if generation > u64::MAX >> 1 || self.terminal.load(Ordering::SeqCst) {
            return Err("revoked");
        }
        let word = self.word.load(Ordering::SeqCst);
        if word >> 1 != generation {
            return Err("yielded");
        }
        self.word
            .compare_exchange(word, generation << 1, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "yielded")?;
        self.check(generation)
    }
    pub fn end(&self) {
        self.terminal.store(true, Ordering::SeqCst);
        self.word.fetch_or(1, Ordering::SeqCst);
    }
}

/// Semantic authority, produced only by the existing signed SessionGate. Lease
/// renewals with unchanged authority deliberately do not change this value.
#[derive(Clone, PartialEq, Eq)]
pub struct InputAuthority {
    pub(crate) instance: u64,
    pub(crate) input_epoch: u64,
    pub(crate) session: String,
    pub(crate) source: Source,
    pub(crate) grants: Vec<(String, u64, bool, bool)>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Barrier {
    pub p: String,
    pub v: u32,
    pub session: String,
    pub grant: String,
    pub r#gen: u64,
    pub seq: u32,
    pub kind: String,
    pub epoch: u32,
    pub t: u64,
}
impl Barrier {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.p != "palisade.assist.control"
            || self.v != 2
            || self.kind != "input-barrier"
            || !token(&self.session, 8, 128)
            || !token(&self.grant, 8, 128)
            || !(1..=9_007_199_254_740_991).contains(&self.r#gen)
            || self.epoch > MAX_EPOCH
            || self.t > 9_007_199_254_740_991
        {
            return Err("invalid-message");
        }
        Ok(())
    }
    /// This decoder is separate from legacy/Profile 2 decoders. A wire field
    /// cannot opt a receiver into a feature it has not negotiated.
    pub fn parse(raw: &str, negotiated: bool) -> Result<Self, &'static str> {
        if raw.len() > 2_048 {
            return Err("too-large");
        }
        let m: Self = serde_json::from_str(raw).map_err(|_| "invalid-message")?;
        m.validate()?;
        if !negotiated {
            return Err("unsupported");
        }
        Ok(m)
    }
}

/// Only the signed gate can issue this after peer, capability, generation,
/// sequence and monotonic sender-age validation. A barrier grants no input.
pub struct VerifiedBarrier {
    pub(crate) instance: u64,
    pub(crate) input_epoch: u64,
    pub(crate) session: String,
    pub(crate) epoch: u32,
    pub(crate) until: Instant,
    pub(crate) ingress: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum YieldReason {
    LocalInput,
    LocalHeld,
    Neutralizing,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Negotiating,
    Legacy,
    Active,
    Yielded,
    Ready,
    Ended,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InputState {
    pub state: State,
    pub epoch: u32,
    pub reason: Option<YieldReason>,
}
#[derive(Default, Debug, Serialize)]
pub struct Update {
    pub send: Vec<InputState>,
    /// Fence ordinary delivery and settle the EXISTING executor's owned input.
    /// This is not a terminal backend fence or a cleanup-complete assertion.
    pub neutralize: bool,
    pub terminal: Option<&'static str>,
}

/// Native observation only; intentionally not Deserialize or page accessible.
pub enum LocalEvent {
    Activity,
    Key { code: String, down: bool },
    Button { button: String, down: bool },
    Uncertain,
}
/// Native queue entry stamped at observation, before any ordinary work. It is
/// never deserialized from a page and cannot acknowledge a later unseen event.
pub struct LocalObservation {
    event: LocalEvent,
    generation: u64,
    fence: Arc<InputFence>,
}
pub(crate) struct FenceStamp {
    generation: u64,
    fence: Arc<InputFence>,
}
impl FenceStamp {
    pub(crate) fn is_for(&self, fence: &Arc<InputFence>) -> bool {
        Arc::ptr_eq(&self.fence, fence)
    }
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }
    pub(crate) fn attach(self, event: LocalEvent) -> LocalObservation {
        LocalObservation {
            event,
            generation: self.generation,
            fence: self.fence,
        }
    }
}
#[derive(Clone, Default)]
pub struct LocalHolds {
    keys: BTreeSet<String>,
    buttons: BTreeSet<String>,
    uncertain: bool,
}
impl LocalHolds {
    pub fn held(&self) -> bool {
        !self.keys.is_empty() || !self.buttons.is_empty()
    }
    pub fn certain(&self) -> bool {
        !self.uncertain
    }
    pub fn owns(&self, action: &Action) -> bool {
        match action {
            Action::Key { code, .. } => self.keys.contains(code),
            Action::Button { button, .. } => self.buttons.contains(button),
            _ => false,
        }
    }
    fn observe(&mut self, event: LocalEvent) -> Result<(), &'static str> {
        match event {
            LocalEvent::Activity => (),
            LocalEvent::Uncertain => self.uncertain = true,
            LocalEvent::Key { code, down } => {
                // Unknown physical scan codes cannot be guessed into neutrality.
                if crate::input::scan_code2(&code).is_none() {
                    self.uncertain = true;
                    return Ok(());
                }
                if down {
                    self.keys.insert(code);
                } else {
                    self.keys.remove(&code);
                }
            }
            LocalEvent::Button { button, down } => {
                if !["left", "middle", "right", "back", "forward"].contains(&button.as_str()) {
                    self.uncertain = true;
                    return Ok(());
                }
                if down {
                    self.buttons.insert(button);
                } else {
                    self.buttons.remove(&button);
                }
            }
        }
        Ok(())
    }
}

pub struct LocalYield {
    initial: InputAuthority,
    snapshot: Option<InputAuthority>,
    state: State,
    epoch: u32,
    reason: Option<YieldReason>,
    last_local: Option<u64>,
    last_time: u64,
    ready_at: Option<Instant>,
    local: LocalHolds,
    fence: Arc<InputFence>,
    observed_generation: u64,
}
impl LocalYield {
    /// Construct only for a separately negotiated endpoint/extension v3. Merely
    /// constructing this scheduler does not advertise localYield or opt a peer in.
    pub fn new(authority: InputAuthority) -> Self {
        Self {
            initial: authority,
            snapshot: None,
            state: State::Negotiating,
            epoch: 0,
            reason: None,
            last_local: None,
            last_time: 0,
            ready_at: None,
            local: LocalHolds::default(),
            fence: Arc::new(InputFence::default()),
            observed_generation: 0,
        }
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn epoch(&self) -> u32 {
        self.epoch
    }
    pub fn local_holds(&self) -> &LocalHolds {
        &self.local
    }
    pub fn input_fence(&self) -> Arc<InputFence> {
        self.fence.clone()
    }
    fn check_binding(&mut self, authority: &InputAuthority) -> Result<(), &'static str> {
        if authority.instance != self.initial.instance
            || authority.session != self.initial.session
            || authority.input_epoch != self.initial.input_epoch
            || authority.source != self.initial.source
        {
            self.end("authority-changed");
            return Err("authority-changed");
        }
        Ok(())
    }
    fn clock(&mut self, at: u64) -> Result<(), &'static str> {
        if at < self.last_time {
            self.end("clock-regressed");
            return Err("revoked");
        }
        self.last_time = at;
        Ok(())
    }
    fn reason_at(&self, at: u64) -> YieldReason {
        if self.local.held() {
            YieldReason::LocalHeld
        } else if self
            .last_local
            .is_some_and(|last| at.saturating_sub(last) < IDLE_MS)
        {
            YieldReason::LocalInput
        } else {
            YieldReason::Neutralizing
        }
    }
    fn busy(&self, at: u64) -> bool {
        self.local.held()
            || !self.local.certain()
            || self
                .last_local
                .is_some_and(|last| at.saturating_sub(last) < IDLE_MS)
    }
    fn status(&self) -> Update {
        Update {
            send: vec![InputState {
                state: self.state,
                epoch: self.epoch,
                reason: self.reason,
            }],
            ..Update::default()
        }
    }
    fn start(&mut self, at: u64, authority: &InputAuthority) -> Update {
        if self.epoch >= MAX_EPOCH {
            return self.end("epoch-exhausted");
        }
        self.epoch += 1;
        self.state = State::Yielded;
        self.ready_at = None;
        self.snapshot = Some(authority.clone());
        self.reason = Some(self.reason_at(at));
        let mut update = self.status();
        update.neutralize = true;
        update
    }
    pub fn observe(
        &mut self,
        event: LocalEvent,
        at: u64,
        authority: &InputAuthority,
        cleanup_complete: bool,
    ) -> Result<Update, &'static str> {
        let observation = self.fence.observation(event)?;
        self.observe_native(observation, at, authority, cleanup_complete)
    }
    pub fn observe_native(
        &mut self,
        observation: LocalObservation,
        at: u64,
        authority: &InputAuthority,
        cleanup_complete: bool,
    ) -> Result<Update, &'static str> {
        self.clock(at)?;
        if self.state == State::Ended {
            return Ok(Update::default());
        }
        self.check_binding(authority)?;
        if !Arc::ptr_eq(&self.fence, &observation.fence)
            || observation.generation <= self.observed_generation
        {
            self.local.observe(LocalEvent::Uncertain)?;
            self.end("observation-invalid");
            return Err("observation-replay");
        }
        if observation.generation != self.observed_generation + 1 {
            self.local.observe(LocalEvent::Uncertain)?;
        }
        self.observed_generation = observation.generation;
        self.local.observe(observation.event)?;
        self.last_local = Some(at);
        if self.state == State::Legacy {
            return Ok(self.end("local-input"));
        }
        // Check the snapshot before starting a new epoch after Ready.
        if matches!(self.state, State::Yielded | State::Ready)
            && self.snapshot.as_ref() != Some(authority)
        {
            return Ok(self.end("authority-changed"));
        }
        if matches!(self.state, State::Active | State::Ready) {
            return Ok(self.start(at, authority));
        }
        self.tick(at, authority, cleanup_complete)
    }
    pub fn tick(
        &mut self,
        at: u64,
        authority: &InputAuthority,
        cleanup_complete: bool,
    ) -> Result<Update, &'static str> {
        self.tick_at(at, Instant::now(), authority, cleanup_complete)
    }
    pub fn tick_at(
        &mut self,
        at: u64,
        instant: Instant,
        authority: &InputAuthority,
        cleanup_complete: bool,
    ) -> Result<Update, &'static str> {
        self.clock(at)?;
        if self.state != State::Ended {
            self.check_binding(authority)?;
        }
        if !matches!(self.state, State::Yielded | State::Ready) {
            return Ok(Update::default());
        }
        if self.snapshot.as_ref() != Some(authority) {
            return Ok(self.end("authority-changed"));
        }
        if self.state == State::Ready {
            return Ok(Update::default());
        }
        if !self.busy(at) && cleanup_complete && self.fence.generation() == self.observed_generation
        {
            self.state = State::Ready;
            self.ready_at = Some(instant);
            self.reason = None;
            return Ok(self.status());
        }
        let next = self.reason_at(at);
        if self.reason == Some(next) {
            return Ok(Update::default());
        }
        self.reason = Some(next);
        Ok(self.status())
    }
    pub fn barrier(
        &mut self,
        barrier: VerifiedBarrier,
        at: u64,
        instant: Instant,
        authority: &InputAuthority,
    ) -> Result<Update, &'static str> {
        self.clock(at)?;
        if self.state == State::Ended {
            return Err("revoked");
        }
        self.check_binding(authority)?;
        if barrier.instance != self.initial.instance
            || barrier.input_epoch != authority.input_epoch
            || barrier.session != self.initial.session
            || instant >= barrier.until
        {
            return Err("binding-mismatch");
        }
        if matches!(self.state, State::Yielded | State::Ready)
            && self.snapshot.as_ref() != Some(authority)
        {
            self.end("authority-changed");
            return Err("authority-changed");
        }
        if barrier.epoch == 0 && self.state == State::Negotiating {
            if self.fence.generation() != self.observed_generation {
                return Err("yielded");
            }
            if self.busy(at) {
                return Ok(self.start(at, authority));
            }
            self.fence.resume(self.observed_generation)?;
            self.state = State::Active;
            return Ok(Update::default());
        }
        if self.state == State::Ready && barrier.epoch == self.epoch {
            if self.ready_at.is_none_or(|ready| barrier.ingress < ready) {
                return Err("stale-epoch");
            }
            self.fence.resume(self.observed_generation)?;
            self.state = State::Active;
            return Ok(Update::default());
        }
        Err(
            if self.state == State::Yielded && barrier.epoch == self.epoch {
                "yielded"
            } else {
                "stale-epoch"
            },
        )
    }
    /// Call only after SessionGate's ordinary permit/age/sequence checks. Releases
    /// still go through the executor's grant-scoped ledger, including while paused.
    pub fn admit(&mut self, release: bool, authority: &InputAuthority) -> Result<(), &'static str> {
        if self.state == State::Ended {
            return Err("revoked");
        }
        self.check_binding(authority)?;
        if matches!(self.state, State::Yielded | State::Ready)
            && self.snapshot.as_ref() != Some(authority)
        {
            self.end("authority-changed");
            return Err("authority-changed");
        }
        if release {
            return Ok(());
        }
        match self.state {
            State::Negotiating => self.state = State::Legacy,
            State::Yielded => return Err("yielded"),
            State::Ready => return Err("stale-epoch"),
            _ => (),
        }
        self.fence.check(self.observed_generation)?;
        Ok(())
    }
    pub fn end(&mut self, why: &'static str) -> Update {
        if self.state == State::Ended {
            return Update::default();
        }
        self.state = State::Ended;
        self.fence.end();
        self.ready_at = None;
        self.reason = None;
        Update {
            neutralize: true,
            terminal: Some(why),
            ..Update::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Ledger, parse, parse_profile2};
    use serde_json::{Value, json};
    use std::time::Duration;

    fn fixtures() -> Value {
        serde_json::from_str(include_str!(
            "../../../tests/fixtures/assist-control-v2-yield.json"
        ))
        .unwrap()
    }
    fn authority(name: &str) -> InputAuthority {
        InputAuthority {
            instance: 3,
            input_epoch: 1,
            session: fixtures()["session"].as_str().unwrap().into(),
            source: Source {
                x: -800,
                y: 0,
                width: 800,
                height: 600,
                generation: 1,
            },
            grants: vec![(name.into(), 1, true, true)],
        }
    }
    fn barrier(gate: &LocalYield, epoch: u32, instant: Instant) -> VerifiedBarrier {
        VerifiedBarrier {
            instance: gate.initial.instance,
            input_epoch: gate.initial.input_epoch,
            session: gate.initial.session.clone(),
            epoch,
            ingress: instant,
            until: instant + Duration::from_millis(100),
        }
    }
    fn local(event: &Value) -> LocalEvent {
        let down = event["type"] == "down";
        if let Some(code) = event["key"].as_str() {
            LocalEvent::Key {
                code: code.into(),
                down,
            }
        } else if let Some(button) = event["button"].as_str() {
            LocalEvent::Button {
                button: button.into(),
                down,
            }
        } else {
            LocalEvent::Activity
        }
    }
    fn held(actions: &[Action]) -> Value {
        let mut keys = vec![];
        let mut buttons = vec![];
        for a in actions {
            match a {
                Action::Key { code, .. } => keys.push(code.clone()),
                Action::Button { button, .. } => buttons.push(button.clone()),
                _ => (),
            }
        }
        keys.sort();
        buttons.sort();
        json!({"keys": keys, "buttons": buttons})
    }
    #[test]
    fn ten_canonical_endpoint_sequences_match_native_scheduler_and_existing_ledger() {
        let fixture = fixtures();
        assert_eq!(
            fixture["defaults"],
            json!({"idleMs": IDLE_MS, "maxEpoch": MAX_EPOCH})
        );
        for scenario in fixture["endpoint"].as_array().unwrap() {
            let initial = authority("A");
            let mut gate = LocalYield::new(initial);
            // Simulation uses the production Ledger; the scheduler has no
            // duplicate helper ledger. Real OS delivery is not exercised here.
            let mut ledger = Ledger::default();
            for step in scenario["steps"].as_array().unwrap() {
                let now = Instant::now();
                let auth = authority(step["ctx"]["authority"].as_str().unwrap_or("A"));
                let clean = step["ctx"]["cleanupComplete"].as_bool().unwrap_or(true);
                let at = step["at"]
                    .as_u64()
                    .or_else(|| step["tick"].as_u64())
                    .unwrap_or(gate.last_time);
                let mut injected = false;
                let mut released = vec![];
                let mut handed = vec![];
                let result = (|| {
                    let update = if let Some(m) = step.get("helper") {
                        if m["session"] != fixture["session"] {
                            return Err("binding-mismatch");
                        }
                        if step["grant"]["active"] == false {
                            return Err("revoked");
                        }
                        let cap = step["grant"]["capability"].as_str().unwrap();
                        if m["kind"] == "input-state" {
                            return Err("not-granted");
                        }
                        if m["kind"] == "input-barrier" {
                            if !["control_mouse", "control_keyboard"].contains(&cap) {
                                return Err("not-granted");
                            }
                            gate.barrier(
                                barrier(&gate, m["epoch"].as_u64().unwrap() as u32, now),
                                at,
                                now,
                                &auth,
                            )?
                        } else {
                            let action = parse(&m.to_string())
                                .or_else(|_| parse_profile2(&m.to_string()).map(|m| m.message))?
                                .action;
                            let mouse = matches!(
                                action,
                                Action::Move { .. } | Action::Button { .. } | Action::Wheel { .. }
                            );
                            if (mouse && cap != "control_mouse")
                                || (!mouse
                                    && action != Action::ReleaseAll
                                    && cap != "control_keyboard")
                            {
                                return Err("not-granted");
                            }
                            let release = matches!(
                                action,
                                Action::ReleaseAll
                                    | Action::Key { down: false, .. }
                                    | Action::Button { down: false, .. }
                            );
                            gate.admit(release, &auth)?;
                            if release {
                                let plan = if action == Action::ReleaseAll {
                                    ledger.clone().releases()
                                } else {
                                    vec![action]
                                };
                                for a in plan {
                                    let matching = matches!(
                                        (&a, cap),
                                        (Action::Key { .. }, "control_keyboard")
                                            | (Action::Button { .. }, "control_mouse")
                                    );
                                    if matching && ledger.owns(&a) {
                                        ledger.plan(&a)?;
                                        released.push(a);
                                    }
                                }
                            } else {
                                ledger.plan(&action)?;
                                injected = true;
                            }
                            Update::default()
                        }
                    } else if let Some(event) = step.get("local") {
                        gate.observe(local(event), at, &auth, clean)?
                    } else if let Some(why) = step["end"].as_str() {
                        gate.end(match why {
                            "revoked" => "revoked",
                            "local-stop" => "local-stop",
                            "stream-ended" => "stream-ended",
                            _ => "stopped",
                        })
                    } else {
                        gate.tick_at(at, now, &auth, clean)?
                    };
                    if update.neutralize {
                        for a in ledger.clone().releases() {
                            if gate.local_holds().owns(&a) {
                                handed.push(a.clone());
                            } else {
                                released.push(a.clone());
                            }
                            ledger.plan(&a)?;
                        }
                    }
                    Ok(
                        json!({"inject":injected,"send":update.send,"release":held(&released),"handedOver":held(&handed),"terminal":update.terminal}),
                    )
                })();
                if let Some(error) = step["error"].as_str() {
                    assert_eq!(result, Err(error), "{} / {}", scenario["name"], step);
                } else {
                    // Reasons are canonical strings; terminal fixture spelling
                    // remains part of the shared comparison.
                    assert_eq!(
                        result.unwrap(),
                        step["expect"],
                        "{} / {}",
                        scenario["name"],
                        step
                    );
                }
                assert_eq!(
                    serde_json::to_value(gate.state()).unwrap(),
                    step["state"],
                    "{}",
                    step
                );
            }
        }
    }
    #[test]
    fn barrier_decoder_rejects_duplicate_extra_malformed_and_unnegotiated_fields() {
        let f = fixtures();
        for m in f["messages"]["valid"].as_array().unwrap() {
            if m["kind"] == "input-barrier" {
                Barrier::parse(&m.to_string(), true).unwrap();
            }
        }
        let m = f["messages"]["valid"][4].to_string();
        assert_eq!(Barrier::parse(&m, false).unwrap_err(), "unsupported");
        let duplicate = m.replacen("{", "{\"epoch\":0,", 1);
        assert_eq!(
            Barrier::parse(&duplicate, true).unwrap_err(),
            "invalid-message"
        );
        for item in f["messages"]["invalid"].as_array().unwrap() {
            if item["message"]["kind"] == "input-barrier" && item.get("features").is_none() {
                assert_eq!(
                    Barrier::parse(&item["message"].to_string(), true).unwrap_err(),
                    item["error"].as_str().unwrap(),
                    "{}",
                    item["name"]
                );
            }
        }
    }
    #[test]
    fn changed_authority_at_barrier_or_release_is_terminal_without_waiting_for_tick() {
        for release in [false, true] {
            let a = authority("A");
            let b = authority("B");
            let mut gate = LocalYield::new(a.clone());
            let t = Instant::now();
            gate.barrier(barrier(&gate, 0, t), 0, t, &a).unwrap();
            gate.observe(LocalEvent::Activity, 1, &a, true).unwrap();
            gate.tick_at(2_001, t, &a, true).unwrap();
            let result = if release {
                gate.admit(true, &b)
            } else {
                gate.barrier(barrier(&gate, 1, t), 2_001, t, &b).map(|_| ())
            };
            assert_eq!(result, Err("authority-changed"));
            assert_eq!(gate.state(), State::Ended);
        }
    }
    #[test]
    fn queued_early_barrier_cannot_become_valid_when_dequeued_after_readiness() {
        let a = authority("A");
        let mut gate = LocalYield::new(a.clone());
        let t = Instant::now();
        gate.barrier(barrier(&gate, 0, t), 0, t, &a).unwrap();
        gate.observe(LocalEvent::Activity, 1, &a, true).unwrap();
        let early = barrier(&gate, 1, t);
        gate.tick_at(2_001, t + Duration::from_millis(1), &a, true)
            .unwrap();
        assert_eq!(
            gate.barrier(early, 2_001, t + Duration::from_millis(2), &a)
                .unwrap_err(),
            "stale-epoch"
        );
        assert_eq!(gate.state(), State::Ready);
    }
    #[test]
    fn uncertain_observation_blocked_cleanup_and_exhaustion_never_resume() {
        let a = authority("A");
        let mut gate = LocalYield::new(a.clone());
        let t = Instant::now();
        gate.barrier(barrier(&gate, 0, t), 0, t, &a).unwrap();
        gate.observe(LocalEvent::Uncertain, 1, &a, false).unwrap();
        gate.tick(20_000, &a, true).unwrap();
        assert_eq!(gate.state(), State::Yielded);
        assert_eq!(gate.reason, Some(YieldReason::Neutralizing));
        assert_eq!(gate.admit(false, &a), Err("yielded"));
        assert_eq!(gate.admit(true, &a), Ok(()));
        gate.epoch = MAX_EPOCH;
        gate.state = State::Ready;
        assert_eq!(
            gate.observe(LocalEvent::Activity, 20_001, &a, true)
                .unwrap()
                .terminal,
            Some("epoch-exhausted")
        );
        assert_eq!(gate.state(), State::Ended);
    }
    #[test]
    fn foreign_observation_cannot_leave_ordinary_admission_open() {
        let a = authority("A");
        let mut gate = LocalYield::new(a.clone());
        let t = Instant::now();
        gate.barrier(barrier(&gate, 0, t), 0, t, &a).unwrap();
        let foreign = Arc::new(InputFence::default())
            .observation(LocalEvent::Activity)
            .unwrap();
        assert_eq!(
            gate.observe_native(foreign, 1, &a, true).unwrap_err(),
            "observation-replay"
        );
        assert_eq!(gate.state(), State::Ended);
        assert_eq!(gate.admit(false, &a), Err("revoked"));
        assert_eq!(
            gate.barrier(barrier(&gate, 0, t), 2, t, &a).unwrap_err(),
            "revoked"
        );
    }
    #[test]
    fn queued_observation_never_acknowledges_a_later_unprocessed_physical_hold() {
        let a = authority("A");
        let mut gate = LocalYield::new(a.clone());
        let t = Instant::now();
        gate.barrier(barrier(&gate, 0, t), 0, t, &a).unwrap();
        let fence = gate.input_fence();
        let first = fence.observation(LocalEvent::Activity).unwrap();
        let second = fence
            .observation(LocalEvent::Key {
                code: "ShiftLeft".into(),
                down: true,
            })
            .unwrap();
        gate.observe_native(first, 1, &a, true).unwrap();
        gate.tick(3_000, &a, true).unwrap();
        assert_eq!(gate.state(), State::Yielded);
        assert_eq!(gate.admit(false, &a), Err("yielded"));
        gate.observe_native(second, 3_001, &a, true).unwrap();
        gate.tick(10_000, &a, true).unwrap();
        assert_eq!(gate.state(), State::Yielded);
        assert!(gate.local_holds().held());
    }
}
