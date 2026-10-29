//! Join evidence is sticky: a failed/panicked worker never becomes successful
//! merely because its handle was consumed. Never call join on an urgent path.
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle, ThreadId},
};
type Outcome = Result<(), &'static str>;
pub fn attempt(work: impl FnOnce() -> Outcome) -> Outcome {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)) {
        Ok(outcome) => outcome,
        Err(payload) => {
            discard_panic(payload);
            Err("cleanup-panicked")
        }
    }
}
fn discard_panic(payload: Box<dyn std::any::Any + Send>) {
    // Panic payload destruction can itself panic. Contain that outside owner
    // locks; forget only a recursive payload rather than unwind through cleanup.
    if let Err(recursive) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload)))
    {
        std::mem::forget(recursive);
    }
}
struct State {
    worker: Option<JoinHandle<Outcome>>,
    outcome: Option<Outcome>,
}
pub struct RetainedWorker {
    state: Mutex<State>,
    id: ThreadId,
    self_join: AtomicBool,
}
impl RetainedWorker {
    pub fn new(worker: JoinHandle<Outcome>) -> Self {
        Self {
            id: worker.thread().id(),
            self_join: AtomicBool::new(false),
            state: Mutex::new(State {
                worker: Some(worker),
                outcome: None,
            }),
        }
    }
    pub fn join(&self) -> Outcome {
        // Check outside the join mutex: an external owner may already hold it
        // while awaiting this worker. Do not consume the handle on self-join.
        if thread::current().id() == self.id {
            self.self_join.store(true, Ordering::SeqCst);
            return Err("cleanup-self-join");
        }
        let mut state = self.state.lock().map_err(|_| "cleanup-state-failed")?;
        if let Some(outcome) = state.outcome {
            return outcome;
        }
        let joined = state.worker.take().ok_or("cleanup-worker-missing")?.join();
        let (mut outcome, payload) = match joined {
            Ok(outcome) => (outcome, None),
            Err(payload) => (Err("cleanup-worker-panicked"), Some(payload)),
        };
        if self.self_join.load(Ordering::SeqCst) {
            outcome = Err("cleanup-self-join");
        }
        state.outcome = Some(outcome);
        drop(state);
        if let Some(payload) = payload {
            discard_panic(payload);
        }
        outcome
    }
    pub fn is_finished(&self) -> bool {
        self.state.try_lock().is_ok_and(|state| {
            state.outcome.is_some() || state.worker.as_ref().is_some_and(JoinHandle::is_finished)
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
        time::Duration,
    };
    #[test]
    fn waits_for_real_exit_and_serializes_concurrent_joins_without_blocking_revocation() {
        let stop = Arc::new(AtomicBool::new(false));
        let (release, wait) = mpsc::channel();
        let worker = Arc::new(RetainedWorker::new(thread::spawn(move || {
            wait.recv().unwrap();
            Ok(())
        })));
        let (done, completion) = mpsc::channel();
        let callers: Vec<_> = (0..2)
            .map(|_| {
                let worker = worker.clone();
                let done = done.clone();
                thread::spawn(move || {
                    done.send(worker.join()).unwrap();
                })
            })
            .collect();
        assert!(completion.recv_timeout(Duration::from_millis(25)).is_err());
        stop.store(true, Ordering::SeqCst);
        assert!(stop.load(Ordering::SeqCst));
        assert!(!worker.is_finished());
        release.send(()).unwrap();
        for _ in 0..2 {
            assert_eq!(
                completion.recv_timeout(Duration::from_secs(2)).unwrap(),
                Ok(())
            );
        }
        for caller in callers {
            caller.join().unwrap();
        }
        assert!(worker.is_finished());
        assert_eq!(worker.join(), Ok(()));
    }
    #[test]
    fn panics_and_returned_failures_are_sticky_across_repeated_joins() {
        for panic in [false, true] {
            let worker = RetainedWorker::new(thread::spawn(move || {
                if panic {
                    panic!("intentional cleanup fixture");
                }
                Err("cleanup-fixture-failed")
            }));
            let first = worker.join();
            assert!(first.is_err());
            assert_eq!(worker.join(), first);
        }
    }
    #[test]
    fn self_join_cannot_deadlock_an_external_join_or_consume_its_handle() {
        let (publish, receive) = mpsc::channel::<Arc<RetainedWorker>>();
        let worker = Arc::new(RetainedWorker::new(thread::spawn(move || {
            let own = receive.recv().unwrap();
            assert_eq!(own.join(), Err("cleanup-self-join"));
            Ok(())
        })));
        let owned = worker.clone();
        let parent = thread::spawn(move || owned.join());
        publish.send(worker.clone()).unwrap();
        assert_eq!(parent.join().unwrap(), Err("cleanup-self-join"));
        assert_eq!(worker.join(), Err("cleanup-self-join"));
        assert!(worker.is_finished());
    }
    #[test]
    fn panicking_cleanup_does_not_prevent_attempting_other_owners() {
        let mut visited = Vec::new();
        assert!(
            attempt(|| {
                visited.push(1);
                panic!("intentional cleanup fixture")
            })
            .is_err()
        );
        assert!(
            attempt(|| {
                visited.push(2);
                Ok(())
            })
            .is_ok()
        );
        assert_eq!(visited, [1, 2]);
    }
    #[test]
    fn finish_and_destructor_panics_do_not_skip_later_cleanup() {
        struct Owner;
        impl Owner {
            fn finish(&mut self) -> Outcome {
                panic!("intentional finish fixture")
            }
        }
        impl Drop for Owner {
            fn drop(&mut self) {
                panic!("intentional drop fixture");
            }
        }
        let mut owner = Owner;
        let finish = attempt(|| owner.finish());
        let destroyed = attempt(|| {
            drop(owner);
            Ok(())
        });
        let mut later = false;
        assert!(
            attempt(|| {
                later = true;
                Ok(())
            })
            .is_ok()
        );
        assert!(finish.is_err() && destroyed.is_err() && later);
    }
    #[test]
    fn panic_payload_destructors_cannot_unwind_through_cleanup_owners() {
        struct Payload;
        impl Drop for Payload {
            fn drop(&mut self) {
                panic!("intentional payload destructor");
            }
        }
        assert!(attempt(|| std::panic::panic_any(Payload)).is_err());
        let worker = RetainedWorker::new(thread::spawn(|| std::panic::panic_any(Payload)));
        assert_eq!(worker.join(), Err("cleanup-worker-panicked"));
        assert_eq!(worker.join(), Err("cleanup-worker-panicked"));
        assert_eq!(attempt(|| Ok(())), Ok(()));
    }
}
