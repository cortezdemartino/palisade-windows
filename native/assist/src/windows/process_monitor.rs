//! Direct application child retains the exact job through coordinator exit.
//! This stage starts only the existing non-authorizing dispatcher. Its proof
//! explicitly covers channel and process exit, NOT native input release.
use super::{
    image::{ImageManifest, VerifiedImage},
    parent::Parent,
    process_tree::{InheritedTree, ProcessTree, StopEvent},
};
use crate::{
    framing::{read_bounded, write_bounded},
    guard::Watchdog,
    live::{Command, Ingress, MAX_FRAME, OUTPUT_CAPACITY, PROTOCOL, Request, Signals},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    os::windows::io::{AsHandle, AsRawHandle},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{Foundation::*, System::Threading::*};
pub const SCOPE: &str = "channel-and-process-exit";
pub const ENDPOINT_SCOPE: &str = "endpoint-and-process-exit";
// A local process may preempt this name to deny service, never to grant any
// authority. The exact validated application object supplies PID + creation.
struct EndpointOwner {
    _handle: std::os::windows::io::OwnedHandle,
}
impl EndpointOwner {
    fn reserve(parent: &Parent) -> Result<Self, &'static str> {
        use std::os::windows::io::FromRawHandle;
        let name: Vec<u16> = format!(
            "Local\\Palisade.Assist.Endpoint.v3.{}.{}",
            parent.pid(),
            parent.creation_time()?
        )
        .encode_utf16()
        .chain(Some(0))
        .collect();
        unsafe {
            SetLastError(0);
            let handle = CreateEventW(std::ptr::null(), 1, 0, name.as_ptr());
            if handle.is_null() {
                return Err("endpoint-owner-unavailable");
            }
            let owner = Self {
                _handle: std::os::windows::io::OwnedHandle::from_raw_handle(handle),
            };
            if GetLastError() == ERROR_ALREADY_EXISTS {
                return Err("endpoint-already-running");
            }
            Ok(owner)
        }
    }
}
struct RevokeOnDrop(Arc<StopEvent>);
impl Drop for RevokeOnDrop {
    fn drop(&mut self) {
        let _ = self.0.revoke();
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    sequence: u32,
    result: ResultKind,
}
#[derive(Deserialize, Serialize)]
#[serde(tag = "type", deny_unknown_fields, rename_all = "kebab-case")]
enum ResultKind {
    CaptureSetup {
        enabled: bool,
    },
    CaptureSource {
        accepted: bool,
    },
    LocalIce {
        candidates: Vec<crate::capture::LocalCandidate>,
    },
    Native {
        document: u32,
        answer: crate::endpoint::Answer,
    },
    Hello {
        protocol: String,
        version: u32,
        challenge: String,
        mode: String,
        #[serde(rename = "nativeInput")]
        native_input: bool,
        #[serde(rename = "cleanupScope")]
        cleanup_scope: String,
    },
    Pulse {},
    Status {
        state: String,
        authority: String,
    },
    ProbeWait {},
    ProbeDescendant {},
    ProbeStopHolder {
        pid: u32,
        coordinator: u32,
    },
    ProbeExecutor {
        pid: u32,
        coordinator: u32,
        released: bool,
    },
    ProbeExecutorCancel {
        pid: u32,
        coordinator: u32,
    },
    CleanupComplete {
        challenge: String,
        scope: String,
    },
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    CaptureSetup,
    CaptureSource,
    InspectLocalIce,
    SourceStop,
    Native,
    Hello,
    Pulse,
    Status,
    ProbeWait,
    ProbeDescendant,
    ProbeStopHolder,
    ProbeExecutor(bool),
    ProbeExecutorCancel,
    Shutdown,
}
impl Kind {
    fn of(command: &Command) -> Self {
        match command {
            Command::CaptureSetup { .. } => Self::CaptureSetup,
            Command::CaptureSource { .. } => Self::CaptureSource,
            Command::InspectLocalIce { .. } => Self::InspectLocalIce,
            Command::Native {
                operation:
                    crate::endpoint::Operation::CaptureStop { .. }
                    | crate::endpoint::Operation::CaptureCancel { .. }
                    | crate::endpoint::Operation::PeerClose { .. },
                ..
            } => Self::SourceStop,
            Command::Native { .. } => Self::Native,
            Command::Hello { .. } => Self::Hello,
            Command::Pulse {} => Self::Pulse,
            Command::Status {} => Self::Status,
            Command::ProbeWait { .. } => Self::ProbeWait,
            Command::ProbeDescendant { .. } => Self::ProbeDescendant,
            Command::ProbeStopHolder {} => Self::ProbeStopHolder,
            Command::ProbeExecutor { release } => Self::ProbeExecutor(*release),
            Command::ProbeExecutorCancel {} => Self::ProbeExecutorCancel,
            Command::Shutdown {} => Self::Shutdown,
        }
    }
}
#[derive(Default)]
struct State {
    pending: BTreeMap<u32, Kind>,
    starting: BTreeMap<u32, super::source_stop::SourceKey>,
    challenge: Option<String>,
    terminal: Option<Answer>,
}
fn fail(signals: &Signals, stop: &StopEvent) {
    signals.fail();
    let _ = stop.revoke();
}
fn wait_code(process: &std::os::windows::io::OwnedHandle) -> Result<Option<u32>, &'static str> {
    match unsafe { WaitForSingleObject(process.as_raw_handle(), 0) } {
        WAIT_TIMEOUT => Ok(None),
        WAIT_OBJECT_0 => {
            let mut code = 0;
            if unsafe { GetExitCodeProcess(process.as_raw_handle(), &mut code) } == 0 {
                return Err("monitor-process-query");
            }
            Ok(Some(code))
        }
        _ => Err("monitor-process-query"),
    }
}
fn validated(
    state: &mut State,
    mut answer: Answer,
    probe: bool,
    endpoint: bool,
) -> Result<Option<Answer>, &'static str> {
    if state.terminal.is_some() {
        return Err("monitor-trailing-output");
    }
    let kind = state
        .pending
        .remove(&answer.sequence)
        .ok_or("monitor-correlation")?;
    let valid = match (&kind, &mut answer.result) {
        (Kind::CaptureSetup, ResultKind::CaptureSetup { .. })
        | (Kind::CaptureSource, ResultKind::CaptureSource { .. })
        | (Kind::InspectLocalIce, ResultKind::LocalIce { .. }) => endpoint,
        (
            Kind::Hello,
            ResultKind::Hello {
                protocol,
                version,
                challenge,
                mode,
                native_input,
                cleanup_scope,
            },
        ) => {
            let valid = protocol == PROTOCOL
                && *version == if endpoint { 3 } else { 2 }
                && Some(challenge.as_str()) == state.challenge.as_deref()
                && mode
                    == if endpoint {
                        "endpoint-coordinator"
                    } else {
                        "tree-coordinator"
                    }
                && !*native_input
                && cleanup_scope
                    == if endpoint {
                        "endpoint-only"
                    } else {
                        "channel-only"
                    };
            *mode = if endpoint && probe {
                "tree-endpoint-probe"
            } else if endpoint {
                "tree-endpoint"
            } else if probe {
                "tree-live-probe"
            } else {
                "tree-live"
            }
            .into();
            *cleanup_scope = if endpoint { ENDPOINT_SCOPE } else { SCOPE }.into();
            valid
        }
        (Kind::Pulse, ResultKind::Pulse {})
        | (Kind::ProbeWait, ResultKind::ProbeWait {})
        | (Kind::ProbeDescendant, ResultKind::ProbeDescendant {}) => true,
        (Kind::Native | Kind::SourceStop, ResultKind::Native { .. }) => endpoint,
        (Kind::ProbeStopHolder, ResultKind::ProbeStopHolder { pid, coordinator }) => {
            *pid > 0 && *coordinator > 0 && pid != coordinator
        }
        (
            Kind::ProbeExecutor(expected),
            ResultKind::ProbeExecutor {
                pid,
                coordinator,
                released,
            },
        ) => {
            probe
                && endpoint
                && expected == released
                && *pid > 0
                && *coordinator > 0
                && pid != coordinator
        }
        (Kind::ProbeExecutorCancel, ResultKind::ProbeExecutorCancel { pid, coordinator }) => {
            probe && endpoint && *pid > 0 && *coordinator > 0 && pid != coordinator
        }
        (Kind::Status, ResultKind::Status { state, authority }) => {
            ["inactive", "revoked"].contains(&state.as_str()) && authority == "none"
        }
        (Kind::Shutdown, ResultKind::CleanupComplete { challenge, scope }) => {
            Some(challenge.as_str()) == state.challenge.as_deref()
                && scope
                    == if endpoint {
                        "endpoint-only"
                    } else {
                        "channel-only"
                    }
        }
        _ => false,
    };
    if !valid {
        return Err("monitor-response");
    }
    if kind == Kind::Shutdown {
        state.terminal = Some(answer);
        Ok(None)
    } else {
        Ok(Some(answer))
    }
}
pub fn coordinator(parent: u32, probe: bool, job: usize, stop: usize) -> Result<(), &'static str> {
    // Validate the creator before adopting any inherited handle.
    let _parent = Parent::validate_executor(parent, probe)?;
    super::live::run_owned(parent, probe, InheritedTree::open(job, stop)?)
}
pub fn endpoint_coordinator(
    parent: u32,
    probe: bool,
    job: usize,
    stop: usize,
    request: usize,
    response: usize,
    urgent: usize,
) -> Result<(), &'static str> {
    let verified_parent = Parent::validate_executor(parent, probe)?;
    let roles = [job, stop, request, response, urgent];
    if roles
        .iter()
        .enumerate()
        .any(|(i, h)| *h == 0 || roles[..i].contains(h))
    {
        return Err("tree-handles");
    }
    let custody = unsafe {
        super::process_input::custody::Client::inherited(
            request,
            response,
            urgent,
            verified_parent,
            probe,
        )?
    };
    super::live::run_endpoint(parent, probe, InheritedTree::open(job, stop)?, custody)
}
pub fn release_probe(parent: u32, milliseconds: u16) -> Result<(), &'static str> {
    let parent = Parent::validate_executor(parent, true)?;
    if milliseconds == 0 || milliseconds > 2000 {
        return Err("invalid-probe");
    }
    crate::framing::write_frame(&mut std::io::stdout().lock(), b"tree-release-probe-ready")
        .map_err(|_| "probe-pipe")?;
    let started = Instant::now();
    while parent.alive() {
        if started.elapsed() >= Duration::from_secs(5) {
            return Err("probe-parent-timeout");
        }
        thread::sleep(Duration::from_millis(5));
    }
    thread::sleep(Duration::from_millis(u64::from(milliseconds)));
    Ok(())
}
pub fn stop_probe(parent: u32) -> Result<(), &'static str> {
    let _parent = Parent::validate_executor(parent, true)?;
    let event = unsafe {
        windows_sys::Win32::System::Console::GetStdHandle(
            windows_sys::Win32::System::Console::STD_ERROR_HANDLE,
        )
    };
    if event.is_null() || event == INVALID_HANDLE_VALUE {
        return Err("probe-stop-event");
    }
    if unsafe { SetEvent(event) } != 0 {
        return Err("probe-stop-rights");
    }
    crate::framing::write_frame(&mut std::io::stdout().lock(), b"tree-stop-probe-ready")
        .map_err(|_| "probe-pipe")?;
    // Intentionally event-only: a parent-death watchdog would hide a broken
    // monitor that waited for job zero before signaling survivors.
    if unsafe { WaitForSingleObject(event, 6000) } != WAIT_OBJECT_0 {
        return Err("probe-stop-timeout");
    }
    Ok(())
}
pub fn run(parent_pid: u32, probe: bool, manifest: ImageManifest) -> Result<(), &'static str> {
    run_inner(parent_pid, probe, manifest, false)
}
pub fn run_endpoint(
    parent_pid: u32,
    probe: bool,
    manifest: ImageManifest,
) -> Result<(), &'static str> {
    run_inner(parent_pid, probe, manifest, true)
}
fn run_inner(
    parent_pid: u32,
    probe: bool,
    manifest: ImageManifest,
    endpoint: bool,
) -> Result<(), &'static str> {
    let parent = Parent::validate(parent_pid, probe)?;
    let _endpoint_owner = endpoint
        .then(|| EndpointOwner::reserve(&parent))
        .transpose()?;
    let image = VerifiedImage::open(&manifest)?;
    let mut tree = ProcessTree::new()?;
    let query = tree.query_handle()?;
    let stop = Arc::new(StopEvent::new()?);
    let _stop_on_all_exit_paths = RevokeOnDrop(stop.clone());
    // Establish the one mandatory Stop surface/hotkey before creating even an
    // inactive coordinator. A later authority bootstrap must also prove receipt
    // registrar readiness; this ordering alone is not an authorization proof.
    let native_flag = Arc::new(AtomicBool::new(false));
    let _native_stop = if probe {
        None
    } else {
        Some(super::lifecycle::NativeStop::start(
            native_flag.clone(),
            false,
        )?)
    };
    if native_flag.load(Ordering::SeqCst) {
        return Err("native-stop-unavailable");
    }
    let child_stop = stop.reader()?;
    let custody_pipes = endpoint
        .then(super::process_input::custody::Pipes::new)
        .transpose()?;
    let mut args: Vec<std::ffi::OsString> = vec![
        if endpoint {
            "--endpoint-coordinator"
        } else {
            "--tree-coordinator"
        }
        .into(),
        std::process::id().to_string().into(),
        (query.as_raw_handle() as usize).to_string().into(),
        (child_stop.as_raw_handle() as usize).to_string().into(),
        if probe { "probe" } else { "normal" }.into(),
    ];
    let urgent = custody_pipes.as_ref().map(|pipes| pipes.urgent.clone());
    let mut extra = vec![query.as_handle(), child_stop.as_handle()];
    if let Some(pipes) = &custody_pipes {
        args.extend([
            (pipes.request.as_raw_handle() as usize).to_string().into(),
            (pipes.response.as_raw_handle() as usize).to_string().into(),
            (pipes.urgent_sender.as_raw_handle() as usize)
                .to_string()
                .into(),
        ]);
        extra.extend([
            pipes.request.as_handle(),
            pipes.response.as_handle(),
            pipes.urgent_sender.as_handle(),
        ]);
    }
    let child = tree.launch(&image, &args, &extra)?;
    drop(extra);
    drop(query);
    drop(child_stop);
    let process = child
        .process_handle()
        .try_clone_to_owned()
        .map_err(|_| "monitor-process-handle")?;
    let mut tree = Some(tree);
    let sources = Arc::new(super::source_stop::SourceStops::default());
    let application = parent.query_handle()?;
    let custody = custody_pipes
        .map(|pipes| {
            super::process_input::custody::Monitor::start(
                tree.take().ok_or("custody-tree")?,
                image,
                process.try_clone().map_err(|_| "custody-coordinator")?,
                stop.clone(),
                pipes,
                probe,
                super::process_input::custody::Context {
                    sources: sources.clone(),
                },
            )
        })
        .transpose()?;
    let signals = Arc::new(Signals::default());
    let finished = Arc::new(AtomicBool::new(false));
    let state = Arc::new(Mutex::new(State::default()));
    let capture_enabled = Arc::new(AtomicBool::new(false));
    let capacity = if endpoint {
        crate::live::ENDPOINT_OUTPUT_CAPACITY
    } else {
        OUTPUT_CAPACITY
    };
    let guard = Arc::new(Mutex::new(Watchdog::new(
        Arc::new(AtomicBool::new(false)),
        super::lease::now()?,
        Instant::now(),
    )));
    let (commands, incoming) = mpsc::sync_channel::<Vec<u8>>(capacity);
    let (outputs, outgoing) = mpsc::sync_channel::<(
        Answer,
        Option<mpsc::SyncSender<Result<(), &'static str>>>,
    )>(capacity);
    let (writer_signals, writer_stop) = (signals.clone(), stop.clone());
    let input_writer = thread::spawn(move || {
        let mut pipe = child.input;
        loop {
            if writer_signals.failed() {
                break;
            }
            let bytes = match incoming.recv_timeout(Duration::from_millis(10)) {
                Ok(bytes) => bytes,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            };
            if write_bounded(&mut pipe, &bytes, MAX_FRAME).is_err() {
                fail(&writer_signals, &writer_stop);
                break;
            }
        }
    });
    let (out_signals, out_stop) = (signals.clone(), stop.clone());
    let outer_writer = thread::spawn(move || {
        let mut pipe = std::io::stdout().lock();
        for (answer, ack) in outgoing {
            let result = serde_json::to_vec(&answer)
                .map_err(|_| "monitor-encoding")
                .and_then(|bytes| {
                    write_bounded(&mut pipe, &bytes, MAX_FRAME).map_err(|_| "monitor-output")
                });
            if result.is_err() {
                fail(&out_signals, &out_stop);
            }
            if let Some(ack) = ack {
                let _ = ack.send(result);
            }
            if result.is_err() {
                break;
            }
        }
    });
    let (read_signals, read_stop, read_state, read_guard) =
        (signals.clone(), stop.clone(), state.clone(), guard.clone());
    // Socket ownership queries must not share a worker with consent or custody.
    // Retain the verified application object; never accept a caller-supplied PID.
    let (inspection, inspect_receive) = mpsc::sync_channel::<(u32, Vec<u16>)>(1);
    let (inspect_signals, inspect_stop, inspect_state, inspect_output) = (
        signals.clone(),
        stop.clone(),
        state.clone(),
        outputs.clone(),
    );
    let inspector = thread::spawn(move || {
        while unsafe { WaitForSingleObject(inspect_stop.handle_for_query(), 0) } == WAIT_TIMEOUT {
            let (sequence, ports) = match inspect_receive.recv_timeout(Duration::from_millis(10)) {
                Ok(value) => value,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            };
            let outcome = (|| {
                if unsafe { WaitForSingleObject(application.as_raw_handle(), 0) } != WAIT_TIMEOUT {
                    return Err("application-ended");
                }
                let candidates = if probe {
                    Vec::new()
                } else {
                    super::local_ice::inspect(
                        unsafe { GetProcessId(application.as_raw_handle()) },
                        &ports,
                        false,
                    )?
                };
                if unsafe { WaitForSingleObject(inspect_stop.handle_for_query(), 0) }
                    != WAIT_TIMEOUT
                {
                    return Err("custody-stopped");
                }
                if unsafe { WaitForSingleObject(application.as_raw_handle(), 0) } != WAIT_TIMEOUT {
                    return Err("application-ended");
                }
                let answer = Answer {
                    sequence,
                    result: ResultKind::LocalIce { candidates },
                };
                let answer = validated(
                    &mut *inspect_state.lock().map_err(|_| "monitor-state")?,
                    answer,
                    probe,
                    endpoint,
                )?
                .ok_or("monitor-inspection")?;
                inspect_output
                    .try_send((answer, None))
                    .map_err(|_| "monitor-output-full")
            })();
            if outcome.is_err() {
                if unsafe { WaitForSingleObject(inspect_stop.handle_for_query(), 0) }
                    != WAIT_OBJECT_0
                {
                    fail(&inspect_signals, &inspect_stop);
                }
                break;
            }
        }
    });
    let read_sources = sources.clone();
    let read_capture = capture_enabled.clone();
    thread::spawn(move || {
        let mut ingress = if endpoint {
            Ingress::endpoint(probe)
        } else {
            Ingress::owned(probe)
        };
        let mut pipe = std::io::stdin().lock();
        let result = (|| {
            loop {
                let bytes = read_bounded(&mut pipe, MAX_FRAME)
                    .map_err(|_| "monitor-input")?
                    .ok_or("monitor-eof")?;
                let request: Request = ingress.accept(&bytes, &read_signals)?;
                let kind = Kind::of(&request.command);
                if kind == Kind::Shutdown {
                    read_stop.revoke()?;
                }
                {
                    let mut state = read_state.lock().map_err(|_| "monitor-state")?;
                    if state.pending.len() >= capacity
                        || (endpoint
                            && kind != Kind::Shutdown
                            && state.pending.len() >= capacity - 1)
                    {
                        return Err("monitor-capacity");
                    }
                    if !matches!(
                        kind,
                        Kind::Hello
                            | Kind::Pulse
                            | Kind::Shutdown
                            | Kind::SourceStop
                            | Kind::InspectLocalIce
                    ) && state
                        .pending
                        .values()
                        .filter(|k| {
                            !matches!(
                                k,
                                Kind::Hello
                                    | Kind::Pulse
                                    | Kind::Shutdown
                                    | Kind::SourceStop
                                    | Kind::InspectLocalIce
                            )
                        })
                        .count()
                        >= crate::live::ORDINARY_CAPACITY
                    {
                        return Err("monitor-capacity");
                    }
                    if let Command::Hello { challenge, .. } = &request.command {
                        state.challenge = Some(challenge.clone());
                        read_guard
                            .lock()
                            .map_err(|_| "monitor-watchdog")?
                            .handshake(Instant::now())?;
                    }
                    state.pending.insert(request.sequence, kind);
                    match &request.command {
                        Command::Native {
                            document,
                            operation: crate::endpoint::Operation::CaptureStart { operation, .. },
                        } if read_capture.load(Ordering::SeqCst) => {
                            let key = super::source_stop::SourceKey {
                                document: *document,
                                operation: operation.clone(),
                            };
                            if read_sources.begin(&key).is_ok() {
                                state.starting.insert(request.sequence, key);
                            }
                        }
                        Command::Native {
                            document,
                            operation:
                                crate::endpoint::Operation::CaptureStop { operation }
                                | crate::endpoint::Operation::CaptureCancel { operation },
                        } => {
                            read_sources.stop(&super::source_stop::SourceKey {
                                document: *document,
                                operation: operation.clone(),
                            })?;
                        }
                        _ => (),
                    }
                }
                if kind == Kind::Pulse {
                    read_guard
                        .lock()
                        .map_err(|_| "monitor-watchdog")?
                        .touch(Instant::now());
                }
                if let Command::InspectLocalIce { ports } = request.command {
                    if ports.is_empty()
                        || ports.len() > 64
                        || ports.contains(&0)
                        || ports
                            .iter()
                            .collect::<std::collections::BTreeSet<_>>()
                            .len()
                            != ports.len()
                    {
                        return Err("invalid-inspection");
                    }
                    inspection
                        .try_send((request.sequence, ports))
                        .map_err(|_| "monitor-inspection-full")?;
                    // The coordinator still validates the same ordered sequence,
                    // but its reader neither queues work nor sends a duplicate.
                    commands.try_send(bytes).map_err(|_| "monitor-input-full")?;
                    continue;
                }
                commands.try_send(bytes).map_err(|_| "monitor-input-full")?;
                if kind == Kind::Shutdown {
                    read_signals.shutdown();
                    return Ok::<(), &'static str>(());
                }
            }
        })();
        if result.is_err() {
            fail(&read_signals, &read_stop);
        }
    });
    let (watch_signals, watch_stop, watch_finished, watch_guard) = (
        signals.clone(),
        stop.clone(),
        finished.clone(),
        guard.clone(),
    );
    let watcher = thread::spawn(move || {
        while !watch_finished.load(Ordering::SeqCst) {
            if urgent.as_ref().is_some_and(
                |event| unsafe { WaitForSingleObject(event.handle_for_query(), 0) } != WAIT_TIMEOUT,
            ) {
                let _ = watch_stop.revoke();
            }
            if native_flag.load(Ordering::SeqCst) {
                fail(&watch_signals, &watch_stop);
            }
            if !watch_signals.closing()
                && super::lease::now()
                    .and_then(|wall| {
                        watch_guard
                            .lock()
                            .map_err(|_| "monitor-watchdog")
                            .map(|mut guard| guard.check(wall, Instant::now(), parent.alive()))
                    })
                    .unwrap_or(true)
            {
                fail(&watch_signals, &watch_stop);
            }
            thread::sleep(Duration::from_millis(10));
        }
    });
    let (response_signals, response_stop, response_state) =
        (signals.clone(), stop.clone(), state.clone());
    let response_writer = outputs.clone();
    let response_sources = sources.clone();
    let reader = thread::spawn(move || {
        let mut pipe = child.output;
        let result = (|| {
            while let Some(bytes) =
                read_bounded(&mut pipe, MAX_FRAME).map_err(|_| "monitor-child-frame")?
            {
                let answer: Answer =
                    serde_json::from_slice(&bytes).map_err(|_| "monitor-child-json")?;
                {
                    let mut state = response_state.lock().map_err(|_| "monitor-state")?;
                    if let Some(key) = state.starting.remove(&answer.sequence) {
                        if !matches!(
                            answer.result,
                            ResultKind::Native {
                                answer: crate::endpoint::Answer::Ok { .. },
                                ..
                            }
                        ) {
                            response_sources.stop(&key)?;
                        }
                    }
                    if let ResultKind::CaptureSetup { enabled } = &answer.result {
                        if state.pending.get(&answer.sequence) != Some(&Kind::CaptureSetup) {
                            return Err("monitor-correlation");
                        }
                        capture_enabled.store(*enabled && !probe, Ordering::SeqCst);
                    }
                }
                let answer = validated(
                    &mut *response_state.lock().map_err(|_| "monitor-state")?,
                    answer,
                    probe,
                    endpoint,
                )?;
                if let Some(answer) = answer {
                    response_writer
                        .try_send((answer, None))
                        .map_err(|_| "monitor-output-full")?;
                }
            }
            Ok::<(), &'static str>(())
        })();
        if result.is_err() {
            fail(&response_signals, &response_stop);
        }
        result
    });
    let error_reader = thread::spawn(move || {
        let mut sink = std::io::sink();
        let mut pipe = child.error;
        std::io::copy(&mut pipe, &mut sink)
    });
    while wait_code(&process)?.is_none() {
        if signals.failed() {
            let _ = stop.revoke();
        }
        thread::sleep(Duration::from_millis(10));
    }
    // Coordinator death closes authority before awaiting any survivor. Job
    // zero cannot itself wake an input holder that is waiting for this event.
    stop.revoke()?;
    inspector.join().map_err(|_| "monitor-inspection-join")?;
    if wait_code(&process)? != Some(0) {
        signals.fail();
    }
    if let Some(tree) = &mut tree {
        tree.close_admission();
    }
    if let Some(custody) = &custody {
        if custody.close().is_err() {
            signals.fail();
        }
    }
    // Admission is closed and the exact coordinator has exited. Zero here
    // includes every inherited descendant, including Windows console helpers.
    while if let Some(custody) = &custody {
        !custody.processes_exited()?
    } else {
        !tree
            .as_ref()
            .ok_or("monitor-tree")?
            .exited_after_admission_closed()?
    } {
        thread::sleep(Duration::from_millis(10));
    }
    if let Some(custody) = &custody {
        // Missing/invalid native evidence retains the monitor and exact original
        // handles. Job zero is not permission to discard an unsettled ledger.
        while !custody.complete().unwrap_or(false) {
            thread::sleep(Duration::from_millis(100));
        }
    }
    let read_result = reader.join().map_err(|_| "monitor-reader-join")?;
    input_writer.join().map_err(|_| "monitor-writer-join")?;
    error_reader
        .join()
        .map_err(|_| "monitor-error-join")?
        .map_err(|_| "monitor-error-pipe")?;
    finished.store(true, Ordering::SeqCst);
    let _ = stop.revoke();
    watcher.join().map_err(|_| "monitor-watchdog-join")?;
    if read_result.is_err() || signals.failed() || wait_code(&process)? != Some(0) {
        return Err("monitor-cleanup-unproven");
    }
    let mut terminal = state
        .lock()
        .map_err(|_| "monitor-state")?
        .terminal
        .take()
        .ok_or("monitor-cleanup-unproven")?;
    if let ResultKind::CleanupComplete { scope, .. } = &mut terminal.result {
        *scope = if endpoint { ENDPOINT_SCOPE } else { SCOPE }.into();
    } else {
        return Err("monitor-cleanup-unproven");
    }
    let (ack, delivered) = mpsc::sync_channel(1);
    outputs
        .try_send((terminal, Some(ack)))
        .map_err(|_| "monitor-output-full")?;
    delivered.recv().map_err(|_| "monitor-output")??;
    drop(outputs);
    outer_writer.join().map_err(|_| "monitor-output-join")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_terminal_cannot_be_substituted_with_old_channel_or_input_proof() {
        for scope in [
            "channel-only",
            "channel-and-process-exit",
            "input-cleanup",
            "endpoint-only",
        ] {
            let challenge = "A".repeat(43);
            let mut state = State {
                challenge: Some(challenge.clone()),
                ..State::default()
            };
            state.pending.insert(1, Kind::Shutdown);
            let result = validated(
                &mut state,
                Answer {
                    sequence: 1,
                    result: ResultKind::CleanupComplete {
                        challenge,
                        scope: scope.into(),
                    },
                },
                true,
                true,
            );
            assert_eq!(result.is_ok(), scope == "endpoint-only");
            assert_eq!(state.terminal.is_some(), scope == "endpoint-only");
        }
    }
    #[test]
    fn early_error_and_unwind_signal_stop_despite_other_arc_owners() {
        for panic in [false, true] {
            let stop = Arc::new(StopEvent::new().unwrap());
            let other = stop.clone();
            let reader = stop.reader().unwrap();
            assert_eq!(
                unsafe { WaitForSingleObject(reader.as_raw_handle(), 0) },
                WAIT_TIMEOUT
            );
            let outcome = crate::cleanup::attempt(|| {
                let _guard = RevokeOnDrop(stop.clone());
                if panic {
                    panic!("scope guard fixture");
                }
                Err("fixture-failure")
            });
            assert!(outcome.is_err());
            assert!(Arc::strong_count(&other) >= 2);
            assert_eq!(
                unsafe { WaitForSingleObject(reader.as_raw_handle(), 0) },
                WAIT_OBJECT_0
            );
        }
    }
    #[test]
    fn process_exit_scope_never_substitutes_for_input_cleanup_or_wrong_launch() {
        let nonce = "A".repeat(43);
        for (challenge, scope) in [
            (nonce.clone(), "input-cleanup"),
            ("B".repeat(43), "channel-only"),
        ] {
            let mut state = State {
                challenge: Some(nonce.clone()),
                ..State::default()
            };
            state.pending.insert(1, Kind::Shutdown);
            assert!(
                validated(
                    &mut state,
                    Answer {
                        sequence: 1,
                        result: ResultKind::CleanupComplete {
                            challenge,
                            scope: scope.into()
                        }
                    },
                    true,
                    false
                )
                .is_err()
            );
            assert!(state.terminal.is_none());
        }
        let mut state = State {
            challenge: Some(nonce.clone()),
            ..State::default()
        };
        state.pending.insert(1, Kind::Shutdown);
        assert!(
            validated(
                &mut state,
                Answer {
                    sequence: 1,
                    result: ResultKind::CleanupComplete {
                        challenge: nonce,
                        scope: "channel-only".into()
                    }
                },
                true,
                false
            )
            .unwrap()
            .is_none()
        );
        state.pending.insert(2, Kind::Pulse);
        assert!(
            validated(
                &mut state,
                Answer {
                    sequence: 2,
                    result: ResultKind::Pulse {}
                },
                true,
                false
            )
            .is_err()
        );
        for raw in [
            r#"{"sequence":0,"sequence":1,"result":{"type":"pulse"}}"#,
            r#"{"sequence":0,"result":{"type":"pulse","approved":true}}"#,
            r#"{"sequence":0,"result":{"type":"sign","data":"arbitrary"}}"#,
        ] {
            assert!(serde_json::from_str::<Answer>(raw).is_err());
        }
    }
}
