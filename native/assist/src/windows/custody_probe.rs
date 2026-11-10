//! Real process/pipe custody qualification with NO input, hooks, identity,
//! clipboard or resources. Probe receipts cannot satisfy native cleanup scope.
use super::{
    image::VerifiedImage,
    parent::Parent,
    process_tree::{self, ProcessTree, StopEvent, TreeChild},
};
use crate::{
    custody::{Mode, ProcessIdentity, Registry, Ticket},
    framing::{read_bounded, write_bounded},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::Write,
    os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle},
    ptr,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Storage::FileSystem::{FILE_TYPE_PIPE, GetFileType},
    System::{
        Pipes::{GetNamedPipeInfo, PeekNamedPipe},
        Threading::*,
    },
};

const MAX_FRAME: usize = 4096;
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fault {
    None,
    Missing,
    Truncated,
    WrongTicket,
    WrongMode,
    WrongArmedTicket,
    WrongArmedMode,
    MissingArmed,
    Duplicate,
    HoldAfterReceipt,
    BlockedOutput,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Bootstrap {
    monitor: u32,
    ticket: Ticket,
    arm: usize,
    receipt: usize,
    coordinator: usize,
    coordinator_pid: u32,
    coordinator_created: u64,
    stop: usize,
    global: usize,
    exit: usize,
    fault: Fault,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoordinatorBootstrap {
    monitor: u32,
    global: usize,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Arm {
    ticket: Ticket,
    mode: Mode,
}
#[derive(Deserialize, Serialize)]
#[serde(tag = "type", deny_unknown_fields, rename_all = "kebab-case")]
enum ChildFrame {
    Armed {
        ticket: Ticket,
        mode: Mode,
    },
    Released {
        ticket: Ticket,
        mode: Mode,
        arm_accepted: bool,
    },
}

fn duplicate(raw: HANDLE, rights: u32, inherit: bool) -> Result<OwnedHandle, &'static str> {
    let mut handle = ptr::null_mut();
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            raw,
            GetCurrentProcess(),
            &mut handle,
            rights,
            inherit as i32,
            0,
        )
    } == 0
    {
        return Err("custody-handle");
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}
fn noninherit(handle: &OwnedHandle) -> Result<(), &'static str> {
    if unsafe { SetHandleInformation(handle.as_raw_handle(), HANDLE_FLAG_INHERIT, 0) } == 0 {
        return Err("custody-handle");
    }
    Ok(())
}
fn wait_stop(handle: &OwnedHandle) -> bool {
    unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) != WAIT_TIMEOUT }
}
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, &'static str> {
    serde_json::to_vec(value).map_err(|_| "custody-message")
}
fn decode<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, &'static str> {
    serde_json::from_slice(bytes).map_err(|_| "custody-message")
}
unsafe fn adopt(raw: usize, pipe: bool) -> Result<OwnedHandle, &'static str> {
    if raw == 0 || raw == usize::MAX {
        return Err("custody-handle");
    }
    let handle = raw as HANDLE;
    let mut flags = 0;
    if unsafe { GetHandleInformation(handle, &mut flags) } == 0
        || (pipe && unsafe { GetFileType(handle) } != FILE_TYPE_PIPE)
    {
        return Err("custody-handle");
    }
    let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
    noninherit(&handle)?;
    Ok(handle)
}

