//! Monitor-owned executor custody. This records cleanup/process evidence only,
//! never grants, consent or held keys. OS adapters must supply retained-handle
//! observations and authenticated dedicated-pipe receipts, not reported PIDs.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CAPACITY: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Probe,
    Native,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ticket {
    launch: [u8; 32],
    session: String,
    slot: u64,
    execution: u64,
    nonce: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub created: u64,
    pub image: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Reserved,
    Spawning,
    Bound,
    NeverStarted,
}
struct Entry {
    ticket: Ticket,
    mode: Mode,
    phase: Phase,
    process: Option<ProcessIdentity>,
    armed: bool,
    closing: bool,
    child_released: bool,
    mirror_released: bool,
    child_exited: bool,
}
/// Not serialized and not transferable between probe/native modes or launches.
pub struct Completion {
    ticket: Ticket,
    mode: Mode,
}
impl Completion {
    pub fn matches(&self, ticket: &Ticket, mode: Mode) -> bool {
        self.ticket == *ticket && self.mode == mode
    }
}
pub struct Registry {
    launch: [u8; 32],
    image: [u8; 32],
    entries: BTreeMap<u64, Entry>,
    closing: bool,
    coordinator_exited: bool,
}
impl Registry {
    pub fn new(launch: [u8; 32], image: [u8; 32]) -> Self {
        Self {
            launch,
            image,
            entries: BTreeMap::new(),
            closing: false,
            coordinator_exited: false,
        }
    }
    /// Reserve before CreateProcess. The adapter generates fresh CSPRNG nonces;
    /// completed records remain retained to reject replay and generation reuse.
    pub fn reserve(
        &mut self,
        session: &str,
        slot: u64,
        execution: u64,
        nonce: [u8; 32],
        mode: Mode,
    ) -> Result<Ticket, &'static str> {
        if self.closing {
            return Err("custody-closed");
        }
        if !crate::grant::token(session, 8, 128) || slot == 0 || execution == 0 {
            return Err("custody-binding");
        }
        if self.entries.len() >= CAPACITY {
            return Err("custody-capacity");
        }
        if self.entries.contains_key(&execution)
            || self.entries.values().any(|entry| {
                entry.ticket.nonce == nonce
                    || (entry.ticket.session == session && entry.ticket.slot != slot)
                    || (entry.ticket.slot == slot && entry.ticket.session != session)
            })
        {
            return Err("custody-reused");
        }
        let ticket = Ticket {
            launch: self.launch,
            session: session.into(),
            slot,
            execution,
            nonce,
        };
        self.entries.insert(
            execution,
            Entry {
                ticket: ticket.clone(),
                mode,
                phase: Phase::Reserved,
                process: None,
                armed: false,
                closing: false,
                child_released: false,
                mirror_released: false,
                child_exited: false,
            },
        );
        Ok(ticket)
    }
    fn entry(&self, ticket: &Ticket) -> Result<&Entry, &'static str> {
        self.entries
            .get(&ticket.execution)
            .filter(|entry| entry.ticket == *ticket)
            .ok_or("custody-binding")
    }
    fn entry_mut(&mut self, ticket: &Ticket) -> Result<&mut Entry, &'static str> {
        self.entries
            .get_mut(&ticket.execution)
            .filter(|entry| entry.ticket == *ticket)
            .ok_or("custody-binding")
    }
    pub(crate) fn mode(&self, ticket: &Ticket) -> Result<Mode, &'static str> {
        Ok(self.entry(ticket)?.mode)
    }
    pub fn begin_spawn(&mut self, ticket: &Ticket) -> Result<(), &'static str> {
        if self.closing {
            return Err("custody-closed");
        }
        let entry = self.entry_mut(ticket)?;
        if entry.phase != Phase::Reserved || entry.closing {
            return Err("custody-state");
        }
        entry.phase = Phase::Spawning;
        Ok(())
    }
    /// Only a positively known CreateProcess failure with NO created child.
    /// An ambiguous/post-create error must retain its exact handle and bind it.
    pub fn never_started(&mut self, ticket: &Ticket) -> Result<(), &'static str> {
        let entry = self.entry_mut(ticket)?;
        if entry.phase != Phase::Spawning {
            return Err("custody-state");
        }
        entry.phase = Phase::NeverStarted;
        entry.closing = true;
        Ok(())
    }
    pub fn bind(&mut self, ticket: &Ticket, process: ProcessIdentity) -> Result<(), &'static str> {
        if process.pid == 0 || process.created == 0 || process.image != self.image {
            return Err("custody-process");
        }
        let entry = self.entry_mut(ticket)?;
        if entry.phase != Phase::Spawning {
            return Err("custody-state");
        }
        entry.process = Some(process);
        entry.phase = Phase::Bound;
        Ok(())
    }
    /// Linearize BEFORE any ARM write/queue operation. A lost/partial ACK stays
    /// armed-pending; neither timeout nor cancellation can undo this fact.
    pub fn arm(&mut self, ticket: &Ticket) -> Result<(), &'static str> {
        if self.closing {
            return Err("custody-closed");
        }
        let entry = self.entry_mut(ticket)?;
        if entry.phase != Phase::Bound || entry.closing || entry.armed || entry.child_exited {
            return Err("custody-state");
        }
        entry.armed = true;
        Ok(())
    }
    /// The adapter closes all possible ARM senders when sealing cancellation.
    /// Because Armed is recorded first, an already queued ARM cannot be erased.
    pub fn stop(&mut self, ticket: &Ticket) -> Result<(), &'static str> {
        let entry = self.entry_mut(ticket)?;
        entry.closing = true;
        if entry.phase == Phase::Reserved {
            entry.phase = Phase::NeverStarted;
        }
        Ok(())
    }
    pub fn close_admission(&mut self) {
        self.closing = true;
        for entry in self.entries.values_mut() {
            entry.closing = true;
            if entry.phase == Phase::Reserved {
                entry.phase = Phase::NeverStarted;
            }
        }
    }
    pub fn coordinator_exited(&mut self) {
        self.coordinator_exited = true;
        self.close_admission();
    }
    pub fn child_exited(
        &mut self,
        ticket: &Ticket,
        process: ProcessIdentity,
    ) -> Result<(), &'static str> {
        let entry = self.entry_mut(ticket)?;
        if entry.process != Some(process) {
            return Err("custody-process");
        }
        entry.child_exited = true;
        entry.closing = true;
        Ok(())
    }
    /// A terminal child receipt means its injectors are fenced, owned intents
    /// settled and no future DOWN possible. It does NOT settle a live mirror.
    pub fn child_released(&mut self, ticket: &Ticket, mode: Mode) -> Result<(), &'static str> {
        let entry = self.entry_mut(ticket)?;
        if entry.mode != mode || entry.phase != Phase::Bound || entry.child_released {
            return Err("custody-receipt");
        }
        entry.child_released = true;
        entry.closing = true;
        Ok(())
    }
    /// The trusted coordinator must fence/reap this child and settle its exact
    /// mirrored intent before emitting this receipt on the dedicated channel.
    pub fn mirror_released(&mut self, ticket: &Ticket, mode: Mode) -> Result<(), &'static str> {
        let entry = self.entry_mut(ticket)?;
        if entry.mode != mode || !entry.child_exited || entry.mirror_released {
            return Err("custody-receipt");
        }
        entry.mirror_released = true;
        Ok(())
    }
    pub fn input_settled(&self, ticket: &Ticket) -> Result<bool, &'static str> {
        let entry = self.entry(ticket)?;
        Ok(if !entry.armed {
            entry.closing
        } else {
            entry.child_exited
                && (entry.mirror_released || (self.coordinator_exited && entry.child_released))
        })
    }
    pub fn completion(&self, ticket: &Ticket) -> Result<Option<Completion>, &'static str> {
        let entry = self.entry(ticket)?;
        let process_complete = entry.phase == Phase::NeverStarted || entry.child_exited;
        Ok(
            (process_complete && self.input_settled(ticket)?).then(|| Completion {
                ticket: ticket.clone(),
                mode: entry.mode,
            }),
        )
    }
    /// Includes ONLY registered entries, not arbitrary descendants/resources.
    /// Full launch cleanup additionally seals admission and verifies actual jobs.
    pub fn known_entries_complete(&self) -> bool {
        self.entries.values().all(|entry| {
            self.completion(&entry.ticket)
                .is_ok_and(|proof| proof.is_some())
        })
    }
    pub fn sealed_entries_complete(&self) -> bool {
        self.closing && self.known_entries_complete()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn process() -> ProcessIdentity {
        ProcessIdentity {
            pid: 123,
            created: 456,
            image: [2; 32],
        }
    }
    fn reserved(mode: Mode) -> (Registry, Ticket) {
        let mut registry = Registry::new([1; 32], [2; 32]);
        let ticket = registry
            .reserve("session_fixture_01", 1, 1, [3; 32], mode)
            .unwrap();
        (registry, ticket)
    }
    fn bound(mode: Mode) -> (Registry, Ticket) {
        let (mut registry, ticket) = reserved(mode);
        registry.begin_spawn(&ticket).unwrap();
        registry.bind(&ticket, process()).unwrap();
        (registry, ticket)
    }
    fn armed() -> (Registry, Ticket) {
        let (mut registry, ticket) = bound(Mode::Native);
        registry.arm(&ticket).unwrap();
        (registry, ticket)
    }
    #[test]
    fn live_coordinator_requires_its_mirror_even_after_child_receipt_and_exit() {
        let (mut registry, ticket) = armed();
        registry.child_released(&ticket, Mode::Native).unwrap();
        assert!(!registry.input_settled(&ticket).unwrap());
        assert!(registry.mirror_released(&ticket, Mode::Native).is_err());
        registry.child_exited(&ticket, process()).unwrap();
        assert!(!registry.input_settled(&ticket).unwrap());
        registry.mirror_released(&ticket, Mode::Native).unwrap();
        assert!(
            registry
                .completion(&ticket)
                .unwrap()
                .unwrap()
                .matches(&ticket, Mode::Native)
        );
        assert!(registry.known_entries_complete());
    }
    #[test]
    fn child_recovery_needs_actual_coordinator_and_child_exit() {
        for child_first in [true, false] {
            let (mut registry, ticket) = armed();
            registry.child_released(&ticket, Mode::Native).unwrap();
            if child_first {
                registry.child_exited(&ticket, process()).unwrap();
            }
            assert!(!registry.input_settled(&ticket).unwrap());
            registry.coordinator_exited();
            if !child_first {
                assert!(registry.completion(&ticket).unwrap().is_none());
                registry.child_exited(&ticket, process()).unwrap();
            }
            assert!(registry.completion(&ticket).unwrap().is_some());
        }
    }
    #[test]
    fn both_processes_dead_without_positive_receipt_remains_unproven() {
        let (mut registry, ticket) = armed();
        registry.stop(&ticket).unwrap();
        registry.child_exited(&ticket, process()).unwrap();
        registry.coordinator_exited();
        assert!(!registry.input_settled(&ticket).unwrap());
        assert!(!registry.known_entries_complete());
        // No time, exit-code or job-zero method exists that can manufacture proof.
    }
    #[test]
    fn mirror_may_recover_a_fenced_child_without_a_child_receipt() {
        let (mut registry, ticket) = armed();
        registry.child_exited(&ticket, process()).unwrap();
        assert!(registry.completion(&ticket).unwrap().is_none());
        registry.mirror_released(&ticket, Mode::Native).unwrap();
        assert!(registry.completion(&ticket).unwrap().is_some());
    }
    #[test]
    fn cancellation_linearizes_against_arm_and_does_not_erase_ambiguous_delivery() {
        for arm_first in [true, false] {
            let (mut registry, ticket) = bound(Mode::Native);
            if arm_first {
                registry.arm(&ticket).unwrap();
            }
            registry.stop(&ticket).unwrap();
            assert!(registry.arm(&ticket).is_err());
            assert_eq!(registry.input_settled(&ticket).unwrap(), !arm_first);
            assert!(registry.completion(&ticket).unwrap().is_none());
            registry.child_exited(&ticket, process()).unwrap();
            assert_eq!(registry.completion(&ticket).unwrap().is_some(), !arm_first);
            if arm_first {
                registry.mirror_released(&ticket, Mode::Native).unwrap();
            }
            assert!(registry.known_entries_complete());
        }
    }
    #[test]
    fn spawn_and_startup_boundaries_retain_pending_process_ownership() {
        let (mut registry, ticket) = reserved(Mode::Native);
        assert!(registry.arm(&ticket).is_err());
        registry.begin_spawn(&ticket).unwrap();
        registry.coordinator_exited();
        assert!(registry.input_settled(&ticket).unwrap());
        assert!(registry.completion(&ticket).unwrap().is_none());
        registry.bind(&ticket, process()).unwrap();
        assert!(registry.arm(&ticket).is_err());
        registry.child_exited(&ticket, process()).unwrap();
        assert!(registry.known_entries_complete());
        let (mut registry, ticket) = reserved(Mode::Native);
        registry.begin_spawn(&ticket).unwrap();
        registry.never_started(&ticket).unwrap();
        assert!(registry.completion(&ticket).unwrap().is_some());
        assert!(registry.bind(&ticket, process()).is_err());
    }
    #[test]
    fn wrong_ticket_process_image_and_replayed_receipts_cannot_clear_ownership() {
        let (mut registry, ticket) = armed();
        for field in 0..5 {
            let mut wrong = ticket.clone();
            match field {
                0 => wrong.launch[0] ^= 1,
                1 => wrong.session.push('x'),
                2 => wrong.slot += 1,
                3 => wrong.execution += 1,
                _ => wrong.nonce[0] ^= 1,
            }
            assert!(registry.child_released(&wrong, Mode::Native).is_err());
        }
        for field in 0..3 {
            let mut wrong = process();
            match field {
                0 => wrong.pid += 1,
                1 => wrong.created += 1,
                _ => wrong.image[0] ^= 1,
            }
            assert!(registry.child_exited(&ticket, wrong).is_err());
        }
        registry.child_released(&ticket, Mode::Native).unwrap();
        assert!(registry.child_released(&ticket, Mode::Native).is_err());
        assert!(!registry.known_entries_complete());
        let (mut registry, ticket) = reserved(Mode::Native);
        registry.begin_spawn(&ticket).unwrap();
        let mut wrong = process();
        wrong.image[0] ^= 1;
        assert!(registry.bind(&ticket, wrong).is_err());
        assert!(registry.arm(&ticket).is_err());
    }
    #[test]
    fn probe_evidence_never_matches_native_scope() {
        let (mut registry, ticket) = bound(Mode::Probe);
        registry.arm(&ticket).unwrap();
        assert!(registry.child_released(&ticket, Mode::Native).is_err());
        registry.child_released(&ticket, Mode::Probe).unwrap();
        registry.child_exited(&ticket, process()).unwrap();
        registry.coordinator_exited();
        let proof = registry.completion(&ticket).unwrap().unwrap();
        assert!(proof.matches(&ticket, Mode::Probe));
        assert!(!proof.matches(&ticket, Mode::Native));
    }
    #[test]
    fn late_old_generation_cannot_complete_a_successor_or_change_its_binding() {
        let (mut registry, old) = armed();
        registry.child_exited(&old, process()).unwrap();
        registry.mirror_released(&old, Mode::Native).unwrap();
        let next = registry
            .reserve("session_fixture_01", 1, 2, [4; 32], Mode::Native)
            .unwrap();
        registry.begin_spawn(&next).unwrap();
        registry.bind(&next, process()).unwrap();
        registry.arm(&next).unwrap();
        assert!(registry.mirror_released(&old, Mode::Native).is_err());
        assert!(registry.completion(&next).unwrap().is_none());
        assert!(
            registry
                .reserve("session_fixture_01", 2, 3, [5; 32], Mode::Native)
                .is_err()
        );
        assert!(
            registry
                .reserve("session_other_01", 1, 3, [5; 32], Mode::Native)
                .is_err()
        );
    }
    #[test]
    fn completed_tickets_are_not_evicted_to_bypass_capacity_or_nonce_reuse() {
        let mut registry = Registry::new([1; 32], [2; 32]);
        for execution in 1..=CAPACITY as u64 {
            let ticket = registry
                .reserve(
                    "session_fixture_01",
                    1,
                    execution,
                    [execution as u8; 32],
                    Mode::Probe,
                )
                .unwrap();
            registry.stop(&ticket).unwrap();
        }
        assert!(registry.known_entries_complete());
        assert!(
            registry
                .reserve("session_fixture_01", 1, 129, [129; 32], Mode::Probe)
                .is_err()
        );
        let (mut registry, _) = reserved(Mode::Native);
        assert!(
            registry
                .reserve("session_fixture_01", 1, 2, [3; 32], Mode::Native)
                .is_err()
        );
        registry.close_admission();
        assert!(
            registry
                .reserve("session_fixture_01", 1, 2, [4; 32], Mode::Native)
                .is_err()
        );
    }
}
