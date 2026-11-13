//! Per-session cancellation under one launch emergency stop. Checking authority
//! reads both atomics directly; no propagation thread or registry lock is needed.
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub struct SessionStop {
    launch: Arc<AtomicBool>,
    session: Arc<AtomicBool>,
    witness: Option<Arc<dyn StopWitness>>,
    parent: Option<Arc<SessionStop>>,
}
/// Native-owned liveness only. It never creates permission or cleanup proof.
pub(crate) trait StopWitness: Send + Sync {
    fn stopped(&self) -> bool;
}
impl SessionStop {
    pub(crate) fn child(launch: Arc<AtomicBool>) -> Self {
        Self {
            launch,
            session: Arc::new(AtomicBool::new(false)),
            witness: None,
            parent: None,
        }
    }
    pub(crate) fn monitored(launch: Arc<AtomicBool>, witness: Arc<dyn StopWitness>) -> Self {
        Self {
            witness: Some(witness),
            ..Self::child(launch)
        }
    }
    /// A related session gets its own terminal flag while retaining every
    /// launch/parent witness. Ending it cannot stop unrelated sibling sessions.
    pub(crate) fn descendant(&self) -> Self {
        Self {
            parent: Some(Arc::new(self.clone())),
            witness: self.witness.clone(),
            ..Self::child(self.launch.clone())
        }
    }
    pub fn is_stopped(&self) -> bool {
        if self.session.load(Ordering::SeqCst) {
            return true;
        }
        // A parent-session end is local. Only launch/native witness loss may
        // revoke unrelated siblings; do not conflate these two authorities.
        if self
            .parent
            .as_ref()
            .is_some_and(|parent| parent.is_stopped())
        {
            self.revoke();
            return true;
        }
        if self.launch.load(Ordering::SeqCst)
            || self
                .witness
                .as_ref()
                .is_some_and(|witness| witness.stopped())
        {
            self.revoke_launch();
            return true;
        }
        false
    }
    /// Terminal for this session only. There is deliberately no reset method.
    pub fn revoke(&self) {
        self.session.store(true, Ordering::SeqCst);
    }
    /// OS security transitions affect the entire interactive launch, unlike
    /// ordinary session end. Never expose this as a page-controlled operation.
    pub(crate) fn revoke_launch(&self) {
        self.launch.store(true, Ordering::SeqCst);
        self.revoke();
    }
    pub(crate) fn downgrade(&self) -> Weak<AtomicBool> {
        Arc::downgrade(&self.session)
    }
}
impl From<Arc<AtomicBool>> for SessionStop {
    fn from(launch: Arc<AtomicBool>) -> Self {
        Self::child(launch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descendants_retain_parent_and_native_witness_without_revoking_siblings() {
        struct Witness(Arc<AtomicBool>);
        impl StopWitness for Witness {
            fn stopped(&self) -> bool {
                self.0.load(Ordering::SeqCst)
            }
        }
        let launch = Arc::new(AtomicBool::new(false));
        let native = Arc::new(AtomicBool::new(false));
        let parent = SessionStop::monitored(launch.clone(), Arc::new(Witness(native.clone())));
        let child = parent.descendant();
        let sibling = parent.descendant();
        child.revoke();
        assert!(child.is_stopped());
        assert!(!parent.is_stopped());
        assert!(!sibling.is_stopped());
        native.store(true, Ordering::SeqCst);
        assert!(sibling.is_stopped());
        assert!(parent.is_stopped());
        native.store(false, Ordering::SeqCst);
        assert!(sibling.is_stopped());
        assert!(parent.descendant().is_stopped());
    }
    #[test]
    fn parent_session_end_reaches_descendants_without_ending_other_sessions() {
        let launch = Arc::new(AtomicBool::new(false));
        let parent = SessionStop::child(launch.clone());
        let child = parent.descendant();
        let unrelated = SessionStop::child(launch.clone());
        parent.revoke();
        assert!(child.is_stopped());
        assert!(!unrelated.is_stopped());
        assert!(!launch.load(Ordering::SeqCst));
    }

    #[test]
    fn sibling_stop_is_local_and_launch_stop_is_immediate_and_terminal() {
        let launch = Arc::new(AtomicBool::new(false));
        let first = SessionStop::child(launch.clone());
        let sibling = SessionStop::child(launch.clone());
        let first_worker = first.clone();
        first.revoke();
        assert!(first_worker.is_stopped());
        assert!(!launch.load(Ordering::SeqCst));
        assert!(!sibling.is_stopped());
        launch.store(true, Ordering::SeqCst);
        assert!(sibling.is_stopped());
        assert!(SessionStop::child(launch.clone()).is_stopped());
        // Even a faulty external reset cannot revive a child that observed Stop.
        launch.store(false, Ordering::SeqCst);
        assert!(sibling.is_stopped());
    }

    #[test]
    fn enrollment_revocation_targets_exact_registered_session() {
        let launch = Arc::new(AtomicBool::new(false));
        let first = SessionStop::child(launch.clone());
        let sibling = SessionStop::child(launch);
        let registered = first.downgrade();
        registered.upgrade().unwrap().store(true, Ordering::SeqCst);
        assert!(first.is_stopped());
        assert!(!sibling.is_stopped());
        drop(first);
        assert!(registered.upgrade().is_none());
    }

    #[test]
    fn security_transition_stops_siblings_without_propagation() {
        let launch = Arc::new(AtomicBool::new(false));
        let first = SessionStop::child(launch.clone());
        let sibling = SessionStop::child(launch);
        first.revoke_launch();
        assert!(first.is_stopped());
        assert!(sibling.is_stopped());
    }
}
