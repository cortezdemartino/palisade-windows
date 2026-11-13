use super::*;
use std::{fs, os::windows::process::CommandExt, path::PathBuf, sync::atomic::AtomicU64, thread};

static NEXT: AtomicU64 = AtomicU64::new(1);
fn terminal_fixture() -> TerminalReceipt {
    TerminalReceipt {
        protocol: "palisade.assist.executor-terminal".into(),
        version: 1,
        creator: std::process::id(),
        executor: 456,
        tag: 123,
        nonce: "A".repeat(43),
        mode: crate::custody::Mode::Native,
        fenced: true,
        owned_clear: true,
    }
}
#[test]
fn dedicated_receipt_parser_requires_one_complete_bounded_strict_frame_and_eof() {
    use std::io::Cursor;
    let mut valid = Vec::new();
    crate::framing::write_bounded(
        &mut valid,
        &serde_json::to_vec(&terminal_fixture()).unwrap(),
        4096,
    )
    .unwrap();
    assert!(read_terminal(Cursor::new(&valid)).is_ok());
    for length in 0..valid.len() {
        assert!(read_terminal(Cursor::new(&valid[..length])).is_err());
    }
    let mut twice = valid.clone();
    twice.extend(&valid);
    assert!(read_terminal(Cursor::new(twice)).is_err());
    let mut trailing = valid.clone();
    trailing.push(1);
    assert!(read_terminal(Cursor::new(trailing)).is_err());
    for bytes in [
        br#"{"version":1,"version":1}"#.as_slice(),
        br#"{"approved":true}"#,
        b"not-json",
        &vec![b'x'; 4097],
    ] {
        let mut frame = (bytes.len() as u32).to_le_bytes().to_vec();
        frame.extend(bytes);
        assert!(read_terminal(Cursor::new(frame)).is_err());
    }
}
#[test]
fn terminal_receipts_bind_actor_generation_mode_and_sticky_failure() {
    for field in [
        "valid", "creator", "executor", "tag", "nonce", "mode", "fenced", "owned",
    ] {
        let (send, receive) = mpsc::sync_channel(2);
        let mut owner = TerminalOwner {
            nonce: "A".repeat(43),
            tag: 123,
            child: 456,
            expected_mode: crate::custody::Mode::Native,
            receive,
            confirmed: false,
            failed: false,
        };
        assert_eq!(owner.poll(), Err("executor-cleanup-pending"));
        let mut receipt = terminal_fixture();
        match field {
            "creator" => receipt.creator += 1,
            "executor" => receipt.executor += 1,
            "tag" => receipt.tag += 1,
            "nonce" => receipt.nonce = "B".repeat(43),
            "mode" => receipt.mode = crate::custody::Mode::Probe,
            "fenced" => receipt.fenced = false,
            "owned" => receipt.owned_clear = false,
            _ => (),
        }
        send.send(Ok(receipt)).unwrap();
        if field == "valid" {
            owner.poll().unwrap();
            owner.poll().unwrap();
        } else {
            assert_eq!(owner.poll(), Err("executor-cleanup-unproven"));
            send.send(Ok(terminal_fixture())).unwrap();
            assert_eq!(owner.poll(), Err("executor-cleanup-unproven"));
        }
    }
}
#[test]
fn missing_or_invalid_terminal_never_becomes_an_empty_ledger_success() {
    for invalid in [false, true] {
        let (send, receive) = mpsc::sync_channel(1);
        let mut owner = TerminalOwner {
            nonce: "A".repeat(43),
            tag: 123,
            child: 456,
            expected_mode: crate::custody::Mode::Native,
            receive,
            confirmed: false,
            failed: false,
        };
        if invalid {
            send.send(Err("executor-terminal-extra")).unwrap();
        }
        drop(send);
        assert_eq!(owner.poll(), Err("executor-cleanup-unproven"));
        assert_eq!(owner.poll(), Err("executor-cleanup-unproven"));
    }
}
fn fixture_files() -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "palisade-input-fence-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir(&root).unwrap();
    Fixture {
        ready: root.join("ready"),
        released: root.join("released"),
        allow: root.join("allow"),
        directory: root,
    }
}
struct Fixture {
    directory: PathBuf,
    ready: PathBuf,
    released: PathBuf,
    allow: PathBuf,
}

#[test]
fn witness_process_fixture() {
    let Some(pid) = std::env::var_os("PALISADE_INPUT_WITNESS_PARENT") else {
        return;
    };
    let parent = Parent::validate_executor(pid.to_str().unwrap().parse().unwrap(), true).unwrap();
    let witness = ExecutorWitness::retain(
        &parent,
        std::env::var("PALISADE_INPUT_WITNESS_HANDLE")
            .unwrap()
            .parse()
            .unwrap(),
    )
    .unwrap();
    let ready = std::env::var_os("PALISADE_INPUT_WITNESS_READY").unwrap();
    let finished = std::env::var_os("PALISADE_INPUT_WITNESS_FINISHED").unwrap();
    fs::write(ready, "ready").unwrap();
    let until = Instant::now() + Duration::from_secs(8);
    while !witness.stopped() {
        assert!(Instant::now() < until, "direct stop not observed");
        thread::sleep(Duration::from_millis(5));
    }
    fs::write(finished, "direct-native-stop").unwrap();
}

