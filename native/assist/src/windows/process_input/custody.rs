//! Monitor-owned normal executor creation and dedicated cleanup evidence.
//! The coordinator receives only fixed-image pipes and an exact query handle.
use super::super::{
    image::VerifiedImage,
    process_tree::{ProcessTree, StopEvent},
};
use super::*;
use crate::{
    custody::{Mode, ProcessIdentity, Registry, Ticket},
    framing::{read_bounded, write_bounded},
};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    os::windows::io::{AsHandle, FromRawHandle},
    ptr, thread,
};
use windows_sys::Win32::{
    Foundation::*,
    Storage::FileSystem::{FILE_TYPE_PIPE, GetFileType},
    System::Threading::*,
};
const MAX: usize = 16_384;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Registration {
    pub session: String,
    pub slot: u64,
    pub execution: u64,
    pub source: Source,
    pub source_key: Option<super::super::source_stop::SourceKey>,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields, rename_all = "kebab-case")]
enum Request {
    Source {
        key: super::super::source_stop::SourceKey,
    },
    Spawn {
        registration: Registration,
        tag: usize,
        owner: usize,
        stop: usize,
    },
    Arm {
        ticket: Ticket,
    },
    Observe {
        ticket: Ticket,
    },
    Mirror {
        ticket: Ticket,
        revision: u64,
        held: BTreeMap<String, bool>,
    },
    Seal {},
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
    sequence: u64,
    command: Request,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields, rename_all = "kebab-case")]
enum Answer {
    Source {
        key: super::super::source_stop::SourceKey,
        stop: usize,
    },
    Spawned {
        ticket: Ticket,
        process: usize,
        input: usize,
        output: usize,
        pid: u32,
        created: u64,
    },
    Observed {
        released: bool,
        exited: bool,
        never_armed: bool,
    },
    Done {},
    Refused {},
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    sequence: u64,
    result: Answer,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Bootstrap {
    pub ticket: Ticket,
    pub tag: usize,
    pub nonce: String,
    pub owner: usize,
    pub stop: usize,
    pub coordinator: usize,
    pub coordinator_pid: u32,
    pub coordinator_created: u64,
    pub global: usize,
    pub arm: usize,
    pub source: Source,
    pub source_stop: usize,
}
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, &'static str> {
    serde_json::to_vec(value).map_err(|_| "custody-encoding")
}
pub(super) fn read<T: for<'de> Deserialize<'de>>(pipe: &mut impl Read) -> Result<T, &'static str> {
    serde_json::from_slice(
        &read_bounded(pipe, MAX)
            .map_err(|_| "custody-frame")?
            .ok_or("custody-eof")?,
    )
    .map_err(|_| "custody-message")
}
fn write<T: Serialize>(pipe: &mut impl Write, value: &T) -> Result<(), &'static str> {
    write_bounded(pipe, &encode(value)?, MAX).map_err(|_| "custody-pipe")
}
fn clear_inherit(handle: &OwnedHandle) -> Result<(), &'static str> {
    if unsafe { SetHandleInformation(handle.as_raw_handle(), HANDLE_FLAG_INHERIT, 0) } == 0 {
        return Err("custody-handle");
    }
    Ok(())
}
pub(super) unsafe fn adopt(raw: usize, pipe: bool) -> Result<OwnedHandle, &'static str> {
    if raw == 0 || raw == usize::MAX {
        return Err("custody-handle");
    }
    let mut flags = 0;
    if unsafe { GetHandleInformation(raw as HANDLE, &mut flags) } == 0 {
        return Err("custody-handle");
    }
    let handle = unsafe { OwnedHandle::from_raw_handle(raw as HANDLE) };
    if pipe && unsafe { GetFileType(handle.as_raw_handle()) } != FILE_TYPE_PIPE {
        return Err("custody-handle");
    }
    clear_inherit(&handle)?;
    Ok(handle)
}
/// Adopt the entire authenticated reply before validation can short-circuit.
/// On any invalid role, close every valid distinct transferred handle.
unsafe fn adopt_transport(roles: [usize; 3]) -> Result<(OwnedHandle, File, File), &'static str> {
    let mut owned = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut failed = false;
    for raw in roles {
        if !seen.insert(raw) {
            failed = true;
            continue;
        }
        match unsafe { adopt(raw, false) } {
            Ok(handle) => owned.push(handle),
            Err(_) => failed = true,
        }
    }
    if failed || owned.len() != 3 {
        return Err("custody-handle");
    }
    let output = owned.pop().ok_or("custody-handle")?;
    let input = owned.pop().ok_or("custody-handle")?;
    let process = owned.pop().ok_or("custody-handle")?;
    if [input.as_raw_handle(), output.as_raw_handle()]
        .iter()
        .any(|raw| unsafe { GetFileType(*raw) } != FILE_TYPE_PIPE)
    {
        return Err("custody-handle");
    }
    Ok((process, File::from(input), File::from(output)))
}
fn duplicate(raw: HANDLE, rights: u32, inherit: bool) -> Result<OwnedHandle, &'static str> {
    let mut out = ptr::null_mut();
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            raw,
            GetCurrentProcess(),
            &mut out,
            rights,
            inherit as i32,
            0,
        )
    } == 0
    {
        return Err("custody-duplicate");
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(out) })
}
fn random() -> Result<[u8; 32], &'static str> {
    let mut bytes = [0; 32];
    if unsafe {
        windows_sys::Win32::Security::Cryptography::BCryptGenRandom(
            ptr::null_mut(),
            bytes.as_mut_ptr(),
            32,
            windows_sys::Win32::Security::Cryptography::BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    } < 0
    {
        return Err("custody-random");
    }
    Ok(bytes)
}

