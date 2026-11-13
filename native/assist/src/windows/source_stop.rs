//! Source cancellation, never a permission or grant registry. The root reader
//! signals events without entering native dispatch or ordinary custody work.
use super::process_tree::StopEvent;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    os::windows::io::{AsRawHandle, OwnedHandle},
    sync::{Arc, Mutex},
};
use windows_sys::Win32::{Foundation::WAIT_TIMEOUT, System::Threading::WaitForSingleObject};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceKey {
    pub document: u32,
    pub operation: String,
}
impl SourceKey {
    fn validate(&self) -> Result<(), &'static str> {
        if self.document == u32::MAX || !crate::grant::token(&self.operation, 8, 128) {
            return Err("invalid-source");
        }
        Ok(())
    }
}
#[derive(Default)]
struct State {
    current: Option<SourceKey>,
    entries: BTreeMap<SourceKey, Arc<StopEvent>>,
    closed: bool,
}
#[derive(Default)]
pub(crate) struct SourceStops(Mutex<State>);
impl SourceStops {
    pub(crate) fn begin(&self, key: &SourceKey) -> Result<(), &'static str> {
        key.validate()?;
        let mut state = self.0.lock().map_err(|_| "source-state")?;
        if state.closed || state.entries.len() >= 128 || state.entries.contains_key(key) {
            return Err("stale-source");
        }
        if state
            .current
            .as_ref()
            .and_then(|key| state.entries.get(key))
            .is_some_and(
                |event| unsafe { WaitForSingleObject(event.handle_for_query(), 0) } == WAIT_TIMEOUT,
            )
        {
            return Err("source-busy");
        }
        state
            .entries
            .insert(key.clone(), Arc::new(StopEvent::new()?));
        state.current = Some(key.clone());
        Ok(())
    }
    pub(crate) fn stop(&self, key: &SourceKey) -> Result<(), &'static str> {
        key.validate()?;
        let mut state = self.0.lock().map_err(|_| "source-state")?;
        if let Some(event) = state.entries.get(key) {
            event.revoke()?;
        } else {
            // Early cancellation is terminal even if a delayed start follows.
            if state.closed || state.entries.len() >= 128 {
                return Err("source-capacity");
            }
            let event = Arc::new(StopEvent::new()?);
            event.revoke()?;
            state.entries.insert(key.clone(), event);
        }
        Ok(())
    }
    pub(crate) fn close(&self) -> Result<(), &'static str> {
        let mut state = self.0.lock().map_err(|_| "source-state")?;
        state.closed = true;
        let mut failed = false;
        for event in state.entries.values() {
            failed |= event.revoke().is_err();
        }
        if failed {
            Err("source-stop-failed")
        } else {
            Ok(())
        }
    }
    pub(crate) fn event(&self, key: &SourceKey) -> Result<Arc<StopEvent>, &'static str> {
        key.validate()?;
        let state = self.0.lock().map_err(|_| "source-state")?;
        let event = state.entries.get(key).ok_or("source-required")?;
        if state.closed
            || state.current.as_ref() != Some(key)
            || event.signal_failed()
            || unsafe { WaitForSingleObject(event.handle_for_query(), 0) } != WAIT_TIMEOUT
        {
            return Err("source-ended");
        }
        Ok(event.clone())
    }
}

/// Duplicated from the exact verified root with wait-only rights. Neither page
/// nor coordinator can reset a source ended by the independent root reader.
pub(crate) struct SourceWitness {
    handle: OwnedHandle,
}
impl SourceWitness {
    #[cfg(test)]
    pub(crate) fn no_authority(event: &StopEvent) -> Arc<Self> {
        Arc::new(Self {
            handle: event.reader().unwrap(),
        })
    }
    pub(crate) fn retain(
        parent: &super::parent::Parent,
        raw: usize,
    ) -> Result<Arc<Self>, &'static str> {
        Ok(Arc::new(Self {
            handle: parent.duplicate_wait(raw)?,
        }))
    }
    pub(crate) fn live(&self) -> bool {
        (unsafe { WaitForSingleObject(self.handle.as_raw_handle(), 0) }) == WAIT_TIMEOUT
    }
}
impl crate::cancellation::StopWitness for SourceWitness {
    fn stopped(&self) -> bool {
        !self.live()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(document: u32, operation: &str) -> SourceKey {
        SourceKey {
            document,
            operation: operation.into(),
        }
    }
    #[test]
    fn stop_is_exact_terminal_and_cannot_touch_a_successor_or_other_document() {
        let stops = SourceStops::default();
        let a = key(0, "capture_first");
        let b = key(0, "capture_second");
        stops.begin(&a).unwrap();
        let first = stops.event(&a).unwrap();
        assert!(stops.begin(&b).is_err());
        stops.stop(&key(1, "capture_first")).unwrap();
        assert_eq!(
            unsafe { WaitForSingleObject(first.handle_for_query(), 0) },
            WAIT_TIMEOUT
        );
        stops.stop(&a).unwrap();
        assert!(stops.event(&a).is_err());
        assert!(stops.begin(&a).is_err());
        stops.begin(&b).unwrap();
        let second = stops.event(&b).unwrap();
        stops.stop(&a).unwrap();
        assert_eq!(
            unsafe { WaitForSingleObject(second.handle_for_query(), 0) },
            WAIT_TIMEOUT
        );
        stops.close().unwrap();
        assert!(stops.event(&b).is_err());
        assert!(stops.begin(&key(0, "capture_third")).is_err());
    }
    #[test]
    fn cancellation_before_start_is_a_bounded_terminal_tombstone() {
        let stops = SourceStops::default();
        let first = key(0, "capture_cancelled_00");
        stops.stop(&first).unwrap();
        assert!(stops.begin(&first).is_err());
        assert!(stops.event(&first).is_err());
        for n in 1..128 {
            stops
                .stop(&key(0, &format!("capture_cancelled_{n:02}")))
                .unwrap();
        }
        assert!(stops.stop(&key(0, "capture_overflow")).is_err());
        stops.stop(&first).unwrap();
        stops.close().unwrap();
    }
    #[test]
    fn source_scope_cannot_revoke_an_unrelated_parent_session() {
        let stops = SourceStops::default();
        let key = key(0, "capture_first");
        stops.begin(&key).unwrap();
        let event = stops.event(&key).unwrap();
        struct Witness(Arc<StopEvent>);
        impl crate::cancellation::StopWitness for Witness {
            fn stopped(&self) -> bool {
                (unsafe { WaitForSingleObject(self.0.handle_for_query(), 0) }) != WAIT_TIMEOUT
            }
        }
        let launch = crate::cancellation::SessionStop::from(Arc::new(
            std::sync::atomic::AtomicBool::new(false),
        ));
        let source = launch.scoped(Arc::new(Witness(event)));
        let session = source.descendant();
        let unrelated = launch.descendant();
        stops.stop(&key).unwrap();
        assert!(session.is_stopped());
        assert!(!launch.is_stopped());
        assert!(!unrelated.is_stopped());
    }
}
