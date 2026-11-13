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
fn ticket() -> Ticket {
    Registry::new([1; 32], [2; 32])
        .reserve("session_fixture", 1, 1, [3; 32], Mode::Probe)
        .unwrap()
}

#[test]
fn partial_unpublished_transfer_failure_closes_every_prior_copy() {
    let coordinator = target();
    let event = StopEvent::new().unwrap();
    let handles;
    {
        let mut transfers = Transfers {
            coordinator: &coordinator,
            unpublished: Vec::new(),
        };
        handles = [
            transfers
                .add(event.handle_for_query(), SYNCHRONIZATION_SYNCHRONIZE)
                .unwrap(),
            transfers
                .add(event.handle_for_query(), SYNCHRONIZATION_SYNCHRONIZE)
                .unwrap(),
        ];
        assert!(handles.iter().all(|h| open(*h)));
        assert!(
            transfers
                .add(ptr::null_mut(), SYNCHRONIZATION_SYNCHRONIZE)
                .is_err()
        );
    }
    assert!(handles.iter().all(|h| !open(*h)));
    assert_eq!(
        unsafe { WaitForSingleObject(event.handle_for_query(), 0) },
        WAIT_TIMEOUT
    );
}

#[test]
fn late_unadopted_reply_closes_full_bundle_without_arm() {
    let coordinator = target();
    let (read, write) = crate::windows::process_tree::custody_pipe().unwrap();
    let process = transfer(
        &coordinator,
        coordinator.as_raw_handle(),
        PROCESS_QUERY_LIMITED_INFORMATION,
    )
    .unwrap();
    let input = transfer(
        &coordinator,
        write.as_raw_handle(),
        windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE,
    )
    .unwrap();
    let output = transfer(
        &coordinator,
        read.as_raw_handle(),
        windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ,
    )
    .unwrap();
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
    assert!([process, input, output].iter().all(|h| !open(*h)));
    assert!(open(read.as_raw_handle() as usize));
    assert!(open(write.as_raw_handle() as usize));
}

#[test]
fn malformed_middle_role_cannot_leak_later_valid_handles() {
    let coordinator = target();
    let event = StopEvent::new().unwrap();
    let a = transfer(
        &coordinator,
        event.handle_for_query(),
        SYNCHRONIZATION_SYNCHRONIZE,
    )
    .unwrap();
    let b = transfer(
        &coordinator,
        event.handle_for_query(),
        SYNCHRONIZATION_SYNCHRONIZE,
    )
    .unwrap();
    assert!(unsafe { adopt_transport([a, 0, b]) }.is_err());
    assert!(!open(a));
    assert!(!open(b));
    let a = transfer(
        &coordinator,
        event.handle_for_query(),
        SYNCHRONIZATION_SYNCHRONIZE,
    )
    .unwrap();
    assert!(unsafe { adopt_transport([a, a, 0]) }.is_err());
    assert!(!open(a));
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
