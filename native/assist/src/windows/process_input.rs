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
use serde::{Deserialize, Serialize};
use std::{
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
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
fn uptime() -> u64 {
    unsafe { GetTickCount64() }
}
pub struct ProcessInput {
    child: Child,
    pipe: ChildStdin,
    replies: mpsc::Receiver<Result<Ack, &'static str>>,
    sequence: u32,
    cleanup: WindowsBackend,
    fenced: bool,
    probe: bool,
    mirror: Ledger,
}
impl ProcessInput {
    pub fn spawn(
        tag: usize,
        probe: bool,
        manifest: &super::image::ImageManifest,
    ) -> Result<Self, &'static str> {
        if !probe && !cfg!(feature = "native-input") {
            return Err("native-input-disabled");
        }
        let verified = super::image::VerifiedImage::open(manifest)?;
        let mut child = Command::new(verified.path())
            .args([
                if probe {
                    "--executor-probe"
                } else {
                    "--executor"
                },
                &std::process::id().to_string(),
                &tag.to_string(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "executor-start")?;
        let pipe = child.stdin.take().ok_or("executor-pipe")?;
        let output = child.stdout.take().ok_or("executor-pipe")?;
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
            .map_err(|_| "executor-start")??;
        if ready.sequence != u32::MAX || !ready.accepted {
            let _ = child.kill();
            let _ = child.wait();
            return Err("executor-start");
        }
        Ok(Self {
            child,
            pipe,
            replies,
            sequence: 0,
            cleanup: WindowsBackend::new(tag),
            fenced: false,
            probe,
            mirror: Ledger::default(),
        })
    }
    fn terminate(&mut self) -> Result<(), &'static str> {
        if !self.fenced {
            if self
                .child
                .try_wait()
                .map_err(|_| "executor-stop")?
                .is_none()
            {
                self.child.kill().map_err(|_| "executor-stop")?;
            }
            self.child.wait().map_err(|_| "executor-stop")?;
            self.fenced = true;
        }
        Ok(())
    }
}
impl ProcessInput {
    fn transmit(&mut self, source: &Source, action: Option<Action>) -> Result<(), &'static str> {
        let request = Step {
            sequence: self.sequence,
            expires: uptime().checked_add(75).ok_or("clock-invalid")?,
            source: source.clone(),
            action,
        };
        self.sequence = self.sequence.checked_add(1).ok_or("sequence-exhausted")?;
        write_frame(
            &mut self.pipe,
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
    fn retain_cleanup(&self) -> bool {
        !self.probe
    }
    fn heartbeat(&mut self, source: &Source) -> Result<(), &'static str> {
        if self.fenced {
            return Err("revoked");
        }
        self.transmit(source, None)
    }
    fn prepare(&mut self) -> Result<(), &'static str> {
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
        self.mirror = ledger.clone();
        Ok(())
    }
    fn apply(&mut self, source: &Source, action: &Action) -> Result<(), &'static str> {
        if self.fenced {
            if !matches!(
                action,
                Action::Key { down: false, .. } | Action::Button { down: false, .. }
            ) || !self.mirror.is_held(action)
            {
                return Err("cleanup-only");
            }
            if self.probe {
                return Ok(());
            }
            return self.cleanup.apply(source, action);
        }
        self.transmit(source, Some(action.clone()))
    }
    fn fence(&mut self) -> Result<(), &'static str> {
        self.terminate()
    }
    fn cleanup_allowed(&mut self) -> Result<(), &'static str> {
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
    }
}

pub fn run_child(parent_pid: u32, tag: usize, probe: bool) -> Result<(), &'static str> {
    if !probe && !cfg!(feature = "native-input") {
        return Err("native-input-disabled");
    }
    let parent = Parent::validate_executor(parent_pid, probe)?;
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
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(10));
            if !parent.alive()
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
                            }
                            _ => false,
                        };
                        if done {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                std::process::exit(125);
            }
        }
    });
    let _activity = if probe {
        None
    } else {
        Some(super::activity::ActivityMonitor::start(stop.clone(), tag)?)
    };
    if !probe && !super::input::local_input_idle() {
        stop.store(true, Ordering::SeqCst);
        return Err("local-input-held");
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
    let outcome = (|| {
        loop {
            let bytes = receive.recv_timeout(Duration::from_millis(25));
            if stop.load(Ordering::SeqCst) {
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
                    || !stop.load(Ordering::SeqCst) && uptime() < request.expires,
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
    })();
    stop.store(true, Ordering::SeqCst);
    if !probe {
        loop {
            let released = match (input.lock(), source.lock()) {
                (Ok(mut input), Ok(source)) => source
                    .as_ref()
                    .is_none_or(|source| input.cleanup(source).is_ok()),
                _ => false,
            };
            if released {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    outcome
}
