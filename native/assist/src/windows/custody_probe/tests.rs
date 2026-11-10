use super::*;
const EXECUTOR: &str = "windows::custody_probe::tests::executor_fixture";
const COORDINATOR: &str = "windows::custody_probe::tests::coordinator_fixture";
#[test]
fn executor_fixture() {
    if !std::env::args().any(|arg| arg == EXECUTOR) {
        return;
    }
    let outcome = (|| {
        let spec: Bootstrap = decode(
            &read_bounded(&mut std::io::stdin().lock(), MAX_FRAME)
                .map_err(|_| "fixture-bootstrap")?
                .ok_or("fixture-bootstrap")?,
        )?;
        let parent = Parent::validate_executor(spec.monitor, true)?;
        run_child(parent, spec)
    })();
    // Never block the framework's result printing behind the intentional stdout
    // pressure fixture. This mode has NO injectors or native resource authority.
    std::process::exit(if outcome.is_ok() { 0 } else { 125 });
}
#[test]
fn coordinator_fixture() {
    if !std::env::args().any(|arg| arg == COORDINATOR) {
        return;
    }
    let outcome = (|| {
        let spec: CoordinatorBootstrap = decode(
            &read_bounded(&mut std::io::stdin().lock(), MAX_FRAME)
                .map_err(|_| "fixture-bootstrap")?
                .ok_or("fixture-bootstrap")?,
        )?;
        let parent = Parent::validate_executor(spec.monitor, true)?;
        run_coordinator(parent, spec)
    })();
    std::process::exit(if outcome.is_ok() { 0 } else { 125 });
}
struct Fixture {
    global: Arc<StopEvent>,
    tree: ProcessTree,
    coordinator: Child,
    registry: Registry,
    processes: ProbeProcesses,
    ticket: Ticket,
    index: usize,
}
impl Fixture {
    fn start(fault: Fault, post_create_failure: bool) -> (Self, Result<usize, &'static str>) {
        Self::start_with(fault, post_create_failure, StartupFault::None)
    }
    fn start_with(
        fault: Fault,
        post_create_failure: bool,
        startup: StartupFault,
    ) -> (Self, Result<usize, &'static str>) {
        let global = Arc::new(StopEvent::new().unwrap());
        let global_read = global.reader().unwrap();
        let mut tree = ProcessTree::new().unwrap();
        let coordinator = tree
            .launch_custody_fixture(false, &[global_read.as_handle()])
            .unwrap();
        write_bounded(
            &mut &coordinator.input,
            &encode(&CoordinatorBootstrap {
                monitor: std::process::id(),
                global: global_read.as_raw_handle() as usize,
            })
            .unwrap(),
            MAX_FRAME,
        )
        .unwrap();
        let coordinator = Child::retained(coordinator);
        let mut registry = Registry::new([1; 32], [2; 32]);
        let ticket = registry
            .reserve("custody_probe_session_01", 1, 1, [3; 32], Mode::Probe)
            .unwrap();
        let mut processes = ProbeProcesses::default();
        let spawned = processes.spawn_with(
            &mut registry,
            ticket.clone(),
            [2; 32],
            ProbeContext {
                coordinator: coordinator.process.as_handle(),
                global: global_read.as_handle(),
                fault,
                startup,
            },
            |extra| {
                let child = tree.launch_custody_fixture(true, extra)?;
                if post_create_failure {
                    Err(process_tree::LaunchFailure {
                        error: "fixture-post-create",
                        child: Some(child),
                    })
                } else {
                    Ok(child)
                }
            },
        );
        (
            Self {
                global,
                tree,
                coordinator,
                registry,
                processes,
                ticket,
                index: 0,
            },
            spawned,
        )
    }
    fn new(fault: Fault) -> Self {
        let (fixture, spawned) = Self::start(fault, false);
        assert_eq!(spawned.unwrap(), fixture.index);
        fixture
    }
    fn arm(&mut self) {
        let process = self.processes.get_mut(self.index).unwrap();
        process.arm(&mut self.registry).unwrap();
        process.ready().unwrap();
    }
    fn settle_child(&mut self) {
        self.processes
            .get_mut(self.index)
            .unwrap()
            .stop(&mut self.registry)
            .unwrap();
    }
    fn receipt(&mut self) -> Result<bool, &'static str> {
        self.processes
            .get_mut(self.index)
            .unwrap()
            .receipt(&mut self.registry)
    }
    fn wait_child(&mut self) {
        let until = Instant::now() + Duration::from_secs(3);
        while !self
            .processes
            .get_mut(self.index)
            .unwrap()
            .observe_exit(&mut self.registry)
            .unwrap()
        {
            assert!(Instant::now() < until, "custody child did not exit");
            thread::sleep(Duration::from_millis(5));
        }
    }
    fn end_coordinator(&mut self) {
        drop(self.coordinator.input.take());
        let until = Instant::now() + Duration::from_secs(3);
        while !self.coordinator.exited().unwrap() {
            assert!(Instant::now() < until, "custody coordinator did not exit");
            thread::sleep(Duration::from_millis(5));
        }
        self.registry.coordinator_exited();
    }
    fn assert_job_empty(&mut self) {
        self.tree.close_admission();
        let until = Instant::now() + Duration::from_secs(3);
        while !self.tree.exited_after_admission_closed().unwrap() {
            assert!(Instant::now() < until, "custody descendants still active");
            thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.global.revoke();
        self.processes.stop_all(&mut self.registry);
        drop(self.coordinator.input.take());
    }
}
#[test]
fn native_pipes_do_not_complete_a_live_coordinators_mirror() {
    let mut f = Fixture::new(Fault::None);
    f.arm();
    f.settle_child();
    assert!(f.receipt().unwrap());
    f.wait_child();
    assert!(!f.coordinator.exited().unwrap());
    assert!(!f.registry.input_settled(&f.ticket).unwrap());
    f.processes
        .get_mut(f.index)
        .unwrap()
        .mirror_probe_settled(&mut f.registry)
        .unwrap();
    assert!(
        f.processes
            .get_mut(f.index)
            .unwrap()
            .completion_is_probe(&f.registry)
            .unwrap()
    );
    f.end_coordinator();
    f.assert_job_empty();
}
#[test]
fn actual_coordinator_death_recovers_only_with_a_positive_child_receipt() {
    let mut f = Fixture::new(Fault::None);
    f.arm();
    f.end_coordinator();
    assert!(f.receipt().unwrap());
    f.wait_child();
    assert!(f.registry.known_entries_complete());
    f.assert_job_empty();
}
#[test]
fn malformed_missing_and_replayed_native_pipe_receipts_remain_unproven() {
    for fault in [
        Fault::Missing,
        Fault::Truncated,
        Fault::WrongTicket,
        Fault::WrongMode,
        Fault::Duplicate,
    ] {
        let mut f = Fixture::new(fault);
        f.arm();
        f.settle_child();
        assert!(f.receipt().is_err());
        f.wait_child();
        f.end_coordinator();
        f.assert_job_empty();
        assert!(!f.registry.known_entries_complete(), "{fault:?}");
    }
}
#[test]
fn lost_and_partial_arm_keep_pending_until_positive_cleanup_and_exit() {
    for partial in [false, true] {
        let mut f = Fixture::new(Fault::None);
        f.processes
            .get_mut(f.index)
            .unwrap()
            .lose_arm(&mut f.registry, partial)
            .unwrap();
        assert!(!f.receipt().unwrap());
        f.wait_child();
        assert!(!f.registry.known_entries_complete());
        f.processes
            .get_mut(f.index)
            .unwrap()
            .mirror_probe_settled(&mut f.registry)
            .unwrap();
        assert!(f.registry.known_entries_complete());
        f.end_coordinator();
        f.assert_job_empty();
    }
}
#[test]
fn positive_receipt_never_substitutes_for_actual_child_exit() {
    let mut f = Fixture::new(Fault::HoldAfterReceipt);
    f.arm();
    f.settle_child();
    assert!(f.receipt().unwrap());
    assert!(
        !f.processes
            .get_mut(f.index)
            .unwrap()
            .observe_exit(&mut f.registry)
            .unwrap()
    );
    assert!(!f.registry.known_entries_complete());
    f.processes.get_mut(f.index).unwrap().allow_exit().unwrap();
    f.wait_child();
    f.end_coordinator();
    assert!(f.registry.known_entries_complete());
    f.assert_job_empty();
}
#[test]
fn blocked_ordinary_stdout_cannot_block_dedicated_custody_cleanup() {
    let mut f = Fixture::new(Fault::BlockedOutput);
    f.arm();
    f.processes
        .get_mut(f.index)
        .unwrap()
        .wait_output_pressure()
        .unwrap();
    f.settle_child();
    assert!(f.receipt().unwrap());
    f.wait_child();
    assert!(
        f.processes
            .get_mut(f.index)
            .unwrap()
            .wait_output_pressure()
            .is_err(),
        "a full pipe retained after child exit is not live output pressure"
    );
    f.end_coordinator();
    assert!(f.registry.known_entries_complete());
    f.assert_job_empty();
}
#[test]
fn never_armed_cancel_and_post_create_errors_retain_exact_processes_until_exit() {
    let mut f = Fixture::new(Fault::None);
    f.settle_child();
    assert!(!f.receipt().unwrap());
    f.wait_child();
    f.end_coordinator();
    assert!(f.registry.known_entries_complete());
    f.assert_job_empty();
    let (mut f, spawned) = Fixture::start(Fault::None, true);
    assert_eq!(spawned, Err("fixture-post-create"));
    assert_eq!(f.processes.entries.len(), 1);
    assert!(!f.registry.known_entries_complete());
    f.wait_child();
    f.end_coordinator();
    assert!(f.registry.known_entries_complete());
    f.assert_job_empty();
}
#[test]
fn partial_bootstrap_and_query_errors_close_pipes_without_dropping_the_process_owner() {
    for (fault, expected) in [
        (StartupFault::PartialBootstrap, "fixture-bootstrap-write"),
        (StartupFault::IdentityQuery, "fixture-identity-query"),
    ] {
        let (mut f, spawned) = Fixture::start_with(Fault::None, false, fault);
        assert_eq!(spawned, Err(expected));
        assert_eq!(f.processes.entries.len(), 1);
        let process = &f.processes.entries[0];
        assert!(process.child.input.is_none());
        assert!(process.arm.is_none());
        assert!(!f.registry.known_entries_complete());
        f.wait_child();
        f.end_coordinator();
        assert!(f.registry.known_entries_complete());
        f.assert_job_empty();
    }
}
#[test]
fn invalid_ready_sequences_cannot_be_repaired_by_a_later_terminal_frame() {
    for fault in [
        Fault::WrongArmedTicket,
        Fault::WrongArmedMode,
        Fault::MissingArmed,
    ] {
        let mut f = Fixture::new(fault);
        let process = f.processes.get_mut(f.index).unwrap();
        process.arm(&mut f.registry).unwrap();
        assert!(process.ready().is_err());
        process.stop(&mut f.registry).unwrap();
        assert!(process.receipt(&mut f.registry).is_err());
        f.wait_child();
        assert!(!f.registry.known_entries_complete());
        // An independent mirror can still prove its own settled state, but
        // the invalid direct-child protocol never becomes a valid receipt.
        f.processes
            .get_mut(f.index)
            .unwrap()
            .mirror_probe_settled(&mut f.registry)
            .unwrap();
        assert!(f.registry.known_entries_complete());
        f.end_coordinator();
        f.assert_job_empty();
    }
}