pub(crate) struct Pipes {
    pub request: OwnedHandle,
    pub response: OwnedHandle,
    pub urgent_sender: OwnedHandle,
    pub urgent: Arc<StopEvent>,
    receive: File,
    send: File,
}
impl Pipes {
    pub fn new() -> Result<Self, &'static str> {
        let (receive, request) = super::super::process_tree::custody_pipe()?;
        let (response, send) = super::super::process_tree::custody_pipe()?;
        clear_inherit(&receive)?;
        clear_inherit(&send)?;
        let urgent = Arc::new(StopEvent::new()?);
        Ok(Self {
            request,
            response,
            urgent_sender: urgent.sender()?,
            urgent,
            receive: File::from(receive),
            send: File::from(send),
        })
    }
}
type RpcResult = Result<Answer, &'static str>;
enum ReplyKind {
    Spawn,
    Observe,
    Done,
    Source,
}
impl ReplyKind {
    fn of(request: &Request) -> Self {
        match request {
            Request::Source { .. } => Self::Source,
            Request::Spawn { .. } => Self::Spawn,
            Request::Observe { .. } => Self::Observe,
            _ => Self::Done,
        }
    }
    fn accepts(&self, answer: &Answer) -> bool {
        matches!(
            (self, answer),
            (_, Answer::Refused {})
                | (Self::Source, Answer::Source { .. })
                | (Self::Spawn, Answer::Spawned { .. })
                | (Self::Observe, Answer::Observed { .. })
                | (Self::Done, Answer::Done {})
        )
    }
}
struct Rpc {
    command: Request,
    reply: mpsc::SyncSender<RpcResult>,
}
fn reply_channel() -> (mpsc::SyncSender<RpcResult>, mpsc::Receiver<RpcResult>) {
    // Rendezvous only: no raw handle bundle may be queued in an unread receiver
    // and silently dropped after a timeout. Send succeeds only on ownership handoff.
    mpsc::sync_channel(0)
}
pub(crate) struct Client {
    requests: mpsc::SyncSender<Rpc>,
    failed: Arc<AtomicBool>,
    mode: Mode,
    urgent: Arc<StopEvent>,
    monitor: Option<Mutex<Parent>>,
}
impl Client {
    pub(crate) fn probe(&self) -> bool {
        self.mode == Mode::Probe
    }
    /// Called only after the creator and inherited role list were validated.
    pub unsafe fn inherited(
        request: usize,
        response: usize,
        urgent: usize,
        monitor: Parent,
        probe: bool,
    ) -> Result<Arc<Self>, &'static str> {
        if request == response || request == urgent || response == urgent {
            return Err("custody-handles");
        }
        let (mut send, mut receive) = unsafe {
            (
                File::from(adopt(request, true)?),
                File::from(adopt(response, true)?),
            )
        };
        let urgent = Arc::new(StopEvent::from_owned(unsafe { adopt(urgent, false)? })?);
        let (requests, incoming) = mpsc::sync_channel::<Rpc>(8);
        let failed = Arc::new(AtomicBool::new(false));
        let worker_failed = failed.clone();
        thread::spawn(move || {
            let mut sequence = 0u64;
            for rpc in incoming {
                let result = (|| {
                    if worker_failed.load(Ordering::SeqCst) {
                        return Err("custody-unavailable");
                    }
                    let expected = ReplyKind::of(&rpc.command);
                    write(
                        &mut send,
                        &Message {
                            sequence,
                            command: rpc.command,
                        },
                    )?;
                    let reply: Reply = read(&mut receive)?;
                    if reply.sequence != sequence {
                        discard(reply.result);
                        return Err("custody-replay");
                    }
                    if !expected.accepts(&reply.result) {
                        discard(reply.result);
                        return Err("custody-response");
                    }
                    sequence = sequence
                        .checked_add(1)
                        .ok_or("custody-sequence-exhausted")?;
                    Ok(reply.result)
                })();
                if result.is_err() {
                    worker_failed.store(true, Ordering::SeqCst);
                }
                deliver(rpc.reply, result);
            }
        });
        Ok(Arc::new(Self {
            requests,
            failed,
            mode: if probe { Mode::Probe } else { Mode::Native },
            urgent,
            monitor: Some(Mutex::new(monitor)),
        }))
    }
    fn call(&self, command: Request) -> Result<Answer, &'static str> {
        if self.failed.load(Ordering::SeqCst) {
            return Err("custody-unavailable");
        }
        let (reply, receive) = reply_channel();
        let result = self
            .requests
            .try_send(Rpc { command, reply })
            .map_err(|_| "custody-busy")
            .and_then(|_| {
                receive
                    .recv_timeout(Duration::from_secs(4))
                    .map_err(|_| "custody-timeout")?
            })
            .and_then(|answer| {
                if matches!(answer, Answer::Refused {}) {
                    Err("custody-refused")
                } else {
                    Ok(answer)
                }
            });
        if result.is_err() && !matches!(result, Err("custody-refused")) {
            self.failed.store(true, Ordering::SeqCst);
            let _ = self.urgent.revoke();
        }
        result
    }
    pub fn seal(&self) -> Result<(), &'static str> {
        // Signal before any bounded actor, ordinary pipe or monitor mutex.
        self.urgent.revoke()?;
        match self.call(Request::Seal {})? {
            Answer::Done {} => Ok(()),
            _ => Err("custody-response"),
        }
    }
    pub(crate) fn source(
        &self,
        key: &super::super::source_stop::SourceKey,
    ) -> Result<Arc<super::super::source_stop::SourceWitness>, &'static str> {
        match self.call(Request::Source { key: key.clone() })? {
            Answer::Source {
                key: observed,
                stop,
            } if observed == *key => {
                let parent = self
                    .monitor
                    .as_ref()
                    .ok_or("custody-parent")?
                    .lock()
                    .map_err(|_| "custody-parent")?;
                super::super::source_stop::SourceWitness::retain(&parent, stop)
            }
            _ => Err("custody-source"),
        }
    }
    pub fn spawn(
        self: &Arc<Self>,
        registration: Registration,
        tag: usize,
        owner: usize,
        stop: Arc<StopEvent>,
    ) -> Result<Managed, &'static str> {
        struct StopOnFailure(Arc<StopEvent>, bool);
        impl Drop for StopOnFailure {
            fn drop(&mut self) {
                if self.1 {
                    let _ = self.0.revoke();
                }
            }
        }
        let mut guard = StopOnFailure(stop.clone(), true);
        let answer = self.call(Request::Spawn {
            registration,
            tag,
            owner,
            stop: stop.raw(),
        })?;
        let Answer::Spawned {
            ticket,
            process,
            input,
            output,
            pid,
            created,
        } = answer
        else {
            return Err("custody-response");
        };
        // Own every transferred handle before an ARM or fallible readiness step.
        let (process, input, output) = unsafe { adopt_transport([process, input, output])? };
        if unsafe { GetProcessId(process.as_raw_handle()) } != pid
            || unsafe { super::super::parent::creation_time(process.as_raw_handle())? } != created
        {
            stop.revoke()?;
            return Err("custody-process");
        }
        let managed = Managed {
            client: self.clone(),
            ticket,
            process: Some(process),
            input: Some(input),
            output: Some(output),
            stop,
            mirror_revision: 0,
            mirror_failed: false,
        };
        guard.1 = false;
        Ok(managed)
    }
}
fn deliver(
    reply: mpsc::SyncSender<Result<Answer, &'static str>>,
    result: Result<Answer, &'static str>,
) {
    if let Err(mpsc::SendError(Ok(answer))) = reply.send(result) {
        // Only this actor owns a late response that its caller never received.
        // Adopt the full bundle, close it, and never send ARM.
        discard(answer);
    }
}
fn discard(answer: Answer) {
    if let Answer::Spawned {
        process,
        input,
        output,
        ..
    } = answer
    {
        let _ = unsafe { adopt_transport([process, input, output]) };
    }
}
pub(crate) struct Managed {
    client: Arc<Client>,
    ticket: Ticket,
    process: Option<OwnedHandle>,
    input: Option<File>,
    output: Option<File>,
    stop: Arc<StopEvent>,
    mirror_revision: u64,
    mirror_failed: bool,
}
impl Managed {
    pub(super) fn probe(&self) -> bool {
        self.client.probe()
    }
    pub(super) fn stop_event(&self) -> Arc<StopEvent> {
        self.stop.clone()
    }
    pub(super) fn pid(&self) -> Result<u32, &'static str> {
        self.process
            .as_ref()
            .map(|handle| unsafe { GetProcessId(handle.as_raw_handle()) })
            .filter(|pid| *pid != 0)
            .ok_or("custody-process")
    }
    pub(super) fn take_transport(&mut self) -> Result<(OwnedHandle, File, File), &'static str> {
        Ok((
            self.process.take().ok_or("custody-process")?,
            self.input.take().ok_or("custody-pipe")?,
            self.output.take().ok_or("custody-pipe")?,
        ))
    }
    pub(super) fn arm(&self) -> Result<(), &'static str> {
        match self.client.call(Request::Arm {
            ticket: self.ticket.clone(),
        })? {
            Answer::Done {} => Ok(()),
            _ => Err("custody-response"),
        }
    }
    pub(super) fn child_released(&self) -> Result<(), &'static str> {
        match self.client.call(Request::Observe {
            ticket: self.ticket.clone(),
        })? {
            Answer::Observed {
                released: true,
                exited: true,
                ..
            } => Ok(()),
            Answer::Observed {
                never_armed: true,
                exited: true,
                ..
            } => Ok(()),
            Answer::Observed { .. } => Err("executor-cleanup-pending"),
            _ => Err("custody-response"),
        }
    }
    pub(super) fn mirror(&mut self, ledger: &Ledger) -> Result<(), &'static str> {
        if self.mirror_failed {
            return Err("input-mirror-unsettled");
        }
        let result = (|| {
            if self.stop.signal_failed() {
                return Err("executor-stop-failed");
            }
            let revision = self
                .mirror_revision
                .checked_add(1)
                .ok_or("custody-sequence-exhausted")?;
            match self.client.call(Request::Mirror {
                ticket: self.ticket.clone(),
                revision,
                held: ledger.snapshot(),
            })? {
                Answer::Done {} => {
                    self.mirror_revision = revision;
                    Ok(())
                }
                _ => Err("custody-response"),
            }
        })();
        if result.is_err() {
            self.mirror_failed = true;
        }
        result
    }
}
impl Drop for Managed {
    fn drop(&mut self) {
        let _ = self.stop.revoke();
        drop(self.input.take());
        // Once attached to ProcessInput it owns the process/stdio handles; this
        // retains only custody identity. An unattached startup guard still joins.
        if let Some(process) = &self.process {
            while unsafe { WaitForSingleObject(process.as_raw_handle(), 100) } != WAIT_OBJECT_0 {}
        }
    }
}

