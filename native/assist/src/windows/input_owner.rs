//! Shared kernel-object lifetime, retained by guardian and executor through cleanup.
//! An untrusted local process can deny service by holding the name, never authorize input.
use std::ptr;
use windows_sys::Win32::{
    Foundation::*,
    System::{RemoteDesktop::ProcessIdToSessionId, Threading::*},
};

pub struct InputOwner(HANDLE);
impl InputOwner {
    fn name() -> Result<String, &'static str> {
        let mut session = 0;
        if unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) } == 0 || session == 0
        {
            return Err("session-unavailable");
        }
        Ok(format!("Local\\Palisade.Assist.Input.v2.{session}"))
    }
    pub(crate) fn acquire() -> Result<Self, &'static str> {
        Self::named(&Self::name()?)
    }
    fn named(name: &str) -> Result<Self, &'static str> {
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        unsafe {
            SetLastError(0);
            let handle = CreateEventW(ptr::null(), 1, 0, name.as_ptr());
            if handle.is_null() {
                return Err("input-owner-unavailable");
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(handle);
                return Err("controller-busy");
            }
            Ok(Self(handle))
        }
    }
    pub(crate) fn raw(&self) -> usize {
        self.0 as usize
    }
    /// Only called after Parent::validate_executor. The expected name is fixed.
    pub(crate) fn from_parent(
        parent: &super::parent::Parent,
        raw: usize,
    ) -> Result<Self, &'static str> {
        if !parent.alive() {
            return Err("parent-ended");
        }
        let owner = Self::duplicate_named(parent.pid(), raw, &Self::name()?)?;
        if !parent.alive() {
            return Err("parent-ended");
        }
        Ok(owner)
    }
    fn duplicate_named(parent: u32, raw: usize, name: &str) -> Result<Self, &'static str> {
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        unsafe {
            let process = OpenProcess(PROCESS_DUP_HANDLE, 0, parent);
            if process.is_null() {
                return Err("input-owner-parent");
            }
            let mut duplicate = ptr::null_mut();
            let ok = DuplicateHandle(
                process,
                raw as HANDLE,
                GetCurrentProcess(),
                &mut duplicate,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            );
            CloseHandle(process);
            if ok == 0 {
                return Err("input-owner-duplicate");
            }
            let owner = Self(duplicate);
            // Open only: never manufacture a replacement for an expired owner.
            let expected = OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr());
            if expected.is_null() {
                return Err("input-owner-missing");
            }
            let same = CompareObjectHandles(owner.0, expected) != 0;
            CloseHandle(expected);
            if !same {
                return Err("input-owner-mismatch");
            }
            Ok(owner)
        }
    }
}
impl Drop for InputOwner {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        process::{Child, Command, Stdio},
        sync::mpsc,
        thread,
        time::Duration,
    };
    const FIXTURE: &str = "windows::input_owner::tests::owner_process_fixture";
    struct ChildGuard(Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    struct ProcessGuard(HANDLE);
    impl Drop for ProcessGuard {
        fn drop(&mut self) {
            unsafe {
                TerminateProcess(self.0, 0);
                WaitForSingleObject(self.0, 3000);
                CloseHandle(self.0);
            }
        }
    }
    #[test]
    fn every_existing_object_blocks_even_in_the_same_thread_and_if_signaled() {
        let name = format!("Local\\Palisade.Assist.Owner.Test.{}", std::process::id());
        let owner = InputOwner::named(&name).unwrap();
        assert!(InputOwner::named(&name).is_err());
        unsafe {
            SetEvent(owner.0);
        }
        assert!(InputOwner::named(&name).is_err());
        // A wrong object cannot be substituted for the expected ownership handle.
        let other = InputOwner::named(&(name.clone() + ".other")).unwrap();
        assert!(InputOwner::duplicate_named(std::process::id(), other.raw(), &name).is_err());
        let duplicate =
            InputOwner::duplicate_named(std::process::id(), owner.raw(), &name).unwrap();
        drop(owner);
        assert!(InputOwner::named(&name).is_err());
        drop(duplicate);
        assert!(InputOwner::named(&name).is_ok());
    }
    #[test]
    fn guardian_death_cannot_admit_a_successor_until_child_cleanup_exits() {
        let name = format!("Local\\Palisade.Assist.Crash.Test.{}", std::process::id());
        let mut parent = ChildGuard(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", FIXTURE, "--nocapture"])
                .env("PALISADE_OWNER_FIXTURE", "parent")
                .env("PALISADE_OWNER_NAME", &name)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let stdout = parent.0.stdout.take().unwrap();
        let (send, receive) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        let (mut child, mut ready) = (None, false);
        while child.is_none() || !ready {
            let line = receive.recv_timeout(Duration::from_secs(5)).unwrap();
            if let Some((_prefix, pid)) = line.split_once("PALISADE-OWNER-CHILD=") {
                let process = unsafe {
                    OpenProcess(
                        PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
                        0,
                        pid.parse().unwrap(),
                    )
                };
                assert!(!process.is_null());
                child = Some(ProcessGuard(process));
            }
            if line.contains("PALISADE-OWNER-READY") {
                ready = true;
            }
        }
        parent.0.kill().unwrap();
        parent.0.wait().unwrap();
        for _ in 0..5 {
            assert!(InputOwner::named(&name).is_err());
            thread::sleep(Duration::from_millis(10));
        }
        drop(child);
        assert!(InputOwner::named(&name).is_ok());
    }
    #[test]
    fn owner_process_fixture() {
        let Ok(role) = std::env::var("PALISADE_OWNER_FIXTURE") else {
            return;
        };
        let name = std::env::var("PALISADE_OWNER_NAME").unwrap();
        if role == "parent" {
            let owner = InputOwner::named(&name).unwrap();
            let child = ChildGuard(
                Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", FIXTURE, "--nocapture"])
                    .env("PALISADE_OWNER_FIXTURE", "child")
                    .env("PALISADE_OWNER_PARENT", std::process::id().to_string())
                    .env("PALISADE_OWNER_HANDLE", owner.raw().to_string())
                    .stdin(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            println!("PALISADE-OWNER-CHILD={}", child.0.id());
            thread::sleep(Duration::from_secs(20));
        } else {
            let _owner = InputOwner::duplicate_named(
                std::env::var("PALISADE_OWNER_PARENT")
                    .unwrap()
                    .parse()
                    .unwrap(),
                std::env::var("PALISADE_OWNER_HANDLE")
                    .unwrap()
                    .parse()
                    .unwrap(),
                &name,
            )
            .unwrap();
            println!("PALISADE-OWNER-READY");
            thread::sleep(Duration::from_secs(20)); // Simulates cleanup-only retention; never injects input.
        }
    }
}
