use super::*;

fn target() -> OwnedHandle {
    duplicate(
        unsafe { GetCurrentProcess() },
        PROCESS_DUP_HANDLE | PROCESS_QUERY_LIMITED_INFORMATION,
        false,
    )
    .unwrap()
}
fn open(raw: usize) -> bool {
    let mut flags = 0;
    unsafe { GetHandleInformation(raw as HANDLE, &mut flags) != 0 }
}
// Check original object lifetime, never whether a closed numeric slot has been
// reused by another libtest thread. Only these tests know the random name.
fn named_event() -> (OwnedHandle, Vec<u16>) {
    let nonce: String = random()
        .unwrap()
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect();
    let name: Vec<u16> = format!(
        "Local\\Palisade.Assist.Custody.Test.{}.{}",
        std::process::id(),
        nonce
    )
    .encode_utf16()
    .chain(Some(0))
    .collect();
    unsafe {
        SetLastError(0);
        let raw = CreateEventW(ptr::null(), 1, 0, name.as_ptr());
        assert!(!raw.is_null());
        let owned = OwnedHandle::from_raw_handle(raw);
        assert_ne!(GetLastError(), ERROR_ALREADY_EXISTS);
        (owned, name)
    }
}
fn event_gone(name: &[u16]) {
    unsafe {
        let raw = OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr());
        if raw.is_null() {
            assert_eq!(GetLastError(), ERROR_FILE_NOT_FOUND);
        } else {
            drop(OwnedHandle::from_raw_handle(raw));
            panic!("exact transferred event is still retained");
        }
    }
}
fn ticket() -> Ticket {
    Registry::new([1; 32], [2; 32])
        .reserve("session_fixture", 1, 1, [3; 32], Mode::Probe)
        .unwrap()
}

#[test]
fn partial_unpublished_transfer_failure_closes_every_prior_copy() {
    let coordinator = target();
    let (event, name) = named_event();
    let handles;
    {
        let mut transfers = Transfers {
            coordinator: &coordinator,
            unpublished: Vec::new(),
        };
        handles = [
            transfers
                .add(event.as_raw_handle(), SYNCHRONIZATION_SYNCHRONIZE)
                .unwrap(),
            transfers
                .add(event.as_raw_handle(), SYNCHRONIZATION_SYNCHRONIZE)
                .unwrap(),
        ];
        assert!(handles.iter().all(|h| open(*h)));
        assert!(
            transfers
                .add(ptr::null_mut(), SYNCHRONIZATION_SYNCHRONIZE)
                .is_err()
        );
    }
    assert_eq!(
        unsafe { WaitForSingleObject(event.as_raw_handle(), 0) },
        WAIT_TIMEOUT
    );
    drop(event);
    event_gone(&name);
}

#[test]
fn late_unadopted_reply_closes_full_bundle_without_arm() {
    let coordinator = target();
    let (event, name) = named_event();
    let (input_read, input_write) = crate::windows::process_tree::custody_pipe().unwrap();
    let (output_read, output_write) = crate::windows::process_tree::custody_pipe().unwrap();
    let process = transfer(
        &coordinator,
        event.as_raw_handle(),
        SYNCHRONIZATION_SYNCHRONIZE,
    )
    .unwrap();
    let input = transfer(
        &coordinator,
        input_write.as_raw_handle(),
        windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE,
    )
    .unwrap();
    let output = transfer(
        &coordinator,
        output_read.as_raw_handle(),
        windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ,
    )
    .unwrap();
    drop(event);
    drop(input_write);
    drop(output_read);
    let (send, receive) = reply_channel();
    assert!(matches!(
        send.try_send(Err("probe")),
        Err(mpsc::TrySendError::Full(_))
    ));
    let worker = thread::spawn(move || {
        deliver(
            send,
            Ok(Answer::Spawned {
                ticket: ticket(),
                process,
                input,
                output,
                pid: 1,
                created: 1,
            }),
        )
    });
    // A receiver exists but never receives. There can be no send-success,
    // drop-unread gap: this is the production zero-capacity reply factory.
    drop(receive);
    worker.join().unwrap();
    // Deliberately allocate more objects after the bundle is closed. Numeric
    // slots may now refer to these unrelated live objects, never to leaked roles.
    let _replacements: Vec<_> = (0..64).map(|_| StopEvent::new().unwrap()).collect();
    event_gone(&name);
    unsafe {
        use windows_sys::Win32::System::Pipes::PeekNamedPipe;
        assert_eq!(
            PeekNamedPipe(
                input_read.as_raw_handle(),
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut()
            ),
            0
        );
        assert_eq!(
            GetLastError(),
            ERROR_BROKEN_PIPE,
            "transferred writer must be closed"
        );
    }
    let error = File::from(output_write).write_all(b"x").unwrap_err();
    assert!(
        matches!(error.raw_os_error(), Some(code) if code == ERROR_BROKEN_PIPE as i32 || code == ERROR_NO_DATA as i32),
        "transferred reader must be closed"
    );
}