struct Entry {
    ticket: Ticket,
    process: OwnedHandle,
    input: Option<File>,
    output: Option<File>,
    arm: Option<File>,
    stop: Arc<StopEvent>,
    terminal: TerminalOwner,
    identity: Option<ProcessIdentity>,
    _owner: Option<OwnedHandle>,
    mirror_revision: u64,
    mirror_complete: bool,
    armed: bool,
    released: bool,
    exited: bool,
    failed: bool,
    handoff: Option<Handoff>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Publication {
    Prepared,
    Uncertain,
    Published,
    Adopted,
    CoordinatorExited,
}
struct Handoff {
    handles: Vec<usize>,
    phase: Publication,
}
impl Handoff {
    fn permits_retirement(&self) -> bool {
        matches!(
            self.phase,
            Publication::Adopted | Publication::CoordinatorExited
        )
    }
    fn publish(&mut self, output: &mut impl Write, bytes: &[u8]) -> Result<(), &'static str> {
        if self.phase != Publication::Prepared {
            return Err("custody-handoff");
        }
        // Even flush failure after full delivery is ambiguous. The recipient may
        // have adopted/closed/reused a handle; never numeric-close after this point.
        self.phase = Publication::Uncertain;
        write_bounded(output, bytes, MAX).map_err(|_| "custody-pipe")?;
        self.phase = Publication::Published;
        Ok(())
    }
    fn adopted(&mut self) -> Result<(), &'static str> {
        if self.phase != Publication::Published {
            return Err("custody-handoff");
        }
        self.phase = Publication::Adopted;
        self.handles.clear();
        Ok(())
    }
    fn coordinator_exited(&mut self) {
        // Caller must hold and positively observe the exact coordinator handle.
        // Windows closed any unadopted copies; this settles no input receipts.
        self.phase = Publication::CoordinatorExited;
        self.handles.clear();
    }
}
struct State {
    tree: ProcessTree,
    image: VerifiedImage,
    registry: Registry,
    entries: Vec<Entry>,
    mode: Mode,
    sealed: bool,
    coordinator_exited: bool,
}
pub(crate) struct Monitor {
    state: Arc<Mutex<State>>,
    global: Arc<StopEvent>,
    coordinator: Arc<OwnedHandle>,
    sources: Arc<super::super::source_stop::SourceStops>,
}
pub(crate) struct Context {
    pub sources: Arc<super::super::source_stop::SourceStops>,
}
impl Monitor {
    pub fn start(
        tree: ProcessTree,
        image: VerifiedImage,
        coordinator: OwnedHandle,
        global: Arc<StopEvent>,
        pipes: Pipes,
        probe: bool,
        context: Context,
    ) -> Result<Self, &'static str> {
        let state = Arc::new(Mutex::new(State {
            registry: Registry::new(random()?, image.digest()),
            tree,
            image,
            entries: Vec::new(),
            mode: if probe { Mode::Probe } else { Mode::Native },
            sealed: false,
            coordinator_exited: false,
        }));
        let coordinator = Arc::new(coordinator);
        let retained_coordinator = coordinator.clone();
        let shared = state.clone();
        let stop = global.clone();
        let sources = context.sources;
        let source_stops = sources.clone();
        thread::spawn(move || {
            let mut input = pipes.receive;
            let mut output = pipes.send;
            drop(pipes.request);
            drop(pipes.response);
            drop(pipes.urgent_sender);
            let mut sequence = 0;
            let result: Result<(), &'static str> = (|| {
                loop {
                    let request: Message = read(&mut input)?;
                    if request.sequence != sequence {
                        return Err("custody-replay");
                    }
                    sequence = sequence
                        .checked_add(1)
                        .ok_or("custody-sequence-exhausted")?;
                    let answer =
                        handle(&shared, &coordinator, &stop, &source_stops, request.command);
                    let result = match answer {
                        Ok(answer) => answer,
                        Err(_) => Answer::Refused {},
                    };
                    let reply = Reply {
                        sequence: request.sequence,
                        result,
                    };
                    let bytes = encode(&reply)?;
                    if let Answer::Spawned { ticket, .. } = &reply.result {
                        // Hold the unpublished handoff through the complete write.
                        // Urgent Stop uses an event and does not need this mutex.
                        let mut state = shared.lock().map_err(|_| "custody-state")?;
                        let entry = state
                            .entries
                            .iter_mut()
                            .find(|entry| entry.ticket == *ticket)
                            .ok_or("custody-binding")?;
                        entry
                            .handoff
                            .as_mut()
                            .ok_or("custody-handoff")?
                            .publish(&mut output, &bytes)?;
                    } else {
                        write_bounded(&mut output, &bytes, MAX).map_err(|_| "custody-pipe")?;
                    }
                }
            })();
            if result.is_err() {
                let _ = stop.revoke();
            }
            if let Ok(mut state) = shared.lock() {
                state.registry.close_admission();
                state.sealed = true;
                for entry in &mut state.entries {
                    let _ = entry.stop.revoke();
                    drop(entry.arm.take());
                    drop(entry.input.take());
                }
            }
        });
        Ok(Self {
            state,
            global,
            coordinator: retained_coordinator,
            sources,
        })
    }
    pub fn close(&self) -> Result<(), &'static str> {
        let mut failed = self.global.revoke().is_err();
        failed |= self.sources.close().is_err();
        if unsafe { WaitForSingleObject(self.coordinator.as_raw_handle(), 0) } != WAIT_OBJECT_0 {
            return Err("custody-coordinator-alive");
        }
        let mut s = self.state.lock().map_err(|_| "custody-state")?;
        s.coordinator_exited = true;
        s.registry.coordinator_exited();
        s.tree.close_admission();
        s.sealed = true;
        for e in &mut s.entries {
            if let Some(handoff) = &mut e.handoff {
                handoff.coordinator_exited();
            }
            failed |= e.stop.revoke().is_err();
            drop(e.arm.take());
            drop(e.input.take());
        }
        if failed {
            Err("custody-stop-failed")
        } else {
            Ok(())
        }
    }
    pub fn processes_exited(&self) -> Result<bool, &'static str> {
        self.state
            .lock()
            .map_err(|_| "custody-state")?
            .tree
            .exited_after_admission_closed()
    }
    pub fn complete(&self) -> Result<bool, &'static str> {
        let mut s = self.state.lock().map_err(|_| "custody-state")?;
        for i in 0..s.entries.len() {
            poll(&mut s, i)?;
        }
        Ok(s.sealed
            && s.coordinator_exited
            && s.registry.sealed_entries_complete()
            && s.entries.iter().all(|entry| {
                entry
                    .handoff
                    .as_ref()
                    .is_none_or(Handoff::permits_retirement)
            }))
    }
}
fn transfer(coordinator: &OwnedHandle, raw: HANDLE, rights: u32) -> Result<usize, &'static str> {
    let mut out = ptr::null_mut();
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            raw,
            coordinator.as_raw_handle(),
            &mut out,
            rights,
            0,
            0,
        )
    } == 0
    {
        return Err("custody-transfer");
    }
    Ok(out as usize)
}
struct Transfers<'a> {
    coordinator: &'a OwnedHandle,
    unpublished: Vec<usize>,
}
impl Transfers<'_> {
    fn add(&mut self, raw: HANDLE, rights: u32) -> Result<usize, &'static str> {
        let handle = transfer(self.coordinator, raw, rights)?;
        self.unpublished.push(handle);
        Ok(handle)
    }
}
impl Drop for Transfers<'_> {
    fn drop(&mut self) {
        // Only handles never published to the coordinator may be reclaimed by
        // numeric value. After handoff it may close/reuse them independently.
        for raw in self.unpublished.drain(..) {
            let mut closed = ptr::null_mut();
            if unsafe {
                DuplicateHandle(
                    self.coordinator.as_raw_handle(),
                    raw as HANDLE,
                    GetCurrentProcess(),
                    &mut closed,
                    0,
                    0,
                    DUPLICATE_CLOSE_SOURCE,
                )
            } != 0
                && !closed.is_null()
            {
                drop(unsafe { OwnedHandle::from_raw_handle(closed) });
            }
        }
    }
}
fn from_coordinator(
    coordinator: &OwnedHandle,
    raw: usize,
    rights: u32,
) -> Result<OwnedHandle, &'static str> {
    if raw == 0 || raw == usize::MAX {
        return Err("custody-handle");
    }
    let mut out = ptr::null_mut();
    if unsafe {
        DuplicateHandle(
            coordinator.as_raw_handle(),
            raw as HANDLE,
            GetCurrentProcess(),
            &mut out,
            rights,
            0,
            0,
        )
    } == 0
    {
        return Err("custody-duplicate");
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(out) })
}
fn poll(state: &mut State, index: usize) -> Result<(), &'static str> {
    let entry = &mut state.entries[index];
    if entry.failed {
        return Err("custody-cleanup-unproven");
    }
    if entry.identity.is_none() {
        let identity = ProcessIdentity {
            pid: unsafe { GetProcessId(entry.process.as_raw_handle()) },
            created: unsafe { super::super::parent::creation_time(entry.process.as_raw_handle())? },
            image: state.image.digest(),
        };
        state.registry.bind(&entry.ticket, identity)?;
        entry.identity = Some(identity);
    }
    if entry.armed && !entry.released {
        match entry.terminal.poll() {
            Ok(()) => {
                state.registry.child_released(&entry.ticket, state.mode)?;
                entry.released = true;
            }
            Err("executor-cleanup-pending") => (),
            Err(_) => {
                entry.failed = true;
                let _ = entry.stop.revoke();
                return Err("custody-cleanup-unproven");
            }
        }
    }
    if !entry.exited
        && unsafe { WaitForSingleObject(entry.process.as_raw_handle(), 0) } == WAIT_OBJECT_0
    {
        entry.exited = true;
        state
            .registry
            .child_exited(&entry.ticket, entry.identity.ok_or("custody-process")?)?;
    }
    if state.registry.completion(&entry.ticket)?.is_some()
        && entry
            .handoff
            .as_ref()
            .is_none_or(Handoff::permits_retirement)
    {
        drop(entry._owner.take());
    }
    Ok(())
}
fn handle(
    shared: &Mutex<State>,
    coordinator: &OwnedHandle,
    global: &StopEvent,
    sources: &super::super::source_stop::SourceStops,
    request: Request,
) -> Result<Answer, &'static str> {
    if matches!(request, Request::Seal {}) {
        global.revoke()?;
    }
    let mut state = shared.lock().map_err(|_| "custody-state")?;
    match request {
        Request::Source { key } => {
            if unsafe { WaitForSingleObject(global.handle_for_query(), 0) } != WAIT_TIMEOUT {
                return Err("custody-stopped");
            }
            Ok(Answer::Source {
                stop: sources.event(&key)?.raw(),
                key,
            })
        }
        Request::Spawn {
            registration,
            tag,
            owner,
            stop,
        } => {
            if state.sealed
                || tag == 0
                || (!cfg!(feature = "native-input") && state.mode == Mode::Native)
            {
                return Err("custody-disabled");
            }
            if unsafe { WaitForSingleObject(global.handle_for_query(), 0) } != WAIT_TIMEOUT {
                return Err("custody-stopped");
            }
            for index in 0..state.entries.len() {
                poll(&mut state, index)?;
            }
            if state.entries.iter().any(|entry| {
                entry
                    .handoff
                    .as_ref()
                    .is_some_and(|handoff| !handoff.permits_retirement())
            }) {
                return Err("custody-handoff-pending");
            }
            registration.source.point(0., 0.)?;
            let source_event = match (&registration.source_key, state.mode) {
                (Some(key), _) => Some(sources.event(key)?),
                (None, Mode::Probe) => None,
                _ => return Err("source-required"),
            };
            let nonce = random()?;
            let mode = state.mode;
            let ticket = state.registry.reserve(
                &registration.session,
                registration.slot,
                registration.execution,
                nonce,
                mode,
            )?;
            let prepared = (|| {
                if stop == owner {
                    return Err("custody-handle-role");
                }
                // Own each duplicate immediately, including concurrent Stop and
                // failed owner duplication paths. No leaked input-slot lifetime.
                let stop_copy = from_coordinator(
                    coordinator,
                    stop,
                    EVENT_MODIFY_STATE | SYNCHRONIZATION_SYNCHRONIZE,
                )?;
                let owner_copy = if mode == Mode::Probe && owner == 0 {
                    None
                } else {
                    Some(from_coordinator(
                        coordinator,
                        owner,
                        SYNCHRONIZATION_SYNCHRONIZE,
                    )?)
                };
                let stop = Arc::new(StopEvent::from_owned(stop_copy)?);
                let (arm_read, arm_write) = super::super::process_tree::custody_pipe()?;
                clear_inherit(&arm_write)?;
                let coor = duplicate(
                    coordinator.as_raw_handle(),
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                    true,
                )?;
                let global_read = global.reader()?;
                let source_read = source_event
                    .as_ref()
                    .map(|event| event.reader())
                    .transpose()?;
                Ok((
                    stop,
                    owner_copy,
                    arm_read,
                    arm_write,
                    coor,
                    global_read,
                    source_read,
                ))
            })();
            let (stop, owner_copy, arm_read, arm_write, coor, global_read, source_read) =
                match prepared {
                    Ok(value) => value,
                    Err(error) => {
                        state.registry.stop(&ticket)?;
                        return Err(error);
                    }
                };
            state.registry.begin_spawn(&ticket)?;
            let mut inherited = vec![
                arm_read.as_handle(),
                coor.as_handle(),
                global_read.as_handle(),
            ];
            if let Some(source) = &source_read {
                inherited.push(source.as_handle());
            }
            let launched = {
                let s = &mut *state;
                s.tree.launch_custody(
                    &s.image,
                    &[
                        "--owned-executor".into(),
                        std::process::id().to_string().into(),
                        if mode == Mode::Probe {
                            "probe".into()
                        } else {
                            "native".into()
                        },
                    ],
                    &inherited,
                )
            };
            let (child, error) = match launched {
                Ok(c) => (c, None),
                Err(e) => match e.child {
                    Some(c) => (c, Some(e.error)),
                    None => {
                        state.registry.never_started(&ticket)?;
                        return Err(e.error);
                    }
                },
            };
            let (process, input, output, error_pipe) = child.into_parts();
            let pid = unsafe { GetProcessId(process.as_raw_handle()) };
            let (send, receive) = mpsc::sync_channel(1);
            let index = state.entries.len();
            state.entries.push(Entry {
                ticket: ticket.clone(),
                process,
                input: Some(input),
                output: Some(output),
                arm: Some(File::from(arm_write)),
                stop: stop.clone(),
                terminal: TerminalOwner {
                    nonce: URL_SAFE_NO_PAD.encode(nonce),
                    tag,
                    receive,
                    child: pid,
                    confirmed: false,
                    failed: false,
                    expected_mode: mode,
                },
                identity: None,
                _owner: owner_copy,
                mirror_revision: 0,
                mirror_complete: false,
                armed: false,
                released: false,
                exited: false,
                failed: false,
                handoff: None,
            });
            thread::spawn(move || {
                let _ = send.send(read_terminal(error_pipe));
            });
            if let Some(error) = error {
                stop.revoke()?;
                state.registry.stop(&ticket)?;
                drop(state.entries[index].arm.take());
                return Err(error);
            }
            let spec = Bootstrap {
                ticket: ticket.clone(),
                tag,
                nonce: URL_SAFE_NO_PAD.encode(nonce),
                owner: state.entries[index]
                    ._owner
                    .as_ref()
                    .map_or(0, |h| h.as_raw_handle() as usize),
                stop: stop.raw(),
                coordinator: coor.as_raw_handle() as usize,
                coordinator_pid: unsafe { GetProcessId(coor.as_raw_handle()) },
                coordinator_created: unsafe {
                    super::super::parent::creation_time(coor.as_raw_handle())?
                },
                global: global_read.as_raw_handle() as usize,
                arm: arm_read.as_raw_handle() as usize,
                source: registration.source,
                source_stop: source_read
                    .as_ref()
                    .map_or(0, |source| source.as_raw_handle() as usize),
            };
            // Publish the original owner before bootstrap/query/transfer can fail.
            let result = (|| {
                write(
                    state.entries[index].input.as_mut().ok_or("custody-pipe")?,
                    &spec,
                )?;
                poll(&mut state, index)?;
                let entry = &mut state.entries[index];
                let identity = entry.identity.ok_or("custody-process")?;
                let mut transfers = Transfers {
                    coordinator,
                    unpublished: Vec::new(),
                };
                let process = transfers.add(
                    entry.process.as_raw_handle(),
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                )?;
                let input = transfers.add(
                    entry.input.as_ref().ok_or("custody-pipe")?.as_raw_handle(),
                    windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE,
                )?;
                let output = transfers.add(
                    entry.output.as_ref().ok_or("custody-pipe")?.as_raw_handle(),
                    windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ,
                )?;
                drop(entry.input.take());
                drop(entry.output.take());
                entry.handoff = Some(Handoff {
                    handles: std::mem::take(&mut transfers.unpublished),
                    phase: Publication::Prepared,
                });
                Ok(Answer::Spawned {
                    ticket,
                    process,
                    input,
                    output,
                    pid,
                    created: identity.created,
                })
            })();
            if result.is_err() {
                stop.revoke()?;
                let failed_ticket = state.entries[index].ticket.clone();
                state.registry.stop(&failed_ticket)?;
                drop(state.entries[index].arm.take());
                drop(state.entries[index].input.take());
            }
            result
        }
        Request::Arm { ticket } => {
            let index = state
                .entries
                .iter()
                .position(|e| e.ticket == ticket)
                .ok_or("custody-binding")?;
            if state.entries[index]
                .handoff
                .as_ref()
                .is_none_or(|handoff| handoff.phase != Publication::Published)
            {
                return Err("custody-handoff");
            }
            state.entries[index]
                .handoff
                .as_mut()
                .ok_or("custody-handoff")?
                .adopted()?;
            if state.sealed
                || unsafe { WaitForSingleObject(global.handle_for_query(), 0) } != WAIT_TIMEOUT
                || state.entries[index].stop.signal_failed()
                || unsafe { WaitForSingleObject(state.entries[index].stop.handle_for_query(), 0) }
                    != WAIT_TIMEOUT
            {
                state.registry.stop(&ticket)?;
                let entry = &mut state.entries[index];
                let signal = entry.stop.revoke();
                drop(entry.arm.take());
                drop(entry.input.take());
                signal?;
                return Err("custody-closed");
            }
            state.registry.arm(&ticket)?;
            state.entries[index].armed = true;
            write(
                state.entries[index].arm.as_mut().ok_or("custody-arm")?,
                &ticket,
            )?;
            drop(state.entries[index].arm.take());
            Ok(Answer::Done {})
        }
        Request::Observe { ticket } => {
            let index = state
                .entries
                .iter()
                .position(|e| e.ticket == ticket)
                .ok_or("custody-binding")?;
            poll(&mut state, index)?;
            let e = &state.entries[index];
            Ok(Answer::Observed {
                released: e.released,
                exited: e.exited,
                // Positive no-ARM evidence, not an input cleanup receipt. ARM is
                // ledgered before its pipe write, and the child cannot run any
                // primitive before consuming it. Never applies to armed entries.
                never_armed: !e.armed && e.exited && state.registry.input_settled(&ticket)?,
            })
        }
        Request::Mirror {
            ticket,
            revision,
            held,
        } => {
            let index = state
                .entries
                .iter()
                .position(|e| e.ticket == ticket)
                .ok_or("custody-binding")?;
            if held.len() > 512
                || held.values().any(|v| !*v)
                || revision
                    != state.entries[index]
                        .mirror_revision
                        .checked_add(1)
                        .ok_or("custody-replay")?
            {
                state.entries[index].failed = true;
                return Err("custody-mirror");
            }
            poll(&mut state, index)?;
            state.entries[index].mirror_revision = revision;
            if state.entries[index].mirror_complete && !held.is_empty() {
                state.entries[index].failed = true;
                state.entries[index].stop.revoke()?;
                return Err("custody-mirror-reopened");
            }
            if !state.entries[index].mirror_complete
                && held.is_empty()
                && state.entries[index].exited
                && state.entries[index].released
            {
                if state.entries[index].stop.signal_failed() {
                    state.entries[index].failed = true;
                    return Err("custody-stop-failed");
                }
                let mode = state.mode;
                state.registry.mirror_released(&ticket, mode)?;
                state.entries[index].mirror_complete = true;
                // Keep replay tombstones and original process evidence, but a
                // positively settled generation must release the shared slot.
                if state.entries[index]
                    .handoff
                    .as_ref()
                    .is_none_or(Handoff::permits_retirement)
                {
                    drop(state.entries[index]._owner.take());
                }
            }
            Ok(Answer::Done {})
        }
        Request::Seal {} => {
            state.registry.close_admission();
            state.sealed = true;
            let mut failed = false;
            for e in &mut state.entries {
                failed |= e.stop.revoke().is_err();
                drop(e.arm.take());
                drop(e.input.take());
            }
            if failed {
                Err("custody-stop-failed")
            } else {
                Ok(Answer::Done {})
            }
        }
    }
}

#[cfg(test)]
mod tests;