#[test]
fn created_child_observes_native_stop_without_pipe_eof_or_boolean_propagation() {
    let files = fixture_files();
    let stop = Arc::new(super::super::process_tree::StopEvent::new().unwrap());
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "windows::process_input::tests::witness_process_fixture",
            "--nocapture",
        ])
        .env(
            "PALISADE_INPUT_WITNESS_PARENT",
            std::process::id().to_string(),
        )
        .env("PALISADE_INPUT_WITNESS_HANDLE", stop.raw().to_string())
        .env("PALISADE_INPUT_WITNESS_READY", &files.ready)
        .env("PALISADE_INPUT_WITNESS_FINISHED", &files.released)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
        .spawn()
        .unwrap();
    let mut owner = ExecutorProcess::new(child, Some(stop.clone()));
    let until = Instant::now() + Duration::from_secs(10);
    while !files.ready.exists() {
        assert!(owner.child.try_wait().unwrap().is_none());
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(5));
    }
    stop.revoke().unwrap();
    // Keep stdin open throughout the actual child exit. No reader, renderer
    // event or mirrored AtomicBool relays this stop to its retained witness.
    assert!(owner.pipe.is_some());
    assert!(owner.child.wait().unwrap().success());
    owner.reaped = true;
    assert_eq!(
        fs::read_to_string(&files.released).unwrap(),
        "direct-native-stop"
    );
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.ready);
        let _ = fs::remove_file(&self.released);
        let _ = fs::remove_file(&self.allow);
        let _ = fs::remove_dir(&self.directory);
    }
}
// A fixed native test actor, with no hooks, input or command interpreter. Its
// private marker is startup synchronization, not an executor READY/receipt.
#[test]
fn pipe_process_fixture() {
    use std::io::{Read, Write};
    let Ok(mode) = std::env::var("PALISADE_FENCE_TEST_MODE") else {
        return;
    };
    assert!(
        [
            "silent",
            "quiet",
            "denied",
            "wrong",
            "malformed",
            "gated",
            "ready"
        ]
        .contains(&mode.as_str())
    );
    let parent = Parent::validate_executor(
        std::env::var("PALISADE_FENCE_TEST_PARENT")
            .unwrap()
            .parse()
            .unwrap(),
        true,
    )
    .unwrap();
    // The unit-test runner has its own stdout banner. The fixture preparer
    // consumes only this bounded private preamble before testing strict ACKs.
    std::io::stdout()
        .write_all(b"palisade-pipe-fixture\n")
        .unwrap();
    std::io::stdout().flush().unwrap();
    fs::write(
        std::env::var_os("PALISADE_FENCE_TEST_READY").unwrap(),
        "ready",
    )
    .unwrap();
    if !["silent", "quiet"].contains(&mode.as_str()) {
        let payload = match mode.as_str() {
            "denied" => br#"{"sequence":4294967295,"accepted":false,"uptime":1}"#.as_slice(),
            "wrong" => br#"{"sequence":0,"accepted":true,"uptime":1}"#,
            "malformed" => br#"{"extra":true}"#,
            _ => br#"{"sequence":4294967295,"accepted":true,"uptime":1}"#,
        };
        write_frame(&mut std::io::stdout().lock(), payload).unwrap();
    }
    let mut bytes = [0; 1024];
    while std::io::stdin().read(&mut bytes).unwrap() != 0 {}
    if mode == "gated" {
        let allow = PathBuf::from(std::env::var_os("PALISADE_FENCE_TEST_ALLOW").unwrap());
        let until = Instant::now() + Duration::from_secs(10);
        while !allow.exists() {
            assert!(parent.alive());
            assert!(Instant::now() < until);
            thread::sleep(Duration::from_millis(5));
        }
    }
    fs::write(
        std::env::var_os("PALISADE_FENCE_TEST_RELEASED").unwrap(),
        "released-after-eof",
    )
    .unwrap();
}
// Records real EOF cleanup, so forced process termination cannot pass the tests.
fn fixture(mode: &str, stdout: bool) -> (Child, Fixture) {
    let files = fixture_files();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "windows::process_input::tests::pipe_process_fixture",
            "--nocapture",
            "--quiet",
        ])
        .env("PALISADE_FENCE_TEST_MODE", mode)
        .env("PALISADE_FENCE_TEST_PARENT", std::process::id().to_string())
        .env("PALISADE_FENCE_TEST_READY", &files.ready)
        .env("PALISADE_FENCE_TEST_RELEASED", &files.released)
        .env("PALISADE_FENCE_TEST_ALLOW", &files.allow)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
        .spawn()
        .unwrap();
    let mut output = child.stdout.take().unwrap();
    let (send, receive) = mpsc::sync_channel(1);
    thread::spawn(move || {
        use std::io::Read;
        let result = (|| {
            let mut preamble = Vec::new();
            loop {
                let mut byte = [0];
                output
                    .read_exact(&mut byte)
                    .map_err(|_| "fixture preamble EOF")?;
                preamble.push(byte[0]);
                if preamble.len() > 1024 {
                    return Err("fixture preamble too large");
                }
                if preamble.ends_with(b"palisade-pipe-fixture\n") {
                    return Ok(output);
                }
            }
        })();
        let _ = send.send(result);
    });
    // Prepare the independent fixture before measuring the native three-second
    // startup deadline. This neither changes that deadline nor fabricates ACKs.
    let until = Instant::now() + Duration::from_secs(15);
    child.stdout = Some(
        receive
            .recv_timeout(Duration::from_secs(15))
            .expect("fixture registration timed out")
            .expect("fixture registration failed"),
    );
    while !files.ready.exists() {
        assert!(child.try_wait().unwrap().is_none(), "fixture failed");
        assert!(Instant::now() < until, "fixture startup failed");
        thread::sleep(Duration::from_millis(5));
    }
    if !stdout {
        drop(child.stdout.take());
    }
    (child, files)
}
fn source() -> Source {
    Source {
        x: 0,
        y: 0,
        width: 100,
        height: 100,
        generation: 1,
    }
}

