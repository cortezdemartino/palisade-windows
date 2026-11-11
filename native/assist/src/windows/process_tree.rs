//! Native-owned, non-killing process accounting. Exit accounting is NOT input
//! cleanup proof. The monitor must also validate positive cleanup receipts.
use super::image::VerifiedImage;
use std::{
    ffi::OsString,
    fs::File,
    mem::{size_of, zeroed},
    os::windows::{
        ffi::OsStrExt,
        io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle},
    },
    ptr,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::SECURITY_ATTRIBUTES,
    System::{JobObjects::*, Pipes::CreatePipe, SystemServices::JOB_OBJECT_QUERY, Threading::*},
    UI::WindowsAndMessaging::SW_HIDE,
};

pub struct ProcessTree {
    job: OwnedHandle,
    admission_closed: bool,
    launched: bool,
}
pub struct TreeChild {
    process: OwnedHandle,
    pub input: File,
    pub output: File,
    pub error: File,
}
pub(crate) struct LaunchFailure {
    pub error: &'static str,
    pub child: Option<TreeChild>,
}
impl From<&'static str> for LaunchFailure {
    fn from(error: &'static str) -> Self {
        Self { error, child: None }
    }
}
impl std::fmt::Debug for LaunchFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.error)
    }
}
pub struct StopEvent {
    handle: OwnedHandle,
    signal_failed: std::sync::atomic::AtomicBool,
}
impl StopEvent {
    pub(crate) fn raw(&self) -> usize {
        self.handle.as_raw_handle() as usize
    }
    pub fn new() -> Result<Self, &'static str> {
        let raw = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if raw.is_null() {
            return Err("tree-stop-event");
        }
        Ok(Self {
            handle: unsafe { OwnedHandle::from_raw_handle(raw) },
            signal_failed: std::sync::atomic::AtomicBool::new(false),
        })
    }
    pub fn revoke(&self) -> Result<(), &'static str> {
        if unsafe { SetEvent(self.handle.as_raw_handle()) } == 0 {
            self.signal_failed
                .store(true, std::sync::atomic::Ordering::SeqCst);
            return Err("tree-stop-event");
        }
        Ok(())
    }
    pub(crate) fn signal_failed(&self) -> bool {
        self.signal_failed.load(std::sync::atomic::Ordering::SeqCst)
    }
    pub fn reader(&self) -> Result<OwnedHandle, &'static str> {
        let mut handle = ptr::null_mut();
        if unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                self.handle.as_raw_handle(),
                GetCurrentProcess(),
                &mut handle,
                SYNCHRONIZATION_SYNCHRONIZE,
                1,
                0,
            )
        } == 0
        {
            return Err("tree-stop-reader");
        }
        Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
    }
}
pub struct InheritedTree {
    _job: OwnedHandle,
    stop: OwnedHandle,
}
impl InheritedTree {
    pub(crate) fn stop_reader(&self) -> Result<OwnedHandle, &'static str> {
        let mut handle = ptr::null_mut();
        if unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                self.stop.as_raw_handle(),
                GetCurrentProcess(),
                &mut handle,
                SYNCHRONIZATION_SYNCHRONIZE,
                0,
                0,
            )
        } == 0
        {
            return Err("tree-stop-reader");
        }
        Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
    }
    /// Raw values occur only in the fixed native child launch, never page IPC.
    pub(crate) fn open(job: usize, stop: usize) -> Result<Self, &'static str> {
        if job == stop || [job, stop].iter().any(|&h| h == 0 || h == usize::MAX) {
            return Err("tree-handles");
        }
        for raw in [job, stop] {
            let mut flags = 0;
            if unsafe { GetHandleInformation(raw as HANDLE, &mut flags) } == 0 {
                return Err("tree-handles");
            }
        }
        let job = unsafe { OwnedHandle::from_raw_handle(job as HANDLE) };
        let stop = unsafe { OwnedHandle::from_raw_handle(stop as HANDLE) };
        for h in [&job, &stop] {
            if unsafe { SetHandleInformation(h.as_raw_handle(), HANDLE_FLAG_INHERIT, 0) } == 0 {
                return Err("tree-handles");
            }
        }
        let mut member = 0;
        if unsafe { IsProcessInJob(GetCurrentProcess(), job.as_raw_handle(), &mut member) } == 0
            || member == 0
        {
            return Err("tree-membership");
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        if unsafe {
            QueryInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ptr::null_mut(),
            )
        } == 0
            || limits.BasicLimitInformation.LimitFlags != 0
        {
            return Err("tree-policy");
        }
        let result = Self { _job: job, stop };
        if result.revoked()? {
            return Err("tree-stopped");
        }
        Ok(result)
    }
    pub fn revoked(&self) -> Result<bool, &'static str> {
        match unsafe { WaitForSingleObject(self.stop.as_raw_handle(), 0) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err("tree-stop-query"),
        }
    }
}
impl TreeChild {
    pub(crate) fn into_parts(self) -> (OwnedHandle, File, File, File) {
        (self.process, self.input, self.output, self.error)
    }
    pub fn process_handle(&self) -> BorrowedHandle<'_> {
        self.process.as_handle()
    }
    pub fn pid(&self) -> u32 {
        unsafe { GetProcessId(self.process.as_raw_handle()) }
    }
    pub fn exited(&self) -> Result<bool, &'static str> {
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err("tree-process-query"),
        }
    }
    pub fn exit_code(&self) -> Result<Option<u32>, &'static str> {
        if !self.exited()? {
            return Ok(None);
        }
        let mut code = 0;
        if unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &mut code) } == 0 {
            return Err("tree-process-query");
        }
        Ok(Some(code))
    }
}
impl ProcessTree {
    pub fn new() -> Result<Self, &'static str> {
        let raw = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if raw.is_null() {
            return Err("tree-create");
        }
        // SAFETY: unique successful CreateJobObject handle, never inherited.
        let job = unsafe { OwnedHandle::from_raw_handle(raw) };
        let tree = Self {
            job,
            admission_closed: false,
            launched: false,
        };
        tree.check_policy()?;
        Ok(tree)
    }
    fn check_policy(&self) -> Result<(), &'static str> {
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        if unsafe {
            QueryInformationJobObject(
                self.job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ptr::null_mut(),
            )
        } == 0
            || limits.BasicLimitInformation.LimitFlags != 0
        {
            return Err("tree-policy");
        }
        Ok(())
    }
    pub fn query_handle(&self) -> Result<OwnedHandle, &'static str> {
        let mut handle = ptr::null_mut();
        if unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                self.job.as_raw_handle(),
                GetCurrentProcess(),
                &mut handle,
                JOB_OBJECT_QUERY,
                1,
                0,
            )
        } == 0
        {
            return Err("tree-query-handle");
        }
        Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
    }
    pub fn contains(&self, process: BorrowedHandle<'_>) -> Result<bool, &'static str> {
        let mut member = 0;
        if unsafe {
            IsProcessInJob(
                process.as_raw_handle(),
                self.job.as_raw_handle(),
                &mut member,
            )
        } == 0
        {
            return Err("tree-membership");
        }
        Ok(member != 0)
    }
    pub fn close_admission(&mut self) {
        self.admission_closed = true;
    }
    /// Query the retained exact job, never job notifications or a PID scan.
    pub fn active_processes(&self) -> Result<u32, &'static str> {
        let mut count: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { zeroed() };
        if unsafe {
            QueryInformationJobObject(
                self.job.as_raw_handle(),
                JobObjectBasicAccountingInformation,
                (&mut count as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err("tree-accounting");
        }
        Ok(count.ActiveProcesses)
    }
    pub fn exited_after_admission_closed(&self) -> Result<bool, &'static str> {
        if !self.admission_closed || !self.launched {
            return Err("tree-admission-open");
        }
        Ok(self.active_processes()? == 0)
    }
    /// Internal fixed-image launcher; no page-facing executable or shell API.
    /// Job assignment is atomic with process creation on Windows 10/11. Failure
    /// has no unassigned-child fallback. The caller keeps VerifiedImage locked.
    pub fn launch(
        &mut self,
        image: &VerifiedImage,
        args: &[OsString],
        extra: &[BorrowedHandle<'_>],
    ) -> Result<TreeChild, &'static str> {
        self.launch_inner(image.path().as_os_str(), args, extra, None)
    }
    #[cfg(test)]
    pub(crate) fn launch_custody_fixture(
        &mut self,
        executor: bool,
        extra: &[BorrowedHandle<'_>],
    ) -> Result<TreeChild, LaunchFailure> {
        let fixture = if executor {
            "windows::custody_probe::tests::executor_fixture"
        } else {
            "windows::custody_probe::tests::coordinator_fixture"
        };
        self.launch_tracked(
            std::env::current_exe()
                .map_err(|_| "fixture-image")?
                .as_os_str(),
            &["--exact".into(), fixture.into(), "--nocapture".into()],
            extra,
            None,
        )
    }
    fn launch_inner(
        &mut self,
        image: &std::ffi::OsStr,
        args: &[OsString],
        extra: &[BorrowedHandle<'_>],
        environment: Option<&[u16]>,
    ) -> Result<TreeChild, &'static str> {
        self.launch_tracked(image, args, extra, environment)
            .map_err(|failure| failure.error)
    }
    pub(crate) fn launch_custody(
        &mut self,
        image: &VerifiedImage,
        args: &[OsString],
        extra: &[BorrowedHandle<'_>],
    ) -> Result<TreeChild, LaunchFailure> {
        self.launch_tracked(image.path().as_os_str(), args, extra, None)
    }
    fn launch_tracked(
        &mut self,
        image: &std::ffi::OsStr,
        args: &[OsString],
        extra: &[BorrowedHandle<'_>],
        environment: Option<&[u16]>,
    ) -> Result<TreeChild, LaunchFailure> {
        if self.admission_closed || args.len() > 16 || extra.len() > 8 {
            return Err("tree-admission-closed".into());
        }
        self.check_policy()?;
        let (child_in, parent_in) = pipe()?;
        let (parent_out, child_out) = pipe()?;
        let (parent_error, child_error) = pipe()?;
        for handle in [&parent_in, &parent_out, &parent_error] {
            if unsafe { SetHandleInformation(handle.as_raw_handle(), HANDLE_FLAG_INHERIT, 0) } == 0
            {
                return Err("tree-pipe".into());
            }
        }
        let mut inherited = vec![
            child_in.as_raw_handle(),
            child_out.as_raw_handle(),
            child_error.as_raw_handle(),
        ];
        for handle in extra {
            let handle = handle.as_raw_handle();
            let mut flags = 0;
            if handle.is_null()
                || handle == INVALID_HANDLE_VALUE
                || unsafe { GetHandleInformation(handle, &mut flags) } == 0
                || flags & HANDLE_FLAG_INHERIT == 0
                || inherited.contains(&handle)
            {
                return Err("tree-inherit".into());
            }
            inherited.push(handle);
        }
        let mut jobs = [self.job.as_raw_handle()];
        let mut attributes = Attributes::new(2)?;
        attributes.set(
            PROC_THREAD_ATTRIBUTE_JOB_LIST,
            jobs.as_mut_ptr().cast(),
            size_of::<HANDLE>(),
        )?;
        attributes.set(
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            inherited.as_mut_ptr().cast(),
            inherited.len() * size_of::<HANDLE>(),
        )?;
        let application = wide(image)?;
        let mut command = quote(image)?;
        for arg in args {
            command.push(b' ' as u16);
            command.extend(quote(arg)?);
        }
        if command.len() >= 32767 {
            return Err("tree-command".into());
        }
        command.push(0);
        let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES | STARTF_USESHOWWINDOW;
        startup.StartupInfo.wShowWindow = SW_HIDE as u16;
        startup.StartupInfo.hStdInput = child_in.as_raw_handle();
        startup.StartupInfo.hStdOutput = child_out.as_raw_handle();
        startup.StartupInfo.hStdError = child_error.as_raw_handle();
        startup.lpAttributeList = attributes.pointer();
        let mut info: PROCESS_INFORMATION = unsafe { zeroed() };
        let env = environment.map_or(ptr::null(), |v| v.as_ptr().cast());
        if unsafe {
            CreateProcessW(
                application.as_ptr(),
                command.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                1,
                EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW,
                env,
                ptr::null(),
                &startup.StartupInfo,
                &mut info,
            )
        } == 0
        {
            return Err("tree-launch".into());
        }
        // No kill-on-drop or kill-on-job-close: descendants retain cleanup.
        let process = unsafe { OwnedHandle::from_raw_handle(info.hProcess) };
        let _thread = unsafe { OwnedHandle::from_raw_handle(info.hThread) };
        self.launched = true;
        let child = TreeChild {
            process,
            input: parent_in.into(),
            output: parent_out.into(),
            error: parent_error.into(),
        };
        match self.contains(child.process.as_handle()) {
            Ok(true) => Ok(child),
            Ok(false) => Err(LaunchFailure {
                error: "tree-membership",
                child: Some(child),
            }),
            Err(error) => Err(LaunchFailure {
                error,
                child: Some(child),
            }),
        }
    }
}
fn wide(value: &std::ffi::OsStr) -> Result<Vec<u16>, &'static str> {
    let mut value: Vec<_> = value.encode_wide().collect();
    if value.contains(&0) {
        return Err("tree-command");
    }
    value.push(0);
    Ok(value)
}
/// Windows CRT argument quoting, without a command interpreter.
fn quote(value: &std::ffi::OsStr) -> Result<Vec<u16>, &'static str> {
    let value = wide(value)?;
    let mut result = vec![34];
    let mut slashes = 0;
    for &unit in &value[..value.len() - 1] {
        match unit {
            92 => slashes += 1,
            34 => {
                result.extend(std::iter::repeat_n(92, slashes * 2 + 1));
                result.push(34);
                slashes = 0;
            }
            _ => {
                result.extend(std::iter::repeat_n(92, slashes));
                slashes = 0;
                result.push(unit);
            }
        }
    }
    result.extend(std::iter::repeat_n(92, slashes * 2));
    result.push(34);
    Ok(result)
}
fn pipe() -> Result<(OwnedHandle, OwnedHandle), &'static str> {
    let security = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: 1,
    };
    let (mut read, mut write) = (ptr::null_mut(), ptr::null_mut());
    if unsafe { CreatePipe(&mut read, &mut write, &security, 0) } == 0 {
        return Err("tree-pipe");
    }
    Ok(unsafe {
        (
            OwnedHandle::from_raw_handle(read),
            OwnedHandle::from_raw_handle(write),
        )
    })
}
pub(crate) fn custody_pipe() -> Result<(OwnedHandle, OwnedHandle), &'static str> {
    pipe()
}
struct Attributes {
    storage: Vec<usize>,
    initialized: bool,
}
impl Attributes {
    fn pointer(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }
    fn new(count: u32) -> Result<Self, &'static str> {
        let mut bytes = 0;
        unsafe {
            InitializeProcThreadAttributeList(ptr::null_mut(), count, 0, &mut bytes);
        }
        if bytes == 0 || bytes > 65536 {
            return Err("tree-attributes");
        }
        let mut result = Self {
            storage: vec![0; bytes.div_ceil(size_of::<usize>())],
            initialized: false,
        };
        if unsafe { InitializeProcThreadAttributeList(result.pointer(), count, 0, &mut bytes) } == 0
        {
            // Failed initialization does not own a list to delete.
            return Err("tree-attributes");
        }
        result.initialized = true;
        Ok(result)
    }
    fn set(
        &mut self,
        kind: u32,
        value: *mut std::ffi::c_void,
        bytes: usize,
    ) -> Result<(), &'static str> {
        if unsafe {
            UpdateProcThreadAttribute(
                self.pointer(),
                0,
                kind as usize,
                value,
                bytes,
                ptr::null_mut(),
                ptr::null(),
            )
        } == 0
        {
            return Err("tree-attributes");
        }
        Ok(())
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                DeleteProcThreadAttributeList(self.pointer());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Command, Stdio},
        sync::mpsc,
        thread,
        time::Duration,
    };
    const FIXTURE: &str = "windows::process_tree::tests::tree_fixture";
    struct Signal(OwnedHandle);
    impl Drop for Signal {
        fn drop(&mut self) {
            unsafe {
                SetEvent(self.0.as_raw_handle());
            }
        }
    }
    fn event_name() -> String {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        format!(
            "Local\\Palisade.Tree.Fixture.{}.{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        )
    }
    fn fixture_child(tree: &mut ProcessTree, role: &str, stop: &str) -> TreeChild {
        let query = tree.query_handle().unwrap();
        let child = tree
            .launch_inner(
                std::env::current_exe().unwrap().as_os_str(),
                &["--exact".into(), FIXTURE.into(), "--nocapture".into()],
                &[query.as_handle()],
                None,
            )
            .unwrap();
        serde_json::to_writer(
            &child.input,
            &json!({"role":role,"stop":stop,"job":query.as_raw_handle() as usize}),
        )
        .unwrap();
        (&child.input).write_all(b"\n").unwrap();
        child
    }
    fn lines(child: &TreeChild) -> mpsc::Receiver<String> {
        let output = child.output.try_clone().unwrap();
        let (send, receive) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        receive
    }
    fn ready(receive: &mpsc::Receiver<String>, prefix: &str) -> u32 {
        loop {
            let line = receive.recv_timeout(Duration::from_secs(8)).unwrap();
            if let Some((_, value)) = line.split_once(prefix) {
                return value.trim().parse().unwrap();
            }
        }
    }
    fn wait_exit(child: &TreeChild) {
        for _ in 0..400 {
            if child.exited().unwrap() {
                assert_eq!(child.exit_code().unwrap(), Some(0));
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("fixture did not exit");
    }
    fn wait_empty(tree: &ProcessTree) {
        for _ in 0..400 {
            if tree.exited_after_admission_closed().unwrap() {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("job still owns descendants");
    }
    #[test]
    fn atomic_job_assignment_and_query_only_inheritance() {
        let name = event_name();
        let wide = wide(std::ffi::OsStr::new(&name)).unwrap();
        let signal = Signal(unsafe {
            OwnedHandle::from_raw_handle(CreateEventW(ptr::null(), 1, 0, wide.as_ptr()))
        });
        let mut tree = ProcessTree::new().unwrap();
        assert_eq!(tree.active_processes().unwrap(), 0);
        assert!(tree.exited_after_admission_closed().is_err());
        let child = fixture_child(&mut tree, "single", &name);
        let receive = lines(&child);
        assert_eq!(ready(&receive, "PALISADE-TREE-READY="), child.pid());
        assert!(tree.contains(child.process.as_handle()).unwrap());
        assert!(tree.active_processes().unwrap() >= 1);
        tree.close_admission();
        assert!(!tree.exited_after_admission_closed().unwrap());
        assert!(
            tree.launch_inner(std::env::current_exe().unwrap().as_os_str(), &[], &[], None)
                .is_err()
        );
        drop(signal);
        wait_exit(&child);
        wait_empty(&tree);
        assert!(tree.exited_after_admission_closed().unwrap());
    }
    #[test]
    fn descendant_outlives_coordinator_and_job_close_does_not_kill_it() {
        for close_job in [false, true] {
            let name = event_name();
            let wide = wide(std::ffi::OsStr::new(&name)).unwrap();
            let signal = Signal(unsafe {
                OwnedHandle::from_raw_handle(CreateEventW(ptr::null(), 1, 0, wide.as_ptr()))
            });
            let mut tree = ProcessTree::new().unwrap();
            let parent = fixture_child(&mut tree, "parent", &name);
            let receive = lines(&parent);
            let pid = ready(&receive, "PALISADE-TREE-DESCENDANT=");
            assert_eq!(ready(&receive, "PALISADE-TREE-HOLDER="), pid);
            let raw = unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                    0,
                    pid,
                )
            };
            assert!(!raw.is_null());
            let descendant = unsafe { OwnedHandle::from_raw_handle(raw) };
            wait_exit(&parent);
            assert!(tree.contains(descendant.as_handle()).unwrap());
            tree.close_admission();
            assert!(tree.active_processes().unwrap() >= 1);
            assert!(!tree.exited_after_admission_closed().unwrap());
            if close_job {
                drop(tree);
            } else {
                // Exit accounting only: it cannot be advertised as release proof.
                assert!(!tree.exited_after_admission_closed().unwrap());
            }
            assert_eq!(
                unsafe { WaitForSingleObject(descendant.as_raw_handle(), 0) },
                WAIT_TIMEOUT
            );
            drop(signal);
            assert_eq!(
                unsafe { WaitForSingleObject(descendant.as_raw_handle(), 5000) },
                WAIT_OBJECT_0
            );
        }
    }
    #[test]
    fn windows_command_arguments_are_quoted_without_a_shell() {
        for (arg, expected) in [
            ("", "\"\""),
            ("a b", "\"a b\""),
            ("a\"b", "\"a\\\"b\""),
            ("C:\\end\\", "\"C:\\end\\\\\""),
        ] {
            assert_eq!(
                String::from_utf16(&quote(std::ffi::OsStr::new(arg)).unwrap()).unwrap(),
                expected
            );
        }
        assert!(quote(std::ffi::OsStr::new("bad\0argument")).is_err());
    }
    #[test]
    fn tree_fixture() {
        if !std::env::args().any(|a| a == FIXTURE) {
            return;
        }
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
        let config: Value = serde_json::from_str(&line).unwrap();
        let stop_name = config["stop"].as_str().unwrap();
        if config["role"] == "parent" {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", FIXTURE, "--nocapture"])
                .stdin(Stdio::piped())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap();
            serde_json::to_writer(
                child.stdin.as_mut().unwrap(),
                &json!({"role":"descendant","stop":stop_name}),
            )
            .unwrap();
            child.stdin.as_mut().unwrap().write_all(b"\n").unwrap();
            println!("PALISADE-TREE-DESCENDANT={}", child.id());
            thread::spawn(move || child.wait().unwrap());
            // The fixture process exits without joining the descendant waiter;
            // the root test retains and awaits the descendant process handle.
            return;
        }
        if config["role"] == "single" {
            let job = config["job"].as_u64().unwrap() as HANDLE;
            let mut member = 0;
            assert_ne!(
                unsafe { IsProcessInJob(GetCurrentProcess(), job, &mut member) },
                0
            );
            assert_eq!(member, 1);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
            assert_ne!(
                unsafe {
                    QueryInformationJobObject(
                        job,
                        JobObjectExtendedLimitInformation,
                        (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                        size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                        ptr::null_mut(),
                    )
                },
                0
            );
            assert_eq!(limits.BasicLimitInformation.LimitFlags, 0);
            assert_eq!(
                unsafe {
                    SetInformationJobObject(
                        job,
                        JobObjectExtendedLimitInformation,
                        (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                        size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                    )
                },
                0
            );
        }
        let name = wide(std::ffi::OsStr::new(stop_name)).unwrap();
        let raw = unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr()) };
        assert!(!raw.is_null());
        let event = unsafe { OwnedHandle::from_raw_handle(raw) };
        println!(
            "{}{}",
            if config["role"] == "single" {
                "PALISADE-TREE-READY="
            } else {
                "PALISADE-TREE-HOLDER="
            },
            std::process::id()
        );
        assert_eq!(
            unsafe { WaitForSingleObject(event.as_raw_handle(), 5000) },
            WAIT_OBJECT_0
        );
    }
}
