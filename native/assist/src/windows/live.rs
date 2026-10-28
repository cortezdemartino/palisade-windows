//! Non-authorizing live-channel foundation. Reader/watchdog never wait for
//! ordinary dispatch, stdout, UI or HTTPS. No StreamRuntime can be started here.
use super::parent::Parent;
use crate::{
    framing::{read_bounded, write_bounded},
    guard::Watchdog,
    live::{
        Command, Ingress, MAX_FRAME, ORDINARY_CAPACITY, OUTPUT_CAPACITY, PROTOCOL, Reply, Response,
        Signals,
    },
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
struct Output {
    response: Response,
    delivered: Option<mpsc::SyncSender<Result<(), &'static str>>>,
}
fn emit(
    output: &mpsc::SyncSender<Output>,
    sequence: u32,
    result: Reply,
    signals: &Signals,
) -> Result<(), &'static str> {
    output
        .try_send(Output {
            response: Response { sequence, result },
            delivered: None,
        })
        .map_err(|_| {
            signals.fail();
            "live-output-full"
        })
}
pub fn run(parent_pid: u32, probe: bool) -> Result<(), &'static str> {
    let parent = Parent::validate(parent_pid, probe)?;
    let signals = Arc::new(Signals::default());
    let finished = Arc::new(AtomicBool::new(false));
    let guard = Arc::new(Mutex::new(Watchdog::new(
        Arc::new(AtomicBool::new(false)),
        super::lease::now()?,
        Instant::now(),
    )));
    let (output, outgoing) = mpsc::sync_channel::<Output>(OUTPUT_CAPACITY);
    let writer_signals = signals.clone();
    thread::spawn(move || {
        let mut pipe = std::io::stdout().lock();
        for item in outgoing {
            let outcome = serde_json::to_vec(&item.response)
                .map_err(|_| "invalid-response")
                .and_then(|bytes| {
                    write_bounded(&mut pipe, &bytes, MAX_FRAME).map_err(|_| "live-pipe-failed")
                });
            if outcome.is_err() {
                writer_signals.fail();
            }
            if let Some(delivered) = item.delivered {
                let _ = delivered.send(outcome);
            }
            if outcome.is_err() {
                break;
            }
        }
    });
    let watch_signals = signals.clone();
    let watch_guard = guard.clone();
    let watch_finished = finished.clone();
    let watcher = thread::spawn(move || {
        while !watch_finished.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(25));
            if super::lease::now()
                .and_then(|wall| {
                    watch_guard
                        .lock()
                        .map_err(|_| "watchdog")
                        .map(|mut g| g.check(wall, Instant::now(), parent.alive()))
                })
                .unwrap_or(true)
            {
                watch_signals.fail();
            }
        }
    });
    let (ordinary, receive) = mpsc::sync_channel(ORDINARY_CAPACITY);
    let shutdown = Arc::new(Mutex::new(None));
    let read_shutdown = shutdown.clone();
    let read_signals = signals.clone();
    let read_output = output.clone();
    thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        let mut ingress = Ingress::new(probe);
        let outcome = (|| {
            loop {
                let bytes = read_bounded(&mut input, MAX_FRAME)
                    .map_err(|_| "live-frame")?
                    .ok_or("live-eof")?;
                let request = ingress.accept(&bytes, &read_signals)?;
                match &request.command {
                    Command::Hello { challenge, .. } => {
                        guard
                            .lock()
                            .map_err(|_| "watchdog")?
                            .handshake(Instant::now())?;
                        emit(
                            &read_output,
                            request.sequence,
                            Reply::Hello {
                                protocol: PROTOCOL,
                                version: 1,
                                challenge: challenge.clone(),
                                mode: if probe { "live-probe" } else { "live" },
                                native_input: false,
                                cleanup_scope: "channel-only",
                            },
                            &read_signals,
                        )?;
                    }
                    Command::Pulse {} => {
                        guard.lock().map_err(|_| "watchdog")?.touch(Instant::now());
                        emit(&read_output, request.sequence, Reply::Pulse, &read_signals)?;
                    }
                    Command::Shutdown {} => {
                        // Ingress has already revoked, before either lock or dispatch.
                        *read_shutdown.lock().map_err(|_| "live-state")? = Some((
                            request.sequence,
                            ingress.challenge().ok_or("handshake-required")?.to_owned(),
                        ));
                        read_signals.shutdown();
                        return Ok::<(), &'static str>(());
                    }
                    _ => ordinary.try_send(request).map_err(|_| "live-queue-full")?,
                }
            }
        })();
        if outcome.is_err() {
            read_signals.fail();
        }
    });
    let outcome = (|| {
        while !signals.closing() {
            let request = match receive.recv_timeout(Duration::from_millis(10)) {
                Ok(request) => request,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => {
                    if !signals.closing() {
                        signals.fail();
                    }
                    break;
                }
            };
            match request.command {
                Command::Status {} => emit(
                    &output,
                    request.sequence,
                    Reply::Status {
                        state: if signals.revoked() {
                            "revoked"
                        } else {
                            "inactive"
                        },
                        authority: "none",
                    },
                    &signals,
                )?,
                Command::ProbeWait { milliseconds } => {
                    let started = Instant::now();
                    while started.elapsed() < Duration::from_millis(u64::from(milliseconds))
                        && !signals.revoked()
                    {
                        thread::sleep(Duration::from_millis(5));
                    }
                    if !signals.revoked() {
                        emit(&output, request.sequence, Reply::ProbeWait, &signals)?;
                    }
                }
                _ => return Err("invalid-dispatch"),
            }
        }
        // No sessions/children are admitted in this revision. Active-resource
        // cleanup and crash-surviving ownership must precede a broader proof.
        signals.revoke();
        if signals.failed() {
            return Err("live-failed");
        }
        let (sequence, challenge) = shutdown
            .lock()
            .map_err(|_| "live-state")?
            .take()
            .ok_or("live-failed")?;
        let (delivered, confirmation) = mpsc::sync_channel(1);
        output
            .try_send(Output {
                response: Response {
                    sequence,
                    result: Reply::CleanupComplete {
                        challenge,
                        scope: "channel-only",
                    },
                },
                delivered: Some(delivered),
            })
            .map_err(|_| "live-output-full")?;
        // Backpressure cannot cause an unproven success. There is no force-exit
        // timeout; the parent drains stdout while its lifecycle holder waits.
        confirmation.recv().map_err(|_| "live-output-failed")??;
        if signals.failed() {
            return Err("live-failed");
        }
        Ok(())
    })();
    signals.revoke();
    finished.store(true, Ordering::SeqCst);
    watcher.join().map_err(|_| "live-watchdog-failed")?;
    outcome
}