#[test]
fn fence_is_nonblocking_and_mirror_release_requires_actual_child_exit() {
    let (child, files) = fixture("gated", true);
    let stop = Arc::new(super::super::process_tree::StopEvent::new().unwrap());
    let observer = stop.reader().unwrap();
    let mut input = ProcessInput::from_child(child, 123, true, Some(stop)).unwrap();
    let up = Action::Key {
        code: "ShiftLeft".into(),
        down: false,
    };
    input
        .mirror
        .plan(&Action::Key {
            code: "ShiftLeft".into(),
            down: true,
        })
        .unwrap();
    assert_eq!(input.fence(), Err("executor-cleanup-pending"));
    assert_ne!(
        unsafe { WaitForSingleObject(observer.as_raw_handle(), 0) },
        WAIT_TIMEOUT
    );
    assert!(input.process.fenced);
    assert!(!input.process.reaped);
    assert!(input.process.pipe.is_none());
    assert_eq!(input.heartbeat(&source()), Err("revoked"));
    assert_eq!(input.apply(&source(), &up), Err("executor-cleanup-pending"));
    // The child cannot exit before this permission. A blocking fence would
    // time out the fixture and fail, rather than pass on a lucky delay.
    fs::write(&files.allow, "finish").unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while input.fence() == Err("executor-cleanup-pending") {
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(5));
    }
    assert!(input.process.reaped);
    assert_eq!(input.fence(), Ok(()));
    assert_eq!(
        fs::read_to_string(&files.released).unwrap(),
        "released-after-eof"
    );
    assert_eq!(input.apply(&source(), &up), Ok(()));
    assert_eq!(
        input.apply(
            &source(),
            &Action::Key {
                code: "AltLeft".into(),
                down: false
            }
        ),
        Err("cleanup-only")
    );
    assert_eq!(
        input.apply(
            &source(),
            &Action::Key {
                code: "ShiftLeft".into(),
                down: true
            }
        ),
        Err("cleanup-only")
    );
}

#[test]
fn malformed_or_denied_startup_retains_child_until_eof_cleanup_exits() {
    for mode in ["denied", "wrong", "malformed"] {
        let (child, files) = fixture(mode, true);
        assert!(ProcessInput::from_child(child, 123, true, None).is_err());
        assert_eq!(
            fs::read_to_string(&files.released).unwrap(),
            "released-after-eof",
            "{mode}"
        );
    }
}

#[test]
fn silent_startup_timeout_and_missing_output_do_not_kill_the_created_child() {
    for (mode, output) in [("silent", true), ("quiet", false)] {
        let (child, files) = fixture(mode, output);
        assert!(ProcessInput::from_child(child, 123, true, None).is_err());
        assert_eq!(
            fs::read_to_string(&files.released).unwrap(),
            "released-after-eof",
            "{mode}"
        );
    }
}

#[test]
fn dropping_a_live_owner_closes_ordinary_input_and_retains_exit_ownership() {
    let (child, files) = fixture("ready", true);
    let input = ProcessInput::from_child(child, 123, true, None).unwrap();
    assert!(!files.released.exists());
    drop(input);
    assert_eq!(
        fs::read_to_string(&files.released).unwrap(),
        "released-after-eof"
    );
}