/// Owns a probe process and both custody directions. Normal stdout/stdin remain
/// separate, so blocked ordinary I/O cannot serve as a cleanup receipt.
struct Child {
    process: OwnedHandle,
    input: Option<File>,
    _output: File,
    _error: File,
}
impl Child {
    fn retained(child: TreeChild) -> Self {
        let (process, input, output, error) = child.into_parts();
        Self {
            process,
            input: Some(input),
            _output: output,
            _error: error,
        }
    }
    fn exited(&self) -> Result<bool, &'static str> {
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err("custody-process-query"),
        }
    }
}
pub struct ProbeProcess {
    child: Child,
    ticket: Ticket,
    identity: Option<ProcessIdentity>,
    image: [u8; 32],
    arm: Option<File>,
    receipts: mpsc::Receiver<Result<ChildFrame, &'static str>>,
    stop: Arc<StopEvent>,
    exit: StopEvent,
}
pub struct ProbeContext<'a> {
    pub coordinator: BorrowedHandle<'a>,
    pub global: BorrowedHandle<'a>,
    pub fault: Fault,
    #[cfg(test)]
    startup: StartupFault,
}
#[cfg(test)]
#[derive(Clone, Copy)]
enum StartupFault {
    None,
    PartialBootstrap,
    IdentityQuery,
}
/// The collection retains every created process BEFORE bootstrap/query failure
/// can escape. A failed spawn never drops its exact child owner behind an Err.
#[derive(Default)]
pub struct ProbeProcesses {
    entries: Vec<ProbeProcess>,
}
impl ProbeProcesses {
    pub fn get_mut(&mut self, index: usize) -> Result<&mut ProbeProcess, &'static str> {
        self.entries.get_mut(index).ok_or("custody-process")
    }
    pub fn stop_all(&mut self, registry: &mut Registry) {
        for process in &mut self.entries {
            let _ = process.stop(registry);
        }
    }
    pub fn spawn(
        &mut self,
        tree: &mut ProcessTree,
        image: &VerifiedImage,
        registry: &mut Registry,
        ticket: Ticket,
        context: ProbeContext<'_>,
    ) -> Result<usize, &'static str> {
        self.spawn_with(registry, ticket, image.digest(), context, |extra| {
            tree.launch_custody(
                image,
                &[
                    "--custody-executor-probe".into(),
                    std::process::id().to_string().into(),
                ],
                extra,
            )
        })
    }
    fn spawn_with(
        &mut self,
        registry: &mut Registry,
        ticket: Ticket,
        image: [u8; 32],
        context: ProbeContext<'_>,
        launch: impl FnOnce(&[BorrowedHandle<'_>]) -> Result<TreeChild, process_tree::LaunchFailure>,
    ) -> Result<usize, &'static str> {
        if registry.mode(&ticket)? != Mode::Probe {
            return Err("custody-probe-scope");
        }
        let (receipt_read, receipt_write) = process_tree::custody_pipe()?;
        let (arm_read, arm_write) = process_tree::custody_pipe()?;
        noninherit(&receipt_read)?;
        noninherit(&arm_write)?;
        let coordinator = duplicate(
            context.coordinator.as_raw_handle(),
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            true,
        )?;
        let stop = Arc::new(StopEvent::new()?);
        let stop_read = stop.reader()?;
        let exit = StopEvent::new()?;
        let exit_read = exit.reader()?;
        let spec = Bootstrap {
            monitor: std::process::id(),
            ticket: ticket.clone(),
            arm: arm_read.as_raw_handle() as usize,
            receipt: receipt_write.as_raw_handle() as usize,
            coordinator: coordinator.as_raw_handle() as usize,
            coordinator_pid: unsafe { GetProcessId(coordinator.as_raw_handle()) },
            coordinator_created: unsafe {
                super::parent::creation_time(coordinator.as_raw_handle())?
            },
            stop: stop_read.as_raw_handle() as usize,
            global: context.global.as_raw_handle() as usize,
            exit: exit_read.as_raw_handle() as usize,
            fault: context.fault,
        };
        registry.begin_spawn(&ticket)?;
        let launched = launch(&[
            arm_read.as_handle(),
            receipt_write.as_handle(),
            coordinator.as_handle(),
            stop_read.as_handle(),
            context.global,
            exit_read.as_handle(),
        ]);
        let (child, startup_error) = match launched {
            Ok(child) => (child, None),
            Err(failure) => match failure.child {
                Some(child) => (child, Some(failure.error)),
                None => {
                    registry.never_started(&ticket)?;
                    return Err(failure.error);
                }
            },
        };
        let (send, receipts) = mpsc::sync_channel(2);
        let index = self.entries.len();
        self.entries.push(ProbeProcess {
            child: Child::retained(child),
            ticket: ticket.clone(),
            identity: None,
            image,
            arm: Some(File::from(arm_write)),
            receipts,
            stop,
            exit,
        });
        // Close redundant writers BEFORE reading a receipt; only the child owns
        // its dedicated writer, never the coordinator or another descendant.
        drop(arm_read);
        drop(receipt_write);
        drop(coordinator);
        drop(stop_read);
        drop(exit_read);
        let expected_ticket = ticket.clone();
        thread::spawn(move || {
            let mut pipe = File::from(receipt_read);
            let result = (|| {
                let first = read_bounded(&mut pipe, MAX_FRAME)
                    .map_err(|_| "custody-receipt")?
                    .ok_or("custody-receipt")?;
                let first: ChildFrame = decode(&first)?;
                let armed_seen = matches!(first, ChildFrame::Armed { .. });
                let terminal = if let ChildFrame::Armed { ticket, mode } = &first {
                    if *ticket != expected_ticket || *mode != Mode::Probe {
                        return Err("custody-ready-binding");
                    }
                    send.send(Ok(first)).map_err(|_| "custody-receipt")?;
                    decode::<ChildFrame>(
                        &read_bounded(&mut pipe, MAX_FRAME)
                            .map_err(|_| "custody-receipt")?
                            .ok_or("custody-receipt")?,
                    )?
                } else {
                    first
                };
                match &terminal {
                    ChildFrame::Released {
                        ticket,
                        mode,
                        arm_accepted,
                    } if *ticket == expected_ticket
                        && *mode == Mode::Probe
                        && *arm_accepted == armed_seen => {}
                    _ => return Err("custody-receipt-binding"),
                }
                if read_bounded(&mut pipe, MAX_FRAME)
                    .map_err(|_| "custody-receipt")?
                    .is_some()
                {
                    return Err("custody-receipt-extra");
                }
                Ok(terminal)
            })();
            let _ = send.send(result);
        });
        let process = &mut self.entries[index];
        if let Some(error) = startup_error {
            let _ = process.stop(registry);
            return Err(error);
        }
        // The owner is already retained even if writing a partial bootstrap or
        // querying identity fails. stop_all closes stdin as well as ARM.
        let outcome = (|| {
            #[cfg(test)]
            if matches!(context.startup, StartupFault::PartialBootstrap) {
                process
                    .child
                    .input
                    .as_mut()
                    .ok_or("custody-bootstrap")?
                    .write_all(&[100, 0, 0, 0, b'{'])
                    .map_err(|_| "custody-bootstrap")?;
                return Err("fixture-bootstrap-write");
            }
            write_bounded(
                process.child.input.as_mut().ok_or("custody-bootstrap")?,
                &encode(&spec)?,
                MAX_FRAME,
            )
            .map_err(|_| "custody-bootstrap")?;
            #[cfg(test)]
            if matches!(context.startup, StartupFault::IdentityQuery) {
                return Err("fixture-identity-query");
            }
            process.bind(registry)
        })();
        if let Err(error) = outcome {
            // Never leave a partial-bootstrap reader waiting on retained stdin.
            // The exact process owner remains in the collection on every error.
            let _ = process.stop(registry);
            return Err(error);
        }
        Ok(index)
    }
}
impl ProbeProcess {
    pub fn wait_output_pressure(&self) -> Result<(), &'static str> {
        let raw = self.child._output.as_raw_handle();
        let (mut outgoing, mut incoming) = (0, 0);
        if unsafe {
            GetNamedPipeInfo(
                raw,
                ptr::null_mut(),
                &mut outgoing,
                &mut incoming,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err("custody-output-query");
        }
        let capacity = outgoing.max(incoming);
        if capacity == 0 {
            return Err("custody-output-query");
        }
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            let mut available = 0;
            if unsafe {
                PeekNamedPipe(
                    raw,
                    ptr::null_mut(),
                    0,
                    ptr::null_mut(),
                    &mut available,
                    ptr::null_mut(),
                )
            } == 0
            {
                return Err("custody-output-query");
            }
            // Actual kernel occupancy, not a delay or the writer's claim.
            // Nobody drains this ordinary pipe during the custody test.
            if self.child.exited()? {
                return Err("custody-output-child-ended");
            }
            if available >= capacity {
                return Ok(());
            }
            if Instant::now() >= until {
                return Err("custody-output-not-full");
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
    fn bind(&mut self, registry: &mut Registry) -> Result<(), &'static str> {
        if self.identity.is_none() {
            let identity = ProcessIdentity {
                pid: unsafe { GetProcessId(self.child.process.as_raw_handle()) },
                created: unsafe {
                    super::parent::creation_time(self.child.process.as_raw_handle())?
                },
                image: self.image,
            };
            registry.bind(&self.ticket, identity)?;
            self.identity = Some(identity);
        }
        Ok(())
    }
    pub fn arm(&mut self, registry: &mut Registry) -> Result<(), &'static str> {
        registry.arm(&self.ticket)?; // BEFORE any possible delivery.
        write_bounded(
            self.arm.as_mut().ok_or("custody-arm")?,
            &encode(&Arm {
                ticket: self.ticket.clone(),
                mode: Mode::Probe,
            })?,
            MAX_FRAME,
        )
        .map_err(|_| "custody-arm")
    }
    pub fn lose_arm(&mut self, registry: &mut Registry, partial: bool) -> Result<(), &'static str> {
        registry.arm(&self.ticket)?;
        if partial {
            self.arm
                .as_mut()
                .ok_or("custody-arm")?
                .write_all(&[100, 0, 0, 0, b'{'])
                .map_err(|_| "custody-arm")?;
        }
        drop(self.arm.take());
        Ok(())
    }
    pub fn stop(&mut self, registry: &mut Registry) -> Result<(), &'static str> {
        self.stop.revoke()?; // Urgent native signal precedes state/pipe work.
        registry.stop(&self.ticket)?;
        drop(self.arm.take());
        drop(self.child.input.take());
        Ok(())
    }
    pub fn ready(&self) -> Result<(), &'static str> {
        match self
            .receipts
            .recv_timeout(Duration::from_secs(3))
            .map_err(|_| "custody-receipt")??
        {
            ChildFrame::Armed { ticket, mode } if ticket == self.ticket && mode == Mode::Probe => {
                Ok(())
            }
            _ => Err("custody-ready"),
        }
    }
    pub fn receipt(&self, registry: &mut Registry) -> Result<bool, &'static str> {
        loop {
            match self
                .receipts
                .recv_timeout(Duration::from_secs(3))
                .map_err(|_| "custody-receipt")??
            {
                ChildFrame::Armed { ticket, mode }
                    if ticket == self.ticket && mode == Mode::Probe =>
                {
                    continue;
                }
                ChildFrame::Released {
                    ticket,
                    mode,
                    arm_accepted,
                } if ticket == self.ticket => {
                    registry.child_released(&ticket, mode)?;
                    return Ok(arm_accepted);
                }
                _ => return Err("custody-binding"),
            }
        }
    }
    pub fn observe_exit(&mut self, registry: &mut Registry) -> Result<bool, &'static str> {
        if !self.child.exited()? {
            return Ok(false);
        }
        self.bind(registry)?;
        registry.child_exited(&self.ticket, self.identity.ok_or("custody-process")?)?;
        Ok(true)
    }
    pub fn allow_exit(&self) -> Result<(), &'static str> {
        self.exit.revoke()
    }
    pub fn completion_is_probe(&self, registry: &Registry) -> Result<bool, &'static str> {
        Ok(registry.completion(&self.ticket)?.is_some_and(|proof| {
            proof.matches(&self.ticket, Mode::Probe) && !proof.matches(&self.ticket, Mode::Native)
        }))
    }
    pub fn mirror_probe_settled(&mut self, registry: &mut Registry) -> Result<(), &'static str> {
        self.observe_exit(registry)?;
        registry.mirror_released(&self.ticket, Mode::Probe)
    }
}
impl Drop for ProbeProcess {
    fn drop(&mut self) {
        let _ = self.stop.revoke();
        let _ = self.exit.revoke();
    }
}

