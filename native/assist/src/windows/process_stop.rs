//! A direct native stop witness, not an authority or input-cleanup receipt.
//! Retained immutable handles avoid PID reuse and polling propagation windows.
use super::{parent::Parent, process_tree::InheritedTree};
use crate::cancellation::StopWitness;
use std::os::windows::io::{AsRawHandle, OwnedHandle};
use windows_sys::Win32::{Foundation::WAIT_TIMEOUT, System::Threading::WaitForSingleObject};

pub(crate) struct ProcessStop {
    monitor: OwnedHandle,
    stop: OwnedHandle,
}
impl ProcessStop {
    pub(crate) fn retain(parent: &Parent, tree: &InheritedTree) -> Result<Self, &'static str> {
        let result = Self {
            monitor: parent.wait_handle()?,
            stop: tree.stop_reader()?,
        };
        if result.stopped() {
            return Err("monitor-stopped");
        }
        Ok(result)
    }
}
impl StopWitness for ProcessStop {
    fn stopped(&self) -> bool {
        // Event signal, monitor exit, invalid handle or any query error all stop.
        unsafe {
            WaitForSingleObject(self.stop.as_raw_handle(), 0) != WAIT_TIMEOUT
                || WaitForSingleObject(self.monitor.as_raw_handle(), 0) != WAIT_TIMEOUT
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cancellation::SessionStop;
    use std::{
        io::Read,
        os::windows::io::FromRawHandle,
        process::{Command, Stdio},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };
    use windows_sys::Win32::{Foundation::*, System::Threading::*};

    fn duplicate_wait(raw: HANDLE) -> OwnedHandle {
        let mut handle = std::ptr::null_mut();
        assert_ne!(
            unsafe {
                DuplicateHandle(
                    GetCurrentProcess(),
                    raw,
                    GetCurrentProcess(),
                    &mut handle,
                    SYNCHRONIZATION_SYNCHRONIZE,
                    0,
                    0,
                )
            },
            0
        );
        unsafe { OwnedHandle::from_raw_handle(handle) }
    }
    fn event() -> OwnedHandle {
        let handle = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        assert!(!handle.is_null());
        unsafe { OwnedHandle::from_raw_handle(handle) }
    }
    fn noninheritable(handle: &OwnedHandle) {
        let mut flags = 0;
        assert_ne!(
            unsafe { GetHandleInformation(handle.as_raw_handle(), &mut flags) },
            0
        );
        assert_eq!(flags & HANDLE_FLAG_INHERIT, 0);
    }
    #[test]
    fn native_event_is_authoritative_without_atomic_propagation() {
        let event = event();
        let witness = Arc::new(ProcessStop {
            monitor: duplicate_wait(unsafe { GetCurrentProcess() }),
            stop: duplicate_wait(event.as_raw_handle()),
        });
        noninheritable(&witness.monitor);
        noninheritable(&witness.stop);
        // The observer has wait-only event rights and cannot reset or signal it.
        assert_eq!(unsafe { SetEvent(witness.stop.as_raw_handle()) }, 0);
        let root = Arc::new(AtomicBool::new(false));
        let scope = SessionStop::monitored(root.clone(), witness);
        let sibling = SessionStop::child(root.clone());
        assert!(!scope.is_stopped());
        assert_ne!(unsafe { SetEvent(event.as_raw_handle()) }, 0);
        assert!(!root.load(Ordering::SeqCst)); // No watcher has run.
        assert!(scope.is_stopped());
        assert!(sibling.is_stopped());
        assert_ne!(unsafe { ResetEvent(event.as_raw_handle()) }, 0);
        assert!(scope.is_stopped());
    }

    #[test]
    fn monitor_process_fixture() {
        if std::env::var_os("PALISADE_STOP_WITNESS_FIXTURE").is_none() {
            return;
        }
        // No input, UI, identity, clipboard or network. EOF ends this process.
        let _ = std::io::stdin().read(&mut [0u8; 1]);
    }
    struct Fixture(std::process::Child);
    impl Drop for Fixture {
        fn drop(&mut self) {
            drop(self.0.stdin.take());
            // Reap only our fixed no-authority test on assertion failure.
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    #[test]
    fn original_monitor_exit_stops_even_when_native_event_remains_unsignaled() {
        let mut child = Fixture(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "windows::process_stop::tests::monitor_process_fixture",
                ])
                .env("PALISADE_STOP_WITNESS_FIXTURE", "1")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let event = event();
        let witness = ProcessStop {
            monitor: duplicate_wait(child.0.as_raw_handle()),
            stop: duplicate_wait(event.as_raw_handle()),
        };
        assert!(!witness.stopped());
        drop(child.0.stdin.take());
        assert!(child.0.wait().unwrap().success());
        assert_eq!(
            unsafe { WaitForSingleObject(event.as_raw_handle(), 0) },
            WAIT_TIMEOUT
        );
        assert!(witness.stopped());
    }
}
