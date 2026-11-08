//! Inactive native receiver guard for docs/51 §4.4 profile-2 input.
//! Arrival is a local monotonic elapsed-millisecond value since this peer's
//! creation. Never compare wall clocks of the two endpoints or reuse on a new peer.
#[derive(Debug, Default)]
pub struct InputAgeGuard {
    baseline: Option<i128>,
    last_sender_ms: Option<u64>,
}

impl InputAgeGuard {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn observe(
        &mut self,
        sender_ms: u64,
        arrival_ms: u64,
        release: bool,
    ) -> Result<(), &'static str> {
        if self.last_sender_ms.is_some_and(|last| sender_ms < last) {
            return Err("replay");
        }
        self.last_sender_ms = Some(sender_ms);
        let delta = i128::from(arrival_ms) - i128::from(sender_ms);
        let baseline = self.baseline.map_or(delta, |prior| prior.min(delta));
        self.baseline = Some(baseline);
        if !release && delta - baseline > 300 {
            return Err("stale-input");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_profile2_input_age_sequences() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/assist-control-v2-profile2.json"
        )))
        .unwrap();
        for scenario in cases["inputAge"]["sequences"].as_array().unwrap() {
            let mut age = InputAgeGuard::new();
            for event in scenario["events"].as_array().unwrap() {
                let result = age.observe(
                    event["t"].as_u64().unwrap(),
                    event["arrival"].as_u64().unwrap(),
                    event["release"].as_bool().unwrap(),
                );
                let actual = result.map(|_| "accepted").unwrap_or_else(|error| error);
                assert_eq!(actual, event["expect"].as_str().unwrap());
            }
        }
    }

    #[test]
    fn peer_reset_and_replay_release() {
        let mut old = InputAgeGuard::new();
        assert!(old.observe(200, 500, false).is_ok());
        assert_eq!(old.observe(199, 501, true), Err("replay"));
        let mut replacement = InputAgeGuard::new();
        assert!(replacement.observe(0, 100_000, false).is_ok());
        assert_eq!(replacement.observe(1, 100_500, false), Err("stale-input"));
        assert!(replacement.observe(2, 100_501, true).is_ok());
    }
}
