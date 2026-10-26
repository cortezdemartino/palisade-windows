//! One-shot permits, write-ahead intent and fenced cleanup, shared by native and test backends.
use crate::{
    input::{Action, Ledger},
    stream::{ExecutionPermit, SessionGate},
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
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
    fn finish(&mut self);
}
pub struct Executor<B: Backend> {
    backend: B,
    ledger: Ledger,
    stop: Arc<AtomicBool>,
    enabled: bool,
    last_source: Option<Source>,
    epoch: Option<u64>,
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
            if self.stop.load(Ordering::SeqCst)
                || self.epoch.is_some_and(|epoch| epoch != gate.epoch())
            {
                return Err("revoked");
            }
            self.backend.heartbeat(source)
        })();
        if result.is_err() {
            self.stop.store(true, Ordering::SeqCst);
            let _ = self.release_owned(source);
        }
        result
    }
    pub fn new(backend: B, stop: Arc<AtomicBool>, enabled: bool) -> Self {
        Self {
            backend,
            ledger: Ledger::default(),
            stop,
            enabled,
            last_source: None,
            epoch: None,
        }
    }
    pub fn dispatch(
        &mut self,
        permit: ExecutionPermit,
        gate: &mut SessionGate,
        source: &Source,
        action: &Action,
        mut clock: impl FnMut() -> Result<(u64, Instant), &'static str>,
    ) -> Result<(), &'static str> {
        self.last_source = Some(source.clone());
        let result = (|| {
            if !self.enabled {
                return Err("native-input-disabled");
            }
            if !permit.matches(action) {
                return Err("binding-mismatch");
            }
            action.validate()?;
            self.backend.prepare()?;
            let (now, t) = clock()?;
            gate.validate_permit(&permit, source.generation, now, t)?;
            self.epoch = Some(gate.epoch());
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
            let plan = match action {
                Action::Shortcut(_) | Action::ReleaseAll => self.ledger.clone().plan(action)?,
                _ => vec![action.clone()],
            };
            for step in plan {
                if !self.ledger.owns(&step) {
                    continue;
                }
                self.backend.validate(source, &step, &self.ledger)?;
                // Target/desktop checks may block; use endpoint-owned clocks afterwards.
                let (now, t) = clock()?;
                gate.validate_permit(&permit, source.generation, now, t)?;
                if self.stop.load(Ordering::SeqCst) {
                    return Err("revoked");
                }
                let down = matches!(
                    step,
                    Action::Key { down: true, .. } | Action::Button { down: true, .. }
                );
                if down {
                    self.ledger.plan(&step)?;
                    self.backend.intent(&self.ledger)?;
                }
                let (now, t) = clock()?;
                gate.validate_permit(&permit, source.generation, now, t)?;
                if self.stop.load(Ordering::SeqCst) {
                    return Err("revoked");
                }
                self.backend.apply(source, &step)?;
                if !down {
                    self.ledger.plan(&step)?;
                    self.backend.intent(&self.ledger)?;
                }
                let (now, t) = clock()?;
                gate.tick(now, t)?;
            }
            Ok(())
        })();
        if result.is_err() {
            gate.stop();
            self.stop.store(true, Ordering::SeqCst);
            let _ = self.release_owned(source);
        }
        self.backend.finish();
        result
    }
    pub fn release_owned(&mut self, source: &Source) -> Result<(), &'static str> {
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
                self.backend.intent(&self.ledger)?;
            }
            Ok(())
        })();
        self.backend.finish();
        result
    }
    pub fn held(&self) -> bool {
        self.ledger.held()
    }
}
impl<B: Backend> Drop for Executor<B> {
    fn drop(&mut self) {
        if self.enabled && self.ledger.held() {
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
