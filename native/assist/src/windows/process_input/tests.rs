use super::*;
use std::{fs, os::windows::process::CommandExt, path::PathBuf, sync::atomic::AtomicU64, thread};

static NEXT: AtomicU64 = AtomicU64::new(1);
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
// A fixed BCL-only process, with no hooks, native input or arbitrary commands.
// It records that EOF cleanup ran, so a forced kill cannot pass these tests.
fn fixture(mode: &str, stdout: bool) -> (Child, Fixture) {
    let files = fixture_files();
    let script = r#"
$ErrorActionPreference = 'Stop'
$mode = $env:PALISADE_FENCE_TEST_MODE
$output = [Console]::OpenStandardOutput()
if ($mode -ne 'silent' -and $mode -ne 'quiet') {
  $payload = switch ($mode) {
    'denied' { '{"sequence":4294967295,"accepted":false,"uptime":1}' }
    'wrong' { '{"sequence":0,"accepted":true,"uptime":1}' }
    'malformed' { '{"extra":true}' }
    default { '{"sequence":4294967295,"accepted":true,"uptime":1}' }
  }
  $bytes = [Text.Encoding]::UTF8.GetBytes($payload)
  $header = [BitConverter]::GetBytes([uint32]$bytes.Length)
  $output.Write($header,0,$header.Length)
  $output.Write($bytes,0,$bytes.Length)
  $output.Flush()
}
[IO.File]::WriteAllText($env:PALISADE_FENCE_TEST_READY,'ready')
$inputPipe = [Console]::OpenStandardInput()
$buffer = New-Object byte[] 1024
while ($inputPipe.Read($buffer,0,$buffer.Length) -ne 0) {}
if ($mode -eq 'gated') {
  $limit = [DateTime]::UtcNow.AddSeconds(10)
  while (-not [IO.File]::Exists($env:PALISADE_FENCE_TEST_ALLOW)) {
    if ([DateTime]::UtcNow -ge $limit) { exit 90 }
    [Threading.Thread]::Sleep(5)
  }
} else { [Threading.Thread]::Sleep(100) }
[IO.File]::WriteAllText($env:PALISADE_FENCE_TEST_RELEASED,'released-after-eof')
"#;
    let mut child = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("PALISADE_FENCE_TEST_MODE", mode)
        .env("PALISADE_FENCE_TEST_READY", &files.ready)
        .env("PALISADE_FENCE_TEST_RELEASED", &files.released)
        .env("PALISADE_FENCE_TEST_ALLOW", &files.allow)
        .stdin(Stdio::piped())
        .stdout(if stdout {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stderr(Stdio::null())
        .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
        .spawn()
        .unwrap();
    // Prepare the independent fixture before measuring the native three-second
    // startup deadline. This neither changes that deadline nor fabricates ACKs.
    let until = Instant::now() + Duration::from_secs(15);
    while !files.ready.exists() {
        assert!(child.try_wait().unwrap().is_none(), "fixture failed");
        assert!(Instant::now() < until, "fixture startup failed");
        thread::sleep(Duration::from_millis(5));
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