/// Fixed non-injecting child. No Native Backend, input hooks, consent or identity
/// constructor is reachable here, including when sensitive features compile.
pub fn run_probe(parent_pid: u32) -> Result<(), &'static str> {
    let parent = Parent::validate_executor(parent_pid, true)?;
    let spec: Bootstrap = decode(
        &read_bounded(&mut std::io::stdin().lock(), MAX_FRAME)
            .map_err(|_| "custody-bootstrap")?
            .ok_or("custody-bootstrap")?,
    )?;
    run_child(parent, spec)
}
pub fn run_coordinator_probe(parent_pid: u32) -> Result<(), &'static str> {
    let parent = Parent::validate_executor(parent_pid, true)?;
    let spec: CoordinatorBootstrap = decode(
        &read_bounded(&mut std::io::stdin().lock(), MAX_FRAME)
            .map_err(|_| "custody-bootstrap")?
            .ok_or("custody-bootstrap")?,
    )?;
    run_coordinator(parent, spec)
}
fn run_coordinator(parent: Parent, spec: CoordinatorBootstrap) -> Result<(), &'static str> {
    use std::io::Read;
    if spec.monitor != parent.pid() {
        return Err("custody-parent");
    }
    let global = unsafe { adopt(spec.global, false)? };
    let (send, receive) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = std::io::stdin().read(&mut [0u8; 1]);
        let _ = send.send(());
    });
    while parent.alive() && !wait_stop(&global) && receive.try_recv().is_err() {
        thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}
