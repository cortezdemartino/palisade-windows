use super::*;
use crate::stream::{Binding, SignedStreamGrant, SignedStreamLease};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signer, SigningKey};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};
const SEND: &str = "grant_send_fixture_01";
const RECEIVE: &str = "grant_receive_fixture_01";
const BYTES: &[u8] = b"approved temporary fixture bytes";
struct Fixture {
    files: Arc<SessionFiles>,
    gate: Arc<Mutex<SessionGate>>,
    stop: Arc<AtomicBool>,
    clock: Arc<AtomicU64>,
    root: PathBuf,
}
fn wait_until(mut predicate: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(
            Instant::now() < until,
            "native fixture exceeded cleanup deadline"
        );
        thread::sleep(Duration::from_millis(5));
    }
}
fn metadata() -> Metadata {
    Metadata {
        name: "received.txt".into(),
        size: BYTES.len() as u64,
        sha256: super::super::hex(&sha2::Sha256::digest(BYTES)),
    }
}
use sha2::Digest;
impl Fixture {
    fn new() -> Self {
        Self::with_consent(true, true)
    }
    fn with_consent(consent: bool, file_confirmation: bool) -> Self {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../../tests/fixtures/assist-control-v2.json"
        ))
        .unwrap();
        let now = fixture["now"].as_u64().unwrap();
        let key = SigningKey::from_bytes(&[42; 32]);
        let instant = Instant::now();
        let base: SignedStreamGrant =
            serde_json::from_value(fixture["grants"]["valid"][1]["signed"].clone()).unwrap();
        let g = &base.grant;
        let binding = Binding {
            session: g.session.clone(),
            room: g.room.clone(),
            stream: g.stream.clone(),
            publisher_account: g.publisher_account.clone(),
            publisher_device: g.publisher_device.clone(),
            helper_account: g.helper_account.clone(),
            helper_device: g.helper_device.clone(),
            helper_transport: g.helper_transport.clone(),
            endpoint: g.endpoint.clone().unwrap(),
        };
        let mut gate = SessionGate::new(key.verifying_key().to_bytes(), binding, now, 0, instant);
        let mut ids = Vec::new();
        for (index, (id, capability)) in [
            (SEND, "file_to_guest"),
            (RECEIVE, "file_to_host"),
            ("grant_keyboard_fixture_01", "control_keyboard"),
        ]
        .iter()
        .enumerate()
        {
            let mut grant = g.clone();
            grant.grant_id = (*id).into();
            grant.capability = (*capability).into();
            grant.generation = 3 + index as u64;
            grant.approved_at = now - 1000;
            grant.issued_at = now - 10;
            grant.session_deadline = now + 120000;
            grant.expires_at = now + if index == 0 { 5000 } else { 100000 };
            if index < 2 {
                grant.endpoint = None;
            }
            let signed = SignedStreamGrant {
                signature: URL_SAFE_NO_PAD.encode(key.sign(&grant.canonical()).to_bytes()),
                grant,
            };
            gate.admit(&signed, now).unwrap();
            if consent {
                gate.consent(id, now).unwrap();
            }
            ids.push((*id).to_string());
        }
        ids.sort();
        gate.verify_peer(&g.helper_transport).unwrap();
        let mut lease: SignedStreamLease =
            serde_json::from_value(fixture["leases"]["valid"][0]["signed"].clone()).unwrap();
        lease.lease.grants = ids;
        lease.lease.generation = 5;
        lease.lease.issued_at = now;
        lease.lease.expires_at = now + 20000;
        lease.lease.session_deadline = now + 120000;
        lease.signature = URL_SAFE_NO_PAD.encode(
            key.sign(&serde_json::to_vec(&lease.lease).unwrap())
                .to_bytes(),
        );
        gate.refresh(&lease, now, instant, instant).unwrap();
        let gate = Arc::new(Mutex::new(gate));
        let clock = Arc::new(AtomicU64::new(now));
        let stop = Arc::new(AtomicBool::new(false));
        // These seams are test-only. They simulate time/platform and a separate
        // native file decision; no test touches the personal clipboard/input/UI.
        let files = Arc::new(
            SessionFiles::create(Authority {
                gate: gate.clone(),
                stop: stop.clone(),
                clock: Some(clock.clone()),
                file_confirmation: Some(file_confirmation),
            })
            .unwrap(),
        );
        let root = std::env::temp_dir().join(format!(
            "palisade-bound-file-test-{}",
            super::super::identifier().unwrap()
        ));
        std::fs::create_dir(&root).unwrap();
        Self {
            files,
            gate,
            stop,
            clock,
            root,
        }
    }
    fn source(&self) -> PathBuf {
        let path = self.root.join("selected.txt");
        std::fs::write(&path, BYTES).unwrap();
        path
    }
    fn selected_send(
        &self,
        request: &FileRequest,
        path: &Path,
    ) -> Result<FileSelection, &'static str> {
        self.files.select_with(request, |store, op| {
            store.open_send(path, &mut || self.files.state.check(op))
        })
    }
    fn selected_receive(
        &self,
        request: &FileRequest,
        path: &Path,
    ) -> Result<FileSelection, &'static str> {
        self.files.select_with(request, |store, op| {
            let metadata = op.metadata.lock().unwrap().clone().unwrap();
            let token =
                store.open_receive(path, metadata.clone(), &mut || self.files.state.check(op))?;
            Ok((token, metadata))
        })
    }
    fn empty(&self) -> bool {
        self.files.state.stores.iter().all(|store| {
            store
                .try_lock()
                .is_ok_and(|store| store.transfers.is_empty())
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.files.close();
        wait_until(|| self.empty());
        let path = self.root.canonicalize().unwrap();
        let temp = std::env::temp_dir().canonicalize().unwrap();
        assert!(
            path.starts_with(temp)
                && path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("palisade-bound-file-test-")
        );
        std::fs::remove_dir_all(path).unwrap();
    }
}
#[test]
fn native_authority_and_separate_file_confirmation_are_required_before_creation() {
    let denied = Fixture::with_consent(false, true);
    assert!(
        denied
            .files
            .request_send(SEND, "transfer_denied_01")
            .is_err()
    );
    assert!(
        denied
            .files
            .request_receive(RECEIVE, "transfer_denied_02", metadata())
            .is_err()
    );
    let file_denied = Fixture::with_consent(true, false);
    let request = file_denied
        .files
        .request_receive(RECEIVE, "transfer_denied_03", metadata())
        .unwrap();
    assert!(
        file_denied
            .selected_receive(&request, &file_denied.root.join("denied.txt"))
            .is_err()
    );
    assert_eq!(std::fs::read_dir(&file_denied.root).unwrap().count(), 0);
    let allowed = Fixture::new();
    assert!(
        allowed
            .files
            .request_send(RECEIVE, "transfer_wrong_direction")
            .is_err()
    );
    assert!(allowed.files.request_send(SEND, "bad").is_err());
    if !cfg!(feature = "native-resources") {
        assert!(SessionFiles::publisher(allowed.gate.clone(), allowed.stop.clone()).is_err());
    }
}
#[test]
fn requests_tokens_directions_and_selected_metadata_cannot_be_swapped() {
    let fixture = Fixture::new();
    let other = Fixture::new();
    let source = fixture.source();
    let send = fixture
        .files
        .request_send(SEND, "transfer_send_01")
        .unwrap();
    let selected = fixture.selected_send(&send, &source).unwrap();
    let receive = fixture
        .files
        .request_receive(RECEIVE, "transfer_receive_01", metadata())
        .unwrap();
    let receiving = fixture
        .selected_receive(&receive, &fixture.root.join("received.txt"))
        .unwrap();
    assert!(other.files.read(&send, &selected.handle, 0).is_err());
    assert!(fixture.files.read(&receive, &selected.handle, 0).is_err());
    assert!(
        fixture
            .files
            .write(&receive, &selected.handle, 0, BYTES)
            .is_err()
    );
    assert_eq!(
        fixture.files.read(&send, &selected.handle, 0).unwrap(),
        BYTES
    );
    fixture
        .files
        .write(&receive, &receiving.handle, 0, BYTES)
        .unwrap();
    fixture.files.commit(&receive, &receiving.handle).unwrap();
    assert_eq!(
        std::fs::read(fixture.root.join("received.txt")).unwrap(),
        BYTES
    );
    let altered = fixture
        .files
        .request_receive(RECEIVE, "transfer_altered_01", metadata())
        .unwrap();
    assert!(
        fixture
            .files
            .select_with(&altered, |store, op| {
                let actual = metadata();
                let token = store.open_receive(
                    &fixture.root.join("altered.txt"),
                    actual.clone(),
                    &mut || fixture.files.state.check(op),
                )?;
                let mut wrong = actual;
                wrong.name = "different.txt".into();
                Ok((token, wrong))
            })
            .is_err()
    );
    wait_until(|| {
        fixture
            .files
            .state
            .store(Direction::Receive)
            .try_lock()
            .is_ok_and(|store| store.transfers.is_empty())
    });
    assert!(!fixture.root.join("altered.txt").exists());
}
#[test]
fn cancellation_interrupts_a_busy_direction_without_blocking_the_other() {
    let fixture = Fixture::new();
    let files = fixture.files.clone();
    let source = fixture.source();
    let send = files.request_send(SEND, "transfer_busy_send").unwrap();
    let cancel = send.operation.cancel.clone();
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let result = files.select_with(&send, |store, op| {
            entered.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            store.open_send(&source, &mut || files.state.check(op))
        });
        assert!(result.is_err());
    });
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    fixture.gate.lock().unwrap().revoke(SEND);
    wait_until(|| cancel.load(Ordering::SeqCst));
    fixture.gate.lock().unwrap().suspend_input();
    let receive = fixture
        .files
        .request_receive(RECEIVE, "transfer_live_receive", metadata())
        .unwrap();
    let selection = fixture
        .selected_receive(&receive, &fixture.root.join("other-direction.txt"))
        .unwrap();
    fixture
        .files
        .write(&receive, &selection.handle, 0, BYTES)
        .unwrap();
    fixture.files.commit(&receive, &selection.handle).unwrap();
    release.send(()).unwrap();
    worker.join().unwrap();
    assert!(!fixture.gate.lock().unwrap().ended());
}
#[test]
fn revocation_cleans_staging_without_another_packet_and_preserves_send() {
    let fixture = Fixture::new();
    let source = fixture.source();
    let send = fixture
        .files
        .request_send(SEND, "transfer_send_live")
        .unwrap();
    let selected = fixture.selected_send(&send, &source).unwrap();
    let receive = fixture
        .files
        .request_receive(RECEIVE, "transfer_receive_revoke", metadata())
        .unwrap();
    let receiving = fixture
        .selected_receive(&receive, &fixture.root.join("revoked.txt"))
        .unwrap();
    fixture
        .files
        .write(&receive, &receiving.handle, 0, BYTES)
        .unwrap();
    fixture.gate.lock().unwrap().revoke(RECEIVE);
    wait_until(|| {
        fixture
            .files
            .state
            .store(Direction::Receive)
            .try_lock()
            .is_ok_and(|store| store.transfers.is_empty())
    });
    assert!(fixture.files.commit(&receive, &receiving.handle).is_err());
    assert!(!fixture.root.join("revoked.txt").exists());
    assert_eq!(
        fixture.files.read(&send, &selected.handle, 0).unwrap(),
        BYTES
    );
}
#[test]
fn cancelled_late_selection_and_old_request_drop_cannot_affect_a_successor() {
    let fixture = Fixture::new();
    let source = fixture.source();
    let files = fixture.files.clone();
    let old = files.request_send(SEND, "transfer_old_01").unwrap();
    let cancel = old.operation.cancel.clone();
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let result = files.select_with(&old, |store, _op| {
            entered.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            store.open_send(&source, &mut || Ok(()))
        });
        assert!(result.is_err());
        old
    });
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    cancel.store(true, Ordering::SeqCst);
    assert!(
        fixture
            .files
            .request_send(SEND, "transfer_too_early")
            .is_err()
    );
    release.send(()).unwrap();
    let old = worker.join().unwrap();
    wait_until(|| fixture.files.state.operations.lock().unwrap().is_empty());
    let next = fixture
        .files
        .request_send(SEND, "transfer_successor")
        .unwrap();
    let selected = fixture
        .selected_send(&next, &fixture.root.join("selected.txt"))
        .unwrap();
    drop(old);
    assert_eq!(
        fixture.files.read(&next, &selected.handle, 0).unwrap(),
        BYTES
    );
}
#[test]
fn grant_and_idle_expiration_and_session_end_cancel_without_new_work() {
    let fixture = Fixture::new();
    let source = fixture.source();
    let send = fixture.files.request_send(SEND, "transfer_expiry").unwrap();
    let selected = fixture.selected_send(&send, &source).unwrap();
    fixture.clock.fetch_add(5000, Ordering::SeqCst);
    wait_until(|| send.operation.cancel.load(Ordering::SeqCst));
    assert!(fixture.files.read(&send, &selected.handle, 0).is_err());
    let receive = fixture
        .files
        .request_receive(RECEIVE, "transfer_idle", metadata())
        .unwrap();
    let receiving = fixture
        .selected_receive(&receive, &fixture.root.join("idle.txt"))
        .unwrap();
    *receive.operation.until.lock().unwrap() = Instant::now();
    wait_until(|| fixture.empty());
    assert!(
        fixture
            .files
            .write(&receive, &receiving.handle, 0, BYTES)
            .is_err()
    );
    wait_until(|| fixture.files.state.operations.lock().unwrap().is_empty());
    let pending = fixture
        .files
        .request_receive(RECEIVE, "transfer_session_end", metadata())
        .unwrap();
    fixture.stop.store(true, Ordering::SeqCst);
    wait_until(|| pending.operation.cancel.load(Ordering::SeqCst));
    assert!(
        fixture
            .files
            .request_receive(RECEIVE, "transfer_after_end", metadata())
            .is_err()
    );
}
#[test]
fn quota_is_shared_across_directions_and_never_refunded_after_cancel() {
    let fixture = Fixture::new();
    let send = fixture
        .files
        .request_send(SEND, "transfer_quota_send")
        .unwrap();
    let selected = fixture.selected_send(&send, &fixture.source()).unwrap();
    let receive = fixture
        .files
        .request_receive(RECEIVE, "transfer_quota_receive", metadata())
        .unwrap();
    fixture
        .selected_receive(&receive, &fixture.root.join("quota.txt"))
        .unwrap();
    let quota = fixture
        .files
        .state
        .store(Direction::Send)
        .lock()
        .unwrap()
        .quota
        .clone();
    assert_eq!(quota.load(Ordering::SeqCst), 2 * BYTES.len() as u64);
    send.cancel();
    receive.cancel();
    wait_until(|| fixture.empty());
    assert_eq!(quota.load(Ordering::SeqCst), 2 * BYTES.len() as u64);
    assert!(fixture.files.read(&send, &selected.handle, 0).is_err());
    quota.store(super::super::SESSION_LIMIT - 1, Ordering::SeqCst);
    fixture
        .files
        .state
        .store(Direction::Send)
        .lock()
        .unwrap()
        .charge(1)
        .unwrap();
    assert!(
        fixture
            .files
            .state
            .store(Direction::Receive)
            .lock()
            .unwrap()
            .charge(1)
            .is_err()
    );
}
#[test]
fn closing_a_session_does_not_wait_for_its_busy_native_operation() {
    let fixture = Fixture::new();
    let files = fixture.files.clone();
    let send = files.request_send(SEND, "transfer_close_busy").unwrap();
    let cancel = send.operation.cancel.clone();
    let source = fixture.source();
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        assert!(
            files
                .select_with(&send, |store, op| {
                    entered.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
                    store.open_send(&source, &mut || files.state.check(op))
                })
                .is_err()
        );
    });
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let started = Instant::now();
    fixture.files.close();
    assert!(started.elapsed() < Duration::from_millis(250));
    assert!(cancel.load(Ordering::SeqCst));
    release.send(()).unwrap();
    worker.join().unwrap();
}
#[test]
fn urgent_resource_revocation_does_not_wait_for_registry_or_authority_locks() {
    let fixture = Fixture::new();
    let gate = fixture.gate.lock().unwrap();
    let operations = fixture.files.state.operations.lock().unwrap();
    let revoker = fixture.files.revoker();
    let started = Instant::now();
    revoker.revoke();
    fixture.files.close();
    assert!(started.elapsed() < Duration::from_millis(250));
    assert!(fixture.files.state.closed.load(Ordering::SeqCst));
    drop(operations);
    drop(gate);
    fixture.files.wait_cleanup().unwrap();
}
#[test]
fn cleanup_waits_for_a_selecting_operation_and_both_owned_stores() {
    let fixture = Fixture::new();
    let files = fixture.files.clone();
    let send = files
        .request_send(SEND, "transfer_retained_select")
        .unwrap();
    let (entered, entering) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let selection = thread::spawn(move || {
        assert!(
            files
                .select_with(&send, |_store, _op| {
                    entered.send(()).unwrap();
                    released.recv_timeout(Duration::from_secs(3)).unwrap();
                    Err("fixture-cancelled")
                })
                .is_err()
        );
    });
    entering.recv_timeout(Duration::from_secs(3)).unwrap();
    let files = fixture.files.clone();
    let (done, completed) = mpsc::channel();
    let waiting = thread::spawn(move || {
        done.send(files.wait_cleanup()).unwrap();
    });
    assert!(completed.recv_timeout(Duration::from_millis(50)).is_err());
    release.send(()).unwrap();
    selection.join().unwrap();
    assert_eq!(
        completed.recv_timeout(Duration::from_secs(3)).unwrap(),
        Ok(())
    );
    waiting.join().unwrap();
    fixture.files.wait_cleanup().unwrap();
    assert!(fixture.empty());
}
#[test]
fn poisoned_registry_is_recovered_only_for_release_and_never_reports_success() {
    let fixture = Fixture::new();
    let receive = fixture
        .files
        .request_receive(RECEIVE, "transfer_poisoned_cleanup", metadata())
        .unwrap();
    fixture
        .selected_receive(&receive, &fixture.root.join("poison.txt"))
        .unwrap();
    let files = fixture.files.clone();
    assert!(
        thread::spawn(move || {
            let _held = files.state.operations.lock().unwrap();
            panic!("intentional registry poison");
        })
        .join()
        .is_err()
    );
    assert!(fixture.files.wait_cleanup().is_err());
    assert!(fixture.files.wait_cleanup().is_err());
    assert!(fixture.empty());
    assert!(!fixture.root.join("poison.txt").exists());
}
#[test]
fn an_empty_operation_registry_cannot_hide_either_busy_store() {
    let fixture = Fixture::new();
    let sending = fixture.files.state.store(Direction::Send).lock().unwrap();
    let receiving = fixture
        .files
        .state
        .store(Direction::Receive)
        .lock()
        .unwrap();
    let files = fixture.files.clone();
    let (done, complete) = mpsc::channel();
    let waiter = thread::spawn(move || {
        done.send(files.wait_cleanup()).unwrap();
    });
    assert!(complete.recv_timeout(Duration::from_millis(50)).is_err());
    drop(sending);
    assert!(complete.recv_timeout(Duration::from_millis(50)).is_err());
    drop(receiving);
    assert_eq!(
        complete.recv_timeout(Duration::from_secs(3)).unwrap(),
        Ok(())
    );
    waiter.join().unwrap();
    fixture.files.wait_cleanup().unwrap();
}
#[test]
fn expired_covering_lease_cancels_all_file_work_without_waiting_for_packets() {
    let fixture = Fixture::new();
    let receive = fixture
        .files
        .request_receive(RECEIVE, "transfer_expired_lease", metadata())
        .unwrap();
    let selected = fixture
        .selected_receive(&receive, &fixture.root.join("lease-expired.txt"))
        .unwrap();
    fixture.clock.fetch_add(20000, Ordering::SeqCst);
    wait_until(|| receive.operation.cancel.load(Ordering::SeqCst) && fixture.empty());
    assert!(
        fixture
            .files
            .write(&receive, &selected.handle, 0, BYTES)
            .is_err()
    );
    assert!(!fixture.root.join("lease-expired.txt").exists());
}
#[test]
fn opposite_direction_quota_race_cannot_overcommit_the_session() {
    let fixture = Fixture::new();
    let quota = fixture
        .files
        .state
        .store(Direction::Send)
        .lock()
        .unwrap()
        .quota
        .clone();
    quota.store(super::super::SESSION_LIMIT - 1, Ordering::SeqCst);
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let mut workers = Vec::new();
    for direction in [Direction::Send, Direction::Receive] {
        let files = fixture.files.clone();
        let start = barrier.clone();
        workers.push(thread::spawn(move || {
            start.wait();
            files
                .state
                .store(direction)
                .lock()
                .unwrap()
                .charge(1)
                .is_ok()
        }));
    }
    barrier.wait();
    let accepted = workers
        .into_iter()
        .map(|worker| usize::from(worker.join().unwrap()))
        .sum::<usize>();
    assert_eq!(accepted, 1);
    assert_eq!(quota.load(Ordering::SeqCst), super::super::SESSION_LIMIT);
}