#[test]
fn pipe_lifetime_checks_distinguish_a_live_alias_from_a_closed_transport_role() {
    use windows_sys::Win32::System::Pipes::PeekNamedPipe;
    let (read, write) = crate::windows::process_tree::custody_pipe().unwrap();
    let alias = duplicate(
        write.as_raw_handle(),
        windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE,
        false,
    )
    .unwrap();
    drop(write);
    let peek = || unsafe {
        PeekNamedPipe(
            read.as_raw_handle(),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    assert_ne!(peek(), 0, "a leaked writer must not pass the EOF witness");
    drop(alias);
    assert_eq!(peek(), 0);
    assert_eq!(unsafe { GetLastError() }, ERROR_BROKEN_PIPE);

    let (read, write) = crate::windows::process_tree::custody_pipe().unwrap();
    let alias = duplicate(
        read.as_raw_handle(),
        windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ,
        false,
    )
    .unwrap();
    drop(read);
    let mut write = File::from(write);
    write
        .write_all(b"x")
        .expect("a live reader alias must accept one bounded byte");
    drop(alias);
    let error = write.write_all(b"x").unwrap_err();
    assert!(
        matches!(error.raw_os_error(), Some(code) if code == ERROR_BROKEN_PIPE as i32 || code == ERROR_NO_DATA as i32)
    );
}

#[test]
fn malformed_middle_role_cannot_leak_later_valid_handles() {
    let coordinator = target();
    let (event, name) = named_event();
    let a = transfer(
        &coordinator,
        event.as_raw_handle(),
        SYNCHRONIZATION_SYNCHRONIZE,
    )
    .unwrap();
    let b = transfer(
        &coordinator,
        event.as_raw_handle(),
        SYNCHRONIZATION_SYNCHRONIZE,
    )
    .unwrap();
    assert!(unsafe { adopt_transport([a, 0, b]) }.is_err());
    drop(event);
    event_gone(&name);
    let (event, name) = named_event();
    let a = transfer(
        &coordinator,
        event.as_raw_handle(),
        SYNCHRONIZATION_SYNCHRONIZE,
    )
    .unwrap();
    assert!(unsafe { adopt_transport([a, a, 0]) }.is_err());
    drop(event);
    event_gone(&name);
}

#[test]
fn seal_signals_independent_event_before_any_rpc_progress() {
    let urgent = Arc::new(StopEvent::new().unwrap());
    let (requests, receive) = mpsc::sync_channel(1);
    let client = Client {
        requests,
        failed: Arc::new(AtomicBool::new(false)),
        mode: Mode::Probe,
        urgent: urgent.clone(),
        monitor: None,
    };
    let worker = thread::spawn(move || client.seal());
    // The actor has not consumed the request and cannot produce a reply yet.
    assert_eq!(
        unsafe { WaitForSingleObject(urgent.handle_for_query(), 1000) },
        WAIT_OBJECT_0
    );
    let rpc = receive.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(matches!(rpc.command, Request::Seal {}));
    rpc.reply.send(Ok(Answer::Done {})).ok().unwrap();
    worker.join().unwrap().unwrap();
}

struct FailingWriter {
    remaining: usize,
    flush_fails: bool,
}
impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.remaining == 0 {
            return Err(std::io::ErrorKind::BrokenPipe.into());
        }
        let count = self.remaining.min(bytes.len());
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if self.flush_fails {
            Err(std::io::ErrorKind::BrokenPipe.into())
        } else {
            Ok(())
        }
    }
}
#[test]
fn partial_and_flush_failed_publications_retain_ambiguous_handoffs_without_numeric_close() {
    let event = StopEvent::new().unwrap();
    let raw = event.handle_for_query() as usize;
    for writer in [
        FailingWriter {
            remaining: 0,
            flush_fails: false,
        },
        FailingWriter {
            remaining: 5,
            flush_fails: false,
        },
        FailingWriter {
            remaining: usize::MAX,
            flush_fails: true,
        },
    ] {
        let mut handoff = Handoff {
            handles: vec![raw],
            phase: Publication::Prepared,
        };
        assert!(handoff.publish(&mut { writer }, b"complete reply").is_err());
        assert_eq!(handoff.phase, Publication::Uncertain);
        assert!(!handoff.permits_retirement());
        assert!(handoff.adopted().is_err());
        assert_eq!(handoff.handles, [raw]);
        assert!(
            open(raw),
            "never numerically close a possibly delivered handle"
        );
    }
}
#[test]
fn full_publication_is_not_adoption_or_input_cleanup() {
    let mut handoff = Handoff {
        handles: vec![123],
        phase: Publication::Prepared,
    };
    handoff.publish(&mut Vec::new(), b"complete reply").unwrap();
    assert_eq!(handoff.phase, Publication::Published);
    assert!(!handoff.permits_retirement());
    let mut registry = Registry::new([1; 32], [2; 32]);
    let ticket = registry
        .reserve("session_fixture", 1, 1, [3; 32], Mode::Native)
        .unwrap();
    registry.begin_spawn(&ticket).unwrap();
    let process = ProcessIdentity {
        pid: 123,
        created: 456,
        image: [2; 32],
    };
    registry.bind(&ticket, process).unwrap();
    registry.child_exited(&ticket, process).unwrap();
    assert!(registry.completion(&ticket).unwrap().is_some()); // Never armed.
    assert!(!handoff.permits_retirement()); // Still blocks successor/owner retirement.
    handoff.coordinator_exited();
    assert!(handoff.permits_retirement());

    let mut handoff = Handoff {
        handles: vec![123],
        phase: Publication::Prepared,
    };
    handoff.publish(&mut Vec::new(), b"complete reply").unwrap();
    handoff.adopted().unwrap();
    assert!(handoff.permits_retirement());
    assert!(handoff.handles.is_empty());
    assert!(handoff.adopted().is_err());
}

#[test]
fn correlated_sequence_cannot_substitute_another_operation_result() {
    let done = Answer::Done {};
    let observed = Answer::Observed {
        released: false,
        exited: false,
        never_armed: false,
    };
    let spawned = Answer::Spawned {
        ticket: ticket(),
        process: 1,
        input: 2,
        output: 3,
        pid: 123,
        created: 456,
    };
    for (kind, answers) in [
        (ReplyKind::Done, [true, false, false]),
        (ReplyKind::Observe, [false, true, false]),
        (ReplyKind::Spawn, [false, false, true]),
    ] {
        for (answer, accepted) in [&done, &observed, &spawned].into_iter().zip(answers) {
            assert_eq!(kind.accepts(answer), accepted);
        }
        assert!(kind.accepts(&Answer::Refused {}));
    }
}