struct StopOnDrop(Arc<StopEvent>);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        let _ = self.0.revoke();
    }
}
fn random_nonce() -> Result<[u8; 32], &'static str> {
    let mut value = [0u8; 32];
    if unsafe {
        windows_sys::Win32::Security::Cryptography::BCryptGenRandom(
            ptr::null_mut(),
            value.as_mut_ptr(),
            value.len() as u32,
            windows_sys::Win32::Security::Cryptography::BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    } < 0
    {
        return Err("custody-random");
    }
    Ok(value)
}
fn emit_report(value: &serde_json::Value) -> Result<(), &'static str> {
    write_bounded(&mut std::io::stdout().lock(), &encode(value)?, MAX_FRAME)
        .map_err(|_| "custody-output")
}
/// Fixed qualification actor. It never starts a native endpoint or produces a
/// Native receipt, even when native-input/native-resources features compile.
pub fn run_monitor_probe(
    parent_pid: u32,
    manifest: super::image::ImageManifest,
    scenario: &str,
) -> Result<(), &'static str> {
    use std::sync::atomic::{AtomicBool, Ordering};
    let fault = match scenario {
        "mirror" | "child-recovery" | "lost-arm" | "partial-arm" | "cancel-before-arm" => {
            Fault::None
        }
        "missing" => Fault::Missing,
        "truncated" => Fault::Truncated,
        "wrong-ticket" => Fault::WrongTicket,
        "wrong-mode" => Fault::WrongMode,
        "duplicate" => Fault::Duplicate,
        "hold-after-receipt" => Fault::HoldAfterReceipt,
        "blocked-output" => Fault::BlockedOutput,
        _ => return Err("custody-scenario"),
    };
    let parent = Parent::validate(parent_pid, true)?;
    let image = VerifiedImage::open(&manifest)?;
    let mut tree = ProcessTree::new()?;
    let global = Arc::new(StopEvent::new()?);
    let _stop_on_all_paths = StopOnDrop(global.clone());
    let global_read = global.reader()?;
    let coordinator = tree.launch(
        &image,
        &[
            "--custody-coordinator-probe".into(),
            std::process::id().to_string().into(),
        ],
        &[global_read.as_handle()],
    )?;
    write_bounded(
        &mut &coordinator.input,
        &encode(&CoordinatorBootstrap {
            monitor: std::process::id(),
            global: global_read.as_raw_handle() as usize,
        })?,
        MAX_FRAME,
    )
    .map_err(|_| "custody-bootstrap")?;
    let mut coordinator = Child::retained(coordinator);
    let mut registry = Registry::new(random_nonce()?, image.digest());
    let ticket = registry.reserve(
        "custody_probe_session_01",
        1,
        1,
        random_nonce()?,
        Mode::Probe,
    )?;
    let mut processes = ProbeProcesses::default();
    let index = processes.spawn(
        &mut tree,
        &image,
        &mut registry,
        ticket.clone(),
        ProbeContext {
            coordinator: coordinator.process.as_handle(),
            global: global_read.as_handle(),
            fault,
            #[cfg(test)]
            startup: StartupFault::None,
        },
    )?;
    let process = processes.get_mut(index)?;
    emit_report(
        &serde_json::json!({"protocol":"palisade.assist.custody-probe","version":1,"stage":"spawned",
        "mode":"probe","authority":"none","monitor":std::process::id(),
        "coordinator":unsafe{GetProcessId(coordinator.process.as_raw_handle())},
        "child":unsafe{GetProcessId(process.child.process.as_raw_handle())}}),
    )?;
    #[derive(Deserialize)]
    #[serde(tag = "type", deny_unknown_fields)]
    enum Command {
        #[serde(rename = "continue")]
        Continue,
    }
    let (send, receive) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let result = read_bounded(&mut std::io::stdin().lock(), 128)
            .map_err(|_| "custody-parent-frame")
            .and_then(|bytes| decode::<Command>(&bytes.ok_or("custody-parent-frame")?));
        let _ = send.send(result);
    });
    let finished = Arc::new(AtomicBool::new(false));
    let (watch_finished, watch_stop) = (finished.clone(), global.clone());
    thread::spawn(move || {
        let started = Instant::now();
        while !watch_finished.load(Ordering::SeqCst) {
            if !parent.alive() || started.elapsed() >= Duration::from_secs(8) {
                let _ = watch_stop.revoke();
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
    });
    receive
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| "custody-parent-frame")??;
    let mut arm_accepted = false;
    let mut output_pressure = false;
    if scenario == "cancel-before-arm" {
        process.stop(&mut registry)?;
    } else if ["lost-arm", "partial-arm"].contains(&scenario) {
        process.lose_arm(&mut registry, scenario == "partial-arm")?;
    } else {
        process.arm(&mut registry)?;
        process.ready()?;
        arm_accepted = true;
        if scenario == "blocked-output" {
            process.wait_output_pressure()?;
            output_pressure = true;
        }
        if scenario == "child-recovery" {
            drop(coordinator.input.take());
        } else {
            process.stop(&mut registry)?;
        }
    }
    let receipt_ok = process.receipt(&mut registry).is_ok();
    let receipt_before_exit =
        scenario == "hold-after-receipt" && !process.observe_exit(&mut registry)?;
    if scenario == "hold-after-receipt" {
        process.allow_exit()?;
    }
    let until = Instant::now() + Duration::from_secs(3);
    while !process.observe_exit(&mut registry)? {
        if Instant::now() >= until {
            return Err("custody-child-pending");
        }
        thread::sleep(Duration::from_millis(5));
    }
    let before_mirror = registry.completion(&ticket)?.is_some();
    if scenario != "child-recovery" && receipt_ok {
        process.mirror_probe_settled(&mut registry)?;
    }
    drop(coordinator.input.take());
    let until = Instant::now() + Duration::from_secs(3);
    while !coordinator.exited()? {
        if Instant::now() >= until {
            return Err("custody-coordinator-pending");
        }
        thread::sleep(Duration::from_millis(5));
    }
    registry.coordinator_exited();
    tree.close_admission();
    let until = Instant::now() + Duration::from_secs(3);
    while !tree.exited_after_admission_closed()? {
        if Instant::now() >= until {
            return Err("custody-processes-pending");
        }
        thread::sleep(Duration::from_millis(5));
    }
    finished.store(true, Ordering::SeqCst);
    emit_report(
        &serde_json::json!({"protocol":"palisade.assist.custody-probe","version":1,"stage":"complete",
        "mode":"probe","authority":"none","scenario":scenario,"armAccepted":arm_accepted,"receiptAccepted":receipt_ok,
        "beforeMirror":before_mirror,"receiptBeforeExit":receipt_before_exit,"processesExited":true,"outputPressure":output_pressure,
        "probeComplete":process.completion_is_probe(&registry)?,"nativeComplete":false}),
    )
}
fn run_child(parent: Parent, spec: Bootstrap) -> Result<(), &'static str> {
    if spec.monitor != parent.pid() {
        return Err("custody-parent");
    }
    let roles = [
        spec.arm,
        spec.receipt,
        spec.coordinator,
        spec.stop,
        spec.global,
        spec.exit,
    ];
    if roles
        .iter()
        .enumerate()
        .any(|(index, raw)| *raw == 0 || *raw == usize::MAX || roles[..index].contains(raw))
    {
        return Err("custody-handles");
    }
    let arm = unsafe { adopt(spec.arm, true)? };
    let mut receipt = File::from(unsafe { adopt(spec.receipt, true)? });
    let coordinator = unsafe { adopt(spec.coordinator, false)? };
    let stop = unsafe { adopt(spec.stop, false)? };
    let global = unsafe { adopt(spec.global, false)? };
    let exit = unsafe { adopt(spec.exit, false)? };
    if unsafe { GetProcessId(coordinator.as_raw_handle()) } != spec.coordinator_pid
        || unsafe { super::parent::creation_time(coordinator.as_raw_handle())? }
            != spec.coordinator_created
        || !super::parent::same_user_for_query(coordinator.as_raw_handle(), true)
    {
        return Err("custody-coordinator");
    }
    let (send, receive) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let result = read_bounded(&mut File::from(arm), MAX_FRAME)
            .map_err(|_| "custody-arm")
            .and_then(|bytes| decode::<Arm>(&bytes.ok_or("custody-arm")?));
        let _ = send.send(result);
    });
    let started = Instant::now();
    let mut accepted = false;
    loop {
        if !parent.alive() || wait_stop(&coordinator) || wait_stop(&stop) || wait_stop(&global) {
            break;
        }
        match receive.try_recv() {
            Ok(Ok(arm)) if !accepted && arm.ticket == spec.ticket && arm.mode == Mode::Probe => {
                accepted = true;
                if matches!(spec.fault, Fault::MissingArmed) {
                    break;
                }
                let mut ready_ticket = spec.ticket.clone();
                if matches!(spec.fault, Fault::WrongArmedTicket) {
                    let mut json =
                        serde_json::to_value(&ready_ticket).map_err(|_| "custody-message")?;
                    json["session"] = serde_json::json!("session_wrong_01");
                    ready_ticket = serde_json::from_value(json).map_err(|_| "custody-message")?;
                }
                let ready_mode = if matches!(spec.fault, Fault::WrongArmedMode) {
                    Mode::Native
                } else {
                    Mode::Probe
                };
                write_bounded(
                    &mut receipt,
                    &encode(&ChildFrame::Armed {
                        ticket: ready_ticket,
                        mode: ready_mode,
                    })?,
                    MAX_FRAME,
                )
                .map_err(|_| "custody-ready")?;
                if matches!(spec.fault, Fault::BlockedOutput) {
                    thread::spawn(|| {
                        let mut output = std::io::stdout().lock();
                        loop {
                            if output.write_all(&[b'x'; 8192]).is_err() {
                                break;
                            }
                        }
                    });
                }
            }
            Ok(_) | Err(mpsc::TryRecvError::Disconnected) if !accepted => break,
            _ => (),
        }
        if started.elapsed() >= Duration::from_secs(8) {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    // Terminal, and this mode has never owned or injected any OS input. Its
    // dedicated receipt explicitly remains Probe, not native cleanup evidence.
    let mut ticket = spec.ticket;
    let mut mode = Mode::Probe;
    match spec.fault {
        Fault::Missing => (),
        Fault::Truncated => {
            let _ = receipt.write_all(&[100, 0, 0, 0, b'{']);
        }
        fault => {
            if matches!(fault, Fault::WrongTicket) {
                let mut json = serde_json::to_value(&ticket).map_err(|_| "custody-message")?;
                json["session"] = serde_json::json!("session_wrong_01");
                ticket = serde_json::from_value(json).map_err(|_| "custody-message")?;
            }
            if matches!(fault, Fault::WrongMode) {
                mode = Mode::Native;
            }
            let bytes = encode(&ChildFrame::Released {
                ticket,
                mode,
                arm_accepted: accepted,
            })?;
            write_bounded(&mut receipt, &bytes, MAX_FRAME).map_err(|_| "custody-receipt")?;
            if matches!(fault, Fault::Duplicate) {
                write_bounded(&mut receipt, &bytes, MAX_FRAME).map_err(|_| "custody-receipt")?;
            }
        }
    }
    drop(receipt);
    if matches!(spec.fault, Fault::HoldAfterReceipt) {
        while !wait_stop(&exit) && parent.alive() && !wait_stop(&coordinator) && !wait_stop(&global)
        {
            thread::sleep(Duration::from_millis(5));
        }
    }
    if accepted {
        Ok(())
    } else {
        Err("custody-unarmed")
    }
}
#[cfg(test)]
mod tests;
