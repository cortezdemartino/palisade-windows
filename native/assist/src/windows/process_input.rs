//! Private guardian/child pipe. No listener, caller-selected binary or renderer route.
use super::{
    input::{ChildInput, WindowsBackend},
    parent::Parent,
};
use crate::{
    executor::{Backend, Source},
    framing::{read_frame, write_frame},
    input::{Action, Ledger},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::{
    os::windows::io::{AsRawHandle, OwnedHandle},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::WAIT_TIMEOUT,
    System::{SystemInformation::GetTickCount64, Threading::WaitForSingleObject},
};
pub(crate) mod custody;
struct ExecutorWitness {
    parent: OwnedHandle,
    stop: OwnedHandle,
    others: Vec<OwnedHandle>,
}
impl ExecutorWitness {
    fn retain(parent: &Parent, raw: usize) -> Result<Self, &'static str> {
        let witness = Self {
            parent: parent.wait_handle()?,
            stop: parent.duplicate_wait(raw)?,
            others: Vec::new(),
        };
        if witness.stopped() {
            return Err("executor-stopped");
        }
        Ok(witness)
    }
    fn stopped(&self) -> bool {
        unsafe {
            WaitForSingleObject(self.parent.as_raw_handle(), 0) != WAIT_TIMEOUT
                || WaitForSingleObject(self.stop.as_raw_handle(), 0) != WAIT_TIMEOUT
                || self
                    .others
                    .iter()
                    .any(|h| WaitForSingleObject(h.as_raw_handle(), 0) != WAIT_TIMEOUT)
        }
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Step {
    sequence: u32,
    expires: u64,
    source: Source,
    action: Option<Action>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Ack {
    sequence: u32,
    accepted: bool,
    uptime: u64,
}
/// Dedicated terminal channel, separate from the ordinary Step/Ack stream.
/// This attests a fenced executor ledger, never physical cooperative arbitration.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TerminalReceipt {
    protocol: String,
    version: u32,
    creator: u32,
    executor: u32,
    tag: usize,
    nonce: String,
    mode: crate::custody::Mode,
    fenced: bool,
    owned_clear: bool,
}
struct TerminalOwner {
    nonce: String,
    tag: usize,
    receive: mpsc::Receiver<Result<TerminalReceipt, &'static str>>,
    child: u32,
    expected_mode: crate::custody::Mode,
    confirmed: bool,
    failed: bool,
}
impl TerminalOwner {
    fn poll(&mut self) -> Result<(), &'static str> {
        if self.failed {
            return Err("executor-cleanup-unproven");
        }
        if self.confirmed {
            return Ok(());
        }
        match self.receive.try_recv() {
            Ok(Ok(receipt))
                if receipt.protocol == "palisade.assist.executor-terminal"
                    && receipt.version == 1
                    && receipt.creator == std::process::id()
                    && receipt.executor == self.child
                    && receipt.tag == self.tag
                    && receipt.nonce == self.nonce
                    && receipt.mode == self.expected_mode
                    && receipt.fenced
                    && receipt.owned_clear =>
            {
                self.confirmed = true;
                Ok(())
            }
            Err(mpsc::TryRecvError::Empty) => Err("executor-cleanup-pending"),
            _ => {
                self.failed = true;
                Err("executor-cleanup-unproven")
            }
        }
    }
}
fn read_terminal(mut pipe: impl std::io::Read) -> Result<TerminalReceipt, &'static str> {
    let bytes = crate::framing::read_bounded(&mut pipe, 4096)
        .map_err(|_| "executor-terminal-pipe")?
        .ok_or("executor-terminal-missing")?;
    let receipt = serde_json::from_slice::<TerminalReceipt>(&bytes)
        .map_err(|_| "executor-terminal-invalid")?;
    if crate::framing::read_bounded(&mut pipe, 4096)
        .map_err(|_| "executor-terminal-pipe")?
        .is_some()
    {
        return Err("executor-terminal-extra");
    }
    Ok(receipt)
}
fn uptime() -> u64 {
    unsafe { GetTickCount64() }
}
pub struct ProcessInput {
    process: ExecutorProcess,
    replies: mpsc::Receiver<Result<Ack, &'static str>>,
    sequence: u32,
    cleanup: WindowsBackend,
    probe: bool,
    mirror: Ledger,
}
/// Retain the exact created process through startup failure and terminal fencing.
/// EOF revokes ordinary work; the child's independent watchdog/cleanup path owns
/// release retries. Never kill that last release holder to make a wait succeed.
struct ExecutorProcess {
    child: ProcessHandle,
    pipe: Option<Box<dyn std::io::Write + Send>>,
    fenced: bool,
    reaped: bool,
    stop: Option<Arc<super::process_tree::StopEvent>>,
    custody: Option<custody::Managed>,
    startup_only: bool,
}
enum ProcessHandle {
    Child(Child),
    Monitored(OwnedHandle),
}
impl ProcessHandle {
    fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        use std::os::windows::process::ExitStatusExt;
        match self {
            Self::Child(child) => child.try_wait(),
            Self::Monitored(handle) => {
                match unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) } {
                    windows_sys::Win32::Foundation::WAIT_OBJECT_0 => {
                        let mut code = 0;
                        if unsafe {
                            windows_sys::Win32::System::Threading::GetExitCodeProcess(
                                handle.as_raw_handle(),
                                &mut code,
                            )
                        } == 0
                        {
                            return Err(std::io::Error::last_os_error());
                        }
                        Ok(Some(std::process::ExitStatus::from_raw(code)))
                    }
                    WAIT_TIMEOUT => Ok(None),
                    _ => Err(std::io::Error::last_os_error()),
                }
            }
        }
    }
    fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(status);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl ExecutorProcess {
    fn new(mut child: Child, stop: Option<Arc<super::process_tree::StopEvent>>) -> Self {
        Self {
            pipe: child
                .stdin
                .take()
                .map(|pipe| Box::new(pipe) as Box<dyn std::io::Write + Send>),
            child: ProcessHandle::Child(child),
            fenced: false,
            reaped: false,
            stop,
            custody: None,
            startup_only: true,
        }
    }
    fn close_admission(&mut self) -> Result<(), &'static str> {
        self.fenced = true;
        // Signal before closing/draining ordinary I/O. A buffered Step cannot
        // regain authority while its old timestamp is still otherwise valid.
        // Even signal failure must close stdin and retain/query the child.
        let signaled = self.stop.as_ref().map_or(Ok(()), |stop| {
            stop.revoke()?;
            if stop.signal_failed() {
                Err("executor-stop-failed")
            } else {
                Ok(())
            }
        });
        drop(self.pipe.take());
        signaled
    }
    fn fence(&mut self) -> Result<(), &'static str> {
        let signaled = self.close_admission();
        if !self.reaped {
            // Dispatch may hold SessionGate. Do not wait there for protected-
            // desktop cleanup or block independently authorized resources.
            if self
                .child
                .try_wait()
                .map_err(|_| "executor-stop")?
                .is_none()
            {
                signaled?;
                return Err("executor-cleanup-pending");
            }
            self.reaped = true;
        }
        signaled?;
        self.release_proof()?;
        Ok(())
    }
    fn release_proof(&mut self) -> Result<(), &'static str> {
        if !self.fenced || !self.reaped {
            return Err("executor-cleanup-pending");
        }
        if let Some(custody) = &self.custody {
            return custody.child_released();
        }
        Ok(()) // No-authority legacy probe only; normal construction requires custody.
    }
}
impl Drop for ExecutorProcess {
    fn drop(&mut self) {
        let _ = self.close_admission();
        while !self.reaped {
            if self.child.wait().is_ok() {
                self.reaped = true;
                break;
            }
            // A query/wait failure is not release or exit proof. Keep the exact
            // handle and let an independent Stop path remain effective.
            std::thread::sleep(Duration::from_millis(100));
        }
        if self.startup_only && self.custody.is_some() {
            // No Step was admitted. Even a failed READY/ARM exchange must wait
            // for native terminal evidence and settle the empty startup mirror.
            loop {
                let settled = self.release_proof().and_then(|_| {
                    self.custody
                        .as_mut()
                        .ok_or("custody-required")?
                        .mirror(&Ledger::default())
                });
                if settled.is_ok() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}
impl ProcessInput {
    pub(crate) fn probe_cancel_startup(
        client: &Arc<custody::Client>,
        registration: custody::Registration,
    ) -> Result<u32, &'static str> {
        if !client.probe() {
            return Err("probe-required");
        }
        let stop = Arc::new(super::process_tree::StopEvent::new()?);
        let managed = client.spawn(registration, 123, 0, stop.clone())?;
        let pid = managed.pid()?;
        // Deterministic cancellation at the actual creation/adoption/ARM boundary.
        // The same production constructor must retain its failed-startup owner.
        stop.revoke()?;
        if Self::from_monitored(managed, 123).err() != Some("custody-refused") {
            return Err("probe-cancel-failed");
        }
        Ok(pid)
    }
    pub(crate) fn pid(&self) -> u32 {
        match &self.process.child {
            ProcessHandle::Child(child) => child.id(),
            ProcessHandle::Monitored(handle) => unsafe {
                windows_sys::Win32::System::Threading::GetProcessId(handle.as_raw_handle())
            },
        }
    }
    pub fn spawn(
        tag: usize,
        probe: bool,
        manifest: &super::image::ImageManifest,
        _owner: &super::input_owner::InputOwner,
        _direct_stop: Arc<super::process_tree::StopEvent>,
    ) -> Result<Self, &'static str> {
        if !probe {
            return Err("monitor-custody-required");
        }
        let verified = super::image::VerifiedImage::open(manifest)?;
        let mut command = Command::new(verified.path());
        command.args([
            "--executor-probe",
            &std::process::id().to_string(),
            &tag.to_string(),
        ]);
        let child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "executor-start")?;
        Self::from_probe_child(child, tag, true, None)
    }
    #[cfg(test)]
    fn from_child(
        child: Child,
        tag: usize,
        probe: bool,
        stop: Option<Arc<super::process_tree::StopEvent>>,
    ) -> Result<Self, &'static str> {
        Self::from_probe_child(child, tag, probe, stop)
    }
    fn from_probe_child(
        mut child: Child,
        tag: usize,
        probe: bool,
        stop: Option<Arc<super::process_tree::StopEvent>>,
    ) -> Result<Self, &'static str> {
        // Establish the retained owner before any pipe extraction, reader setup
        // or startup result can return an error after CreateProcess succeeded.
        let output = child.stdout.take().ok_or("executor-pipe");
        let process = ExecutorProcess::new(child, stop);
        if !probe {
            return Err("monitor-custody-required");
        }
        if process.pipe.is_none() {
            return Err("executor-pipe");
        }
        Self::from_transport(process, output?, tag, probe)
    }
    pub(crate) fn spawn_monitored(
        client: &Arc<custody::Client>,
        registration: custody::Registration,
        tag: usize,
        owner: usize,
        stop: Arc<super::process_tree::StopEvent>,
    ) -> Result<Self, &'static str> {
        let custody = client.spawn(registration, tag, owner, stop)?;
        Self::from_monitored(custody, tag)
    }
    pub(crate) fn from_monitored(
        mut custody: custody::Managed,
        tag: usize,
    ) -> Result<Self, &'static str> {
        let probe = custody.probe();
        let stop = custody.stop_event();
        let (handle, input, output) = custody.take_transport()?;
        let process = ExecutorProcess {
            child: ProcessHandle::Monitored(handle),
            pipe: Some(Box::new(input)),
            fenced: false,
            reaped: false,
            stop: Some(stop),
            custody: Some(custody),
            startup_only: true,
        };
        // The retained ProcessInput guard exists before ARM can reach the child.
        process.custody.as_ref().ok_or("custody-required")?.arm()?;
        Self::from_transport(process, output, tag, probe)
    }
    fn from_transport(
        mut process: ExecutorProcess,
        output: impl std::io::Read + Send + 'static,
        tag: usize,
        probe: bool,
    ) -> Result<Self, &'static str> {
        let (send, replies) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut output = output;
            loop {
                let response = read_frame(&mut output)
                    .map_err(|_| "executor-pipe")
                    .and_then(|v| {
                        let bytes = v.ok_or("executor-ended")?;
                        serde_json::from_slice::<Ack>(&bytes).map_err(|_| "executor-response")
                    });
                let failed = response.is_err();
                if send.send(response).is_err() || failed {
                    break;
                }
            }
        });
        let ready = replies
            .recv_timeout(Duration::from_secs(3))
            .map_err(|_| "executor-start")
            .and_then(|reply| reply);
        let ready = ready?;
        if ready.sequence != u32::MAX || !ready.accepted {
            return Err("executor-start");
        }
        process.startup_only = false;
        Ok(Self {
            process,
            replies,
            sequence: 0,
            cleanup: WindowsBackend::new(tag),
            probe,
            mirror: Ledger::default(),
        })
    }
    fn terminate(&mut self) -> Result<(), &'static str> {
        self.process.fence()
    }
}
impl ProcessInput {
    fn transmit(&mut self, source: &Source, action: Option<Action>) -> Result<(), &'static str> {
        if self.process.fenced {
            return Err("revoked");
        }
        let request = Step {
            sequence: self.sequence,
            expires: uptime().checked_add(75).ok_or("clock-invalid")?,
            source: source.clone(),
            action,
        };
        self.sequence = self.sequence.checked_add(1).ok_or("sequence-exhausted")?;
        write_frame(
            self.process.pipe.as_mut().ok_or("executor-pipe")?,
            &serde_json::to_vec(&request).map_err(|_| "invalid-message")?,
        )
        .map_err(|_| "executor-pipe")?;
        let ack = self
            .replies
            .recv_timeout(Duration::from_millis(90))
            .map_err(|_| "executor-timeout")??;
        if ack.sequence != request.sequence || !ack.accepted || uptime() >= request.expires {
            return Err("executor-refused");
        }
        Ok(())
    }
}
impl Backend for ProcessInput {
    fn requires_registration(&self) -> bool {
        !self.probe
    }
    fn retain_cleanup(&self) -> bool {
        !self.probe
    }
    fn heartbeat(&mut self, source: &Source) -> Result<(), &'static str> {
        if self.process.fenced {
            return Err("revoked");
        }
        self.transmit(source, None)
    }
    fn prepare(&mut self) -> Result<(), &'static str> {
        if self.process.fenced && (!self.probe || self.process.custody.is_some()) {
            self.process.release_proof()?;
            if !self.process.reaped {
                return Err("executor-cleanup-pending");
            }
            return Ok(());
        }
        if self.probe {
            return Ok(());
        }
        self.cleanup.prepare()
    }
    fn validate(
        &mut self,
        source: &Source,
        action: &Action,
        ledger: &Ledger,
    ) -> Result<(), &'static str> {
        if self.probe {
            source.point(0., 0.)?;
            return action.validate();
        }
        self.cleanup.validate(source, action, ledger)
    }
    // The guardian's Executor owns this write-ahead mirror; a down cannot be
    // sent until its intent is in that surviving process's ledger.
    fn intent(&mut self, ledger: &Ledger) -> Result<(), &'static str> {
        if let Some(custody) = &mut self.process.custody {
            custody.mirror(ledger)?;
        }
        self.mirror = ledger.clone();
        Ok(())
    }
    fn apply(&mut self, source: &Source, action: &Action) -> Result<(), &'static str> {
        if self.process.fenced {
            if !self.process.reaped {
                return Err("executor-cleanup-pending");
            }
            if !matches!(
                action,
                Action::Key { down: false, .. }
                    | Action::Button { down: false, .. }
                    | Action::Unicode { down: false, .. }
            ) || !self.mirror.is_held(action)
            {
                return Err("cleanup-only");
            }
            if self.probe && self.process.custody.is_none() {
                return Ok(());
            }
            // The authenticated terminal proves this generation has fenced and
            // cleared its own ledger. Settle the write-ahead mirror logically;
            // injecting a second UP could release input now owned by the host.
            self.process.release_proof()?;
            return Ok(());
        }
        self.transmit(source, Some(action.clone()))
    }
    fn fence(&mut self) -> Result<(), &'static str> {
        self.terminate()
    }
    fn cleanup_allowed(&mut self) -> Result<(), &'static str> {
        if self.process.fenced && (!self.probe || self.process.custody.is_some()) {
            return self.process.release_proof();
        }
        if self.probe {
            return Ok(());
        }
        self.cleanup.cleanup_allowed()
    }
    fn finish(&mut self) {
        if self.probe {
            return;
        }
        self.cleanup.finish();
    }
}
impl Drop for ProcessInput {
    fn drop(&mut self) {
        let _ = self.terminate();
        if self.process.custody.is_some() {
            loop {
                let settled = self.process.fence().and_then(|_| {
                    // The native child proved its whole owned ledger clear.
                    // This retires the corresponding logical mirror, never
                    // sends a second round of OS UP events.
                    self.process
                        .custody
                        .as_mut()
                        .ok_or("custody-required")?
                        .mirror(&Ledger::default())
                });
                if settled.is_ok() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

pub fn run_child(
    parent_pid: u32,
    tag: usize,
    probe: bool,
    terminal_nonce: Option<String>,
) -> Result<(), &'static str> {
    if !probe {
        return Err("monitor-custody-required");
    }
    let parent = Parent::validate_executor(parent_pid, probe)?;
    if let Some(nonce) = &terminal_nonce {
        crate::grant::decode::<32>(nonce)?;
    }
    run_engine(parent, tag, true, None, terminal_nonce, None, None)
}
/// Only the validated monitor can create this role, supply its sealed source,
/// and send ARM on the dedicated inherited pipe. Ordinary Step cannot arm it.
pub fn run_owned_child(parent_pid: u32, probe: bool) -> Result<(), &'static str> {
    use std::os::windows::io::AsRawHandle;
    let parent = Parent::validate_executor(parent_pid, probe)?;
    if !probe && !cfg!(feature = "native-input") {
        return Err("native-input-disabled");
    }
    let spec: custody::Bootstrap = custody::read(&mut std::io::stdin().lock())?;
    crate::grant::decode::<32>(&spec.nonce)?;
    spec.source.point(0., 0.)?;
    if spec.tag == 0 || (!probe && spec.owner == 0) || spec.stop == spec.owner {
        return Err("custody-bootstrap");
    }
    if !probe && spec.source_stop == 0 {
        return Err("source-stop-required");
    }
    let mut roles = vec![spec.coordinator, spec.global, spec.arm];
    if spec.source_stop != 0 {
        roles.push(spec.source_stop);
    }
    if roles
        .iter()
        .enumerate()
        .any(|(i, h)| *h == 0 || roles[..i].contains(h))
    {
        return Err("custody-handles");
    }
    let (coordinator, global, arm) = unsafe {
        (
            custody::adopt(spec.coordinator, false)?,
            custody::adopt(spec.global, false)?,
            custody::adopt(spec.arm, true)?,
        )
    };
    if unsafe { windows_sys::Win32::System::Threading::GetProcessId(coordinator.as_raw_handle()) }
        != spec.coordinator_pid
        || unsafe { super::parent::creation_time(coordinator.as_raw_handle())? }
            != spec.coordinator_created
        || !super::parent::same_user_for_query(coordinator.as_raw_handle(), probe)
    {
        return Err("custody-coordinator");
    }
    let mut witness = ExecutorWitness::retain(&parent, spec.stop)?;
    witness.others = vec![coordinator, global];
    if spec.source_stop != 0 {
        witness
            .others
            .push(unsafe { custody::adopt(spec.source_stop, false)? });
    }
    let _owner = if probe {
        None
    } else {
        Some(super::input_owner::InputOwner::from_parent(
            &parent, spec.owner,
        )?)
    };
    let (send, receive) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut pipe = std::fs::File::from(arm);
        let result = (|| {
            let ticket: crate::custody::Ticket = custody::read(&mut pipe)?;
            if ticket != spec.ticket {
                return Err("custody-arm-binding");
            }
            if crate::framing::read_bounded(&mut pipe, 16_384)
                .map_err(|_| "custody-arm")?
                .is_some()
            {
                return Err("custody-arm-extra");
            }
            Ok(())
        })();
        let _ = send.send(result);
    });
    run_engine(
        parent,
        spec.tag,
        probe,
        Some(Arc::new(witness)),
        Some(spec.nonce),
        Some(spec.source),
        Some(receive),
    )
}
fn run_engine(
    parent: Parent,
    tag: usize,
    probe: bool,
    witness: Option<Arc<ExecutorWitness>>,
    terminal_nonce: Option<String>,
    sealed_source: Option<Source>,
    arm: Option<mpsc::Receiver<Result<(), &'static str>>>,
) -> Result<(), &'static str> {
    let parent_pid = parent.pid();
    let stop = Arc::new(AtomicBool::new(false));
    let source = Arc::new(Mutex::new(None::<Source>));
    let input = Arc::new(Mutex::new(ChildInput::new(tag)));
    let (sender, receive) = mpsc::sync_channel(1);
    let read_stop = stop.clone();
    std::thread::spawn(move || {
        let mut pipe = std::io::stdin().lock();
        loop {
            match read_frame(&mut pipe) {
                Ok(Some(bytes)) => {
                    if sender.send(bytes).is_err() {
                        break;
                    }
                }
                _ => {
                    read_stop.store(true, Ordering::SeqCst);
                    break;
                }
            }
        }
    });
    let heartbeat = Arc::new(Mutex::new(Instant::now()));
    let (watch_stop, watch_input, watch_source, watch_time) = (
        stop.clone(),
        input.clone(),
        source.clone(),
        heartbeat.clone(),
    );
    let watch_witness = witness.clone();
    let cleanup_owner = std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(10));
            if !parent.alive()
                || watch_witness.as_ref().is_some_and(|w| w.stopped())
                || watch_time
                    .lock()
                    .map(|t| t.elapsed() >= Duration::from_secs(2))
                    .unwrap_or(true)
            {
                watch_stop.store(true, Ordering::SeqCst);
            }
            if watch_stop.load(Ordering::SeqCst) {
                if !probe {
                    // Retain cleanup-only authority on protected desktops. Never
                    // inject there; retry release after Default returns, then exit.
                    loop {
                        let done = match (watch_input.lock(), watch_source.lock()) {
                            (Ok(mut input), Ok(source)) => {
                                source.as_ref().is_none_or(|s| input.cleanup(s).is_ok())
                                    && input.owned_clear()
                            }
                            _ => false,
                        };
                        if done {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                if let Some(nonce) = &terminal_nonce {
                    let receipt = TerminalReceipt {
                        protocol: "palisade.assist.executor-terminal".into(),
                        version: 1,
                        creator: parent_pid,
                        executor: std::process::id(),
                        tag,
                        nonce: nonce.clone(),
                        mode: if probe {
                            crate::custody::Mode::Probe
                        } else {
                            crate::custody::Mode::Native
                        },
                        fenced: true,
                        owned_clear: true,
                    };
                    if let Ok(bytes) = serde_json::to_vec(&receipt) {
                        let _ = crate::framing::write_bounded(
                            &mut std::io::stderr().lock(),
                            &bytes,
                            4096,
                        );
                    }
                }
                std::process::exit(125);
            }
        }
    });
    let outcome = crate::cleanup::attempt(|| {
        if let Some(arm) = arm {
            loop {
                if stop.load(Ordering::SeqCst) || witness.as_ref().is_some_and(|w| w.stopped()) {
                    return Err("revoked");
                }
                match arm.recv_timeout(Duration::from_millis(10)) {
                    Ok(result) => {
                        result?;
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => (),
                    Err(_) => return Err("custody-arm-missing"),
                }
            }
        }
        let _activity = if probe {
            None
        } else {
            Some(super::activity::ActivityMonitor::start(stop.clone(), tag)?)
        };
        if !probe && !super::input::local_input_idle() {
            stop.store(true, Ordering::SeqCst);
            return Err("local-input-held");
        }
        if stop.load(Ordering::SeqCst) || witness.as_ref().is_some_and(|w| w.stopped()) {
            stop.store(true, Ordering::SeqCst);
            return Err("revoked");
        }
        let mut sequence = 0u32;
        let mut output = std::io::stdout().lock();
        write_frame(
            &mut output,
            &serde_json::to_vec(&Ack {
                sequence: u32::MAX,
                accepted: true,
                uptime: uptime(),
            })
            .map_err(|_| "invalid-message")?,
        )
        .map_err(|_| "executor-pipe")?;
        let mut simulated = Ledger::default();
        loop {
            let bytes = receive.recv_timeout(Duration::from_millis(25));
            if stop.load(Ordering::SeqCst) || witness.as_ref().is_some_and(|w| w.stopped()) {
                stop.store(true, Ordering::SeqCst);
                return Err("revoked");
            }
            let bytes = match bytes {
                Ok(v) => v,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => return Err("executor-ended"),
            };
            let request: Step = serde_json::from_slice(&bytes).map_err(|_| "invalid-message")?;
            if request.sequence != sequence
                || request.expires <= uptime()
                || request.expires > uptime().saturating_add(100)
            {
                return Err("replay");
            }
            sequence = sequence.checked_add(1).ok_or("sequence-exhausted")?;
            *heartbeat.lock().map_err(|_| "executor-failed")? = Instant::now();
            request.source.point(0., 0.)?;
            if sealed_source
                .as_ref()
                .is_some_and(|source| source != &request.source)
            {
                return Err("custody-source-changed");
            }
            if let Some(action) = &request.action {
                action.validate()?;
            }
            *source.lock().map_err(|_| "executor-failed")? = Some(request.source.clone());
            let result = if request.action.is_none() {
                Ok(())
            } else if probe {
                if matches!(
                    request.action,
                    Some(Action::Shortcut(_) | Action::ReleaseAll)
                ) {
                    Err("invalid-message")
                } else {
                    simulated
                        .plan(request.action.as_ref().ok_or("invalid-message")?)
                        .map(|_| ())
                }
            } else {
                input.lock().map_err(|_| "executor-failed")?.step(
                    &request.source,
                    request.action.as_ref().ok_or("invalid-message")?,
                    || {
                        !stop.load(Ordering::SeqCst)
                            && witness.as_ref().is_none_or(|w| !w.stopped())
                            && uptime() < request.expires
                    },
                )
            };
            write_frame(
                &mut output,
                &serde_json::to_vec(&Ack {
                    sequence: request.sequence,
                    accepted: result.is_ok(),
                    uptime: uptime(),
                })
                .map_err(|_| "invalid-message")?,
            )
            .map_err(|_| "executor-pipe")?;
            result?;
        }
    });
    stop.store(true, Ordering::SeqCst);
    // Exactly one owner settles native input and publishes the terminal. It is
    // independent of a blocked ordinary stdout writer and exits only afterwards.
    let _ = cleanup_owner.join();
    outcome
}

#[cfg(test)]
mod tests;
