#![cfg_attr(windows, windows_subsystem = "windows")]
use palisade_assist_guardian::{
    framing::{read_frame, write_frame},
    grant::{Expected, SignedGrant, decode, verify},
    guard::Watchdog,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    sequence: u32,
    command: Command,
}
#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields, rename_all = "kebab-case")]
enum Command {
    Hello {
        protocol: String,
        version: u32,
        challenge: String,
    },
    Status,
    VerifyGrant {
        signed: Box<SignedGrant>,
        issuer: String,
        expected: Box<Expected>,
    },
    EnrollmentChallenge {
        challenge: String,
    },
    Stop,
    Shutdown,
}
fn wall() -> Result<u64, &'static str> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock-invalid")?
        .as_millis()
        .try_into()
        .map_err(|_| "clock-invalid")
}
fn main() {
    if let Err(error) = run() {
        // Only fixed internal error codes, never input payloads, identities or keys.
        eprintln!("guardian: {error}");
        std::process::exit(125)
    }
}
#[cfg(not(windows))]
fn run() -> Result<(), &'static str> {
    Err("windows-only")
}

#[cfg(windows)]
fn run() -> Result<(), &'static str> {
    use palisade_assist_guardian::windows::{
        identity::Identity, lifecycle::NativeStop, parent::Parent,
    };
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 3 && ["--executor", "--executor-probe"].contains(&args[0].as_str()) {
        return palisade_assist_guardian::windows::process_input::run_child(
            args[1].parse().map_err(|_| "invalid-parent")?,
            args[2].parse().map_err(|_| "invalid-tag")?,
            args[0] == "--executor-probe",
        );
    }
    if args.len() != 2 || !["--guardian", "--probe"].contains(&args[0].as_str()) {
        return Err("invalid-launch");
    }
    let probe = args[0] == "--probe";
    let parent = Parent::validate(args[1].parse().map_err(|_| "invalid-parent")?, probe)?;
    let stopped = Arc::new(AtomicBool::new(false));
    let guard = Arc::new(Mutex::new(Watchdog::new(
        stopped.clone(),
        wall()?,
        Instant::now(),
    )));
    let _native_stop = NativeStop::start(stopped.clone(), probe)?;
    let watch = guard.clone();
    let watch_stop = stopped.clone();
    // This thread never waits for a frame, stdout, or the renderer.
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(25));
            let invalid = wall()
                .map(|time| {
                    watch
                        .lock()
                        .unwrap()
                        .check(time, Instant::now(), parent.alive())
                })
                .unwrap_or(true);
            if invalid || watch_stop.load(Ordering::SeqCst) {
                std::process::exit(125)
            }
        }
    });
    let (send, receive) = mpsc::sync_channel(1);
    let read_stop = stopped.clone();
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        loop {
            match read_frame(&mut input) {
                Ok(Some(bytes)) => {
                    if send.send(bytes).is_err() {
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
    let mut handshaken = false;
    let mut sequence = 0;
    let mut identity: Option<Identity> = None;
    let mut output = std::io::stdout().lock();
    loop {
        let bytes = receive.recv().map_err(|_| "pipe-ended")?;
        if stopped.load(Ordering::SeqCst) {
            return Err("stopped");
        }
        let request: Request = serde_json::from_slice(&bytes).map_err(|_| "invalid-message")?;
        if request.sequence != sequence {
            return Err("replay");
        }
        sequence = sequence.checked_add(1).ok_or("sequence-exhausted")?;
        let mut shutdown = false;
        let result = match request.command {
            Command::Hello {
                protocol,
                version,
                challenge,
            } if !handshaken => {
                if protocol != "palisade.assist.guardian" || version != 1 {
                    return Err("incompatible-protocol");
                }
                decode::<32>(&challenge)?;
                guard
                    .lock()
                    .map_err(|_| "watchdog")?
                    .handshake(Instant::now())?;
                handshaken = true;
                json!({"protocol":"palisade.assist.guardian","version":1,"challenge":challenge,"mode":if probe {"probe"} else {"guardian"},"nativeInput":false})
            }
            _ if !handshaken => return Err("handshake-required"),
            Command::Status => json!({"state":"inactive","nativeInput":false,"authority":"none"}),
            Command::VerifyGrant {
                signed,
                issuer,
                expected,
            } => {
                // Caller-supplied context/key is diagnostic, never admitted as trust/consent.
                match decode::<32>(&issuer)
                    .and_then(|key| verify(&signed, &key, &expected, wall()?))
                {
                    Ok(()) => json!({"signatureValid":true,"authorized":false}),
                    Err(error) => json!({"signatureValid":false,"authorized":false,"error":error}),
                }
            }
            Command::EnrollmentChallenge { challenge } if !probe => {
                let challenge = decode::<32>(&challenge)?;
                if identity.is_none() {
                    identity = Some(Identity::load_current_user()?);
                }
                let key = identity.as_ref().ok_or("identity-unavailable")?;
                json!({"publicKey":key.public_key(),"signature":key.sign_enrollment_challenge(&challenge)})
            }
            Command::Stop | Command::Shutdown => {
                stopped.store(true, Ordering::SeqCst);
                shutdown = true;
                json!({"state":"terminated","nativeInput":false})
            }
            _ => return Err("operation-unavailable"),
        };
        // A successful parent heartbeat is process liveness only, never Assist authorization.
        guard.lock().map_err(|_| "watchdog")?.touch(Instant::now());
        write_frame(
            &mut output,
            &serde_json::to_vec(&json!({"sequence":request.sequence,"result":result}))
                .map_err(|_| "invalid-response")?,
        )
        .map_err(|_| "pipe-failed")?;
        if shutdown {
            return Ok(());
        }
    }
}
