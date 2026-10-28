//! Private main/native channel, separate from diagnostic guardian v1. This first
//! revision owns no sessions or sensitive resources; its completion proof is
//! deliberately limited to this channel. It cannot attest descendant cleanup.
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

pub const PROTOCOL: &str = "palisade.assist.live";
pub const MAX_FRAME: usize = 65_536;
pub const ORDINARY_CAPACITY: usize = 8;
pub const OUTPUT_CAPACITY: usize = 10;

#[derive(Default)]
pub struct Signals {
    revoked: AtomicBool,
    shutdown: AtomicBool,
    failed: AtomicBool,
}
impl Signals {
    /// No locks, pipe writes, joins, UI or network work on the urgent path.
    pub fn revoke(&self) {
        self.revoked.store(true, Ordering::SeqCst);
    }
    pub fn shutdown(&self) {
        self.revoke();
        self.shutdown.store(true, Ordering::SeqCst);
    }
    pub fn fail(&self) {
        self.failed.store(true, Ordering::SeqCst);
        self.shutdown();
    }
    pub fn revoked(&self) -> bool {
        self.revoked.load(Ordering::SeqCst)
    }
    pub fn closing(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::SeqCst)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub sequence: u32,
    pub command: Command,
}
#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields, rename_all = "kebab-case")]
pub enum Command {
    Hello {
        protocol: String,
        version: u32,
        challenge: String,
    },
    Pulse {},
    Status {},
    Shutdown {},
    // Only a bounded, cancellable scheduling fixture. Never invokes OS adapters.
    ProbeWait {
        milliseconds: u16,
    },
}
pub struct Ingress {
    next: u32,
    challenge: Option<String>,
    probe: bool,
    closed: bool,
}
impl Ingress {
    pub fn new(probe: bool) -> Self {
        Self {
            next: 0,
            challenge: None,
            probe,
            closed: false,
        }
    }
    pub fn accept(&mut self, bytes: &[u8], signals: &Signals) -> Result<Request, &'static str> {
        let result = (|| {
            if self.closed || signals.closing() || bytes.is_empty() || bytes.len() > MAX_FRAME {
                return Err("live-unavailable");
            }
            let request: Request = serde_json::from_slice(bytes).map_err(|_| "invalid-message")?;
            if request.sequence != self.next {
                return Err("replay");
            }
            self.next = self.next.checked_add(1).ok_or("sequence-exhausted")?;
            match &request.command {
                Command::Hello {
                    protocol,
                    version,
                    challenge,
                } if self.challenge.is_none() => {
                    if protocol != PROTOCOL || *version != 1 {
                        return Err("incompatible-protocol");
                    }
                    crate::grant::decode::<32>(challenge)?;
                    self.challenge = Some(challenge.clone());
                }
                _ if self.challenge.is_none() => return Err("handshake-required"),
                Command::Hello { .. } => return Err("repeated-handshake"),
                Command::Shutdown {} => {
                    self.closed = true;
                    signals.revoke();
                }
                Command::ProbeWait { milliseconds }
                    if !self.probe || *milliseconds == 0 || *milliseconds > 10_000 =>
                {
                    return Err("operation-unavailable");
                }
                _ => (),
            }
            Ok(request)
        })();
        if result.is_err() {
            signals.fail();
        }
        result
    }
    pub fn challenge(&self) -> Option<&str> {
        self.challenge.as_deref()
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Reply {
    Hello {
        protocol: &'static str,
        version: u32,
        challenge: String,
        mode: &'static str,
        #[serde(rename = "nativeInput")]
        native_input: bool,
        #[serde(rename = "cleanupScope")]
        cleanup_scope: &'static str,
    },
    Pulse,
    Status {
        state: &'static str,
        authority: &'static str,
    },
    ProbeWait,
    CleanupComplete {
        challenge: String,
        scope: &'static str,
    },
}
#[derive(Serialize)]
pub struct Response {
    pub sequence: u32,
    pub result: Reply,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn wire(sequence: u32, command: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&json!({"sequence":sequence,"command":command})).unwrap()
    }
    fn hello() -> serde_json::Value {
        json!({"type":"hello","protocol":PROTOCOL,"version":1,"challenge":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"})
    }
    #[test]
    fn urgent_shutdown_is_terminal_before_ordinary_dispatch() {
        let signals = Signals::default();
        let mut ingress = Ingress::new(true);
        ingress.accept(&wire(0, hello()), &signals).unwrap();
        ingress
            .accept(
                &wire(1, json!({"type":"probe-wait","milliseconds":10000})),
                &signals,
            )
            .unwrap();
        ingress
            .accept(&wire(2, json!({"type":"shutdown"})), &signals)
            .unwrap();
        // Pause at the reader/dispatcher race: revocation is already effective,
        // but dispatch must wait until the shutdown metadata has been published.
        assert!(signals.revoked() && !signals.closing() && !signals.failed());
        signals.shutdown();
        assert!(
            ingress
                .accept(&wire(3, json!({"type":"pulse"})), &signals)
                .is_err()
        );
    }
    #[test]
    fn closed_types_duplicates_replays_and_invalid_challenges_fail_closed() {
        for bytes in [
            wire(0, json!({"type":"status"})),
            wire(1, hello()),
            wire(
                0,
                json!({"type":"hello","protocol":PROTOCOL,"version":2,"challenge":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}),
            ),
            wire(
                0,
                json!({"type":"hello","protocol":PROTOCOL,"version":1,"challenge":"bad"}),
            ),
            b"{\"sequence\":0,\"sequence\":0,\"command\":{\"type\":\"shutdown\"}}".to_vec(),
            vec![b' '; MAX_FRAME + 1],
        ] {
            let signals = Signals::default();
            assert!(Ingress::new(true).accept(&bytes, &signals).is_err());
            assert!(signals.revoked() && signals.failed());
        }
        for command in [
            hello(),
            json!({"type":"status","approved":true}),
            json!({"type":"key","down":true}),
            json!({"type":"shutdown","reason":"arbitrary"}),
        ] {
            let signals = Signals::default();
            let mut ingress = Ingress::new(true);
            ingress.accept(&wire(0, hello()), &signals).unwrap();
            assert!(ingress.accept(&wire(1, command), &signals).is_err());
            assert!(signals.failed());
        }
    }
    #[test]
    fn compiled_features_never_make_probe_commands_authorizing() {
        let signals = Signals::default();
        let mut ingress = Ingress::new(false);
        ingress.accept(&wire(0, hello()), &signals).unwrap();
        assert!(
            ingress
                .accept(
                    &wire(1, json!({"type":"probe-wait","milliseconds":1})),
                    &signals
                )
                .is_err()
        );
        assert!(signals.failed());
        assert_eq!(crate::framing::MAX_FRAME, 16_384);
    }
}
