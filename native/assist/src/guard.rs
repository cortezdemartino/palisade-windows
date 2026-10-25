use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

// Independent of pipe I/O and renderer progress. No active native authority exists yet.
pub struct Watchdog {
    stopped: Arc<AtomicBool>,
    started: Instant,
    heartbeat: Instant,
    last_wall: u64,
    handshaken: bool,
}
impl Watchdog {
    pub fn new(stopped: Arc<AtomicBool>, wall: u64, now: Instant) -> Self {
        Self {
            stopped,
            started: now,
            heartbeat: now,
            last_wall: wall,
            handshaken: false,
        }
    }
    pub fn handshake(&mut self, now: Instant) -> Result<(), &'static str> {
        if self.handshaken || self.stopped.load(Ordering::SeqCst) {
            return Err("invalid-state");
        }
        self.handshaken = true;
        self.heartbeat = now;
        Ok(())
    }
    pub fn touch(&mut self, now: Instant) {
        self.heartbeat = now
    }
    pub fn check(&mut self, wall: u64, now: Instant, parent_alive: bool) -> bool {
        let invalid = !parent_alive
            || wall < self.last_wall
            || (!self.handshaken && now.duration_since(self.started) >= Duration::from_secs(5))
            || (self.handshaken && now.duration_since(self.heartbeat) >= Duration::from_secs(5));
        self.last_wall = wall;
        if invalid {
            self.stopped.store(true, Ordering::SeqCst)
        }
        self.stopped.load(Ordering::SeqCst)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stalled_pipe_parent_exit_clock_and_terminal_stop() {
        for reason in 0..4 {
            let stop = Arc::new(AtomicBool::new(false));
            let now = Instant::now();
            let mut guard = Watchdog::new(stop.clone(), 1000, now);
            guard.handshake(now).unwrap();
            assert!(!guard.check(1001, now + Duration::from_millis(1), true));
            match reason {
                0 => assert!(guard.check(6001, now + Duration::from_secs(5), true)),
                1 => assert!(guard.check(1002, now, false)),
                2 => assert!(guard.check(999, now, true)),
                _ => {
                    stop.store(true, Ordering::SeqCst);
                    assert!(guard.check(1002, now, true))
                }
            }
            assert!(guard.handshake(now).is_err());
            guard.touch(now);
            assert!(guard.check(1003, now, true));
        }
    }
    #[test]
    fn handshake_is_bounded_and_cannot_repeat() {
        let stop = Arc::new(AtomicBool::new(false));
        let now = Instant::now();
        let mut guard = Watchdog::new(stop, 0, now);
        assert!(guard.check(5000, now + Duration::from_secs(5), true));
        assert!(guard.handshake(now).is_err());
    }
}
