use std::{
    mem::{size_of, zeroed},
    path::PathBuf,
    ptr,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::*,
    Storage::FileSystem::{FILE_TYPE_PIPE, GetFileType},
    System::{
        Console::{GetStdHandle, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE},
        Diagnostics::ToolHelp::*,
        RemoteDesktop::ProcessIdToSessionId,
        Threading::*,
    },
};

pub struct Parent(HANDLE);
// SAFETY: immutable process handle, owned until Drop, only queried/waited on.
unsafe impl Send for Parent {}
impl Drop for Parent {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
impl Parent {
    pub fn alive(&self) -> bool {
        unsafe { WaitForSingleObject(self.0, 0) == WAIT_TIMEOUT }
    }
    pub fn validate(expected_pid: u32, probe: bool) -> Result<Self, &'static str> {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return Err("parent-snapshot");
            }
            let mut entry: PROCESSENTRY32W = zeroed();
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            let own_pid = GetCurrentProcessId();
            let mut parent_pid = 0;
            let mut more = Process32FirstW(snapshot, &mut entry);
            while more != 0 {
                if entry.th32ProcessID == own_pid {
                    parent_pid = entry.th32ParentProcessID;
                    break;
                }
                more = Process32NextW(snapshot, &mut entry);
            }
            CloseHandle(snapshot);
            if parent_pid == 0 || parent_pid != expected_pid {
                return Err("parent-binding");
            }
            let handle = OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                parent_pid,
            );
            if handle.is_null() {
                return Err("parent-open");
            }
            let parent = Self(handle);
            if creation_time(handle)? > creation_time(GetCurrentProcess())? {
                return Err("parent-reused");
            }
            let mut parent_session = 0;
            let mut own_session = 0;
            if ProcessIdToSessionId(parent_pid, &mut parent_session) == 0
                || ProcessIdToSessionId(own_pid, &mut own_session) == 0
                || parent_session != own_session
                || own_session == 0
            {
                return Err("session-binding");
            }
            if !same_user_unelevated(handle)? {
                return Err("user-binding");
            }
            if !probe {
                let mut image = [0u16; 32768];
                let mut length = image.len() as u32;
                if QueryFullProcessImageNameW(handle, 0, image.as_mut_ptr(), &mut length) == 0 {
                    return Err("parent-image");
                }
                let expected = std::env::current_exe()
                    .map_err(|_| "parent-image")?
                    .parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.parent())
                    .ok_or("parent-image")?
                    .join("Palisade.exe")
                    .canonicalize()
                    .map_err(|_| "parent-image")?;
                let image = PathBuf::from(
                    String::from_utf16(&image[..length as usize]).map_err(|_| "parent-image")?,
                )
                .canonicalize()
                .map_err(|_| "parent-image")?;
                if !image
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&expected.to_string_lossy())
                {
                    return Err("parent-image");
                }
            }
            for kind in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE] {
                let pipe = GetStdHandle(kind);
                if pipe.is_null()
                    || pipe == INVALID_HANDLE_VALUE
                    || GetFileType(pipe) != FILE_TYPE_PIPE
                    || SetHandleInformation(pipe, HANDLE_FLAG_INHERIT, 0) == 0
                {
                    return Err("private-pipe-required");
                }
            }
            if !parent.alive() {
                return Err("parent-ended");
            }
            Ok(parent)
        }
    }
}

unsafe fn creation_time(process: HANDLE) -> Result<u64, &'static str> {
    let (mut created, mut exited, mut kernel, mut user): (FILETIME, FILETIME, FILETIME, FILETIME) =
        unsafe { zeroed() };
    if unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
        return Err("parent-time");
    }
    Ok((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}

unsafe fn token(process: HANDLE) -> Result<HANDLE, &'static str> {
    let mut handle = ptr::null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut handle) } == 0 {
        return Err("token-open");
    }
    Ok(handle)
}
unsafe fn token_info(
    handle: HANDLE,
    kind: TOKEN_INFORMATION_CLASS,
) -> Result<Vec<usize>, &'static str> {
    let mut length = 0;
    unsafe {
        GetTokenInformation(handle, kind, ptr::null_mut(), 0, &mut length);
    }
    if length == 0 || length > 16384 {
        return Err("token-size");
    }
    let mut storage = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            handle,
            kind,
            storage.as_mut_ptr().cast(),
            length,
            &mut length,
        )
    } == 0
    {
        return Err("token-info");
    }
    Ok(storage)
}
unsafe fn same_user_unelevated(parent: HANDLE) -> Result<bool, &'static str> {
    struct Token(HANDLE);
    impl Drop for Token {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    unsafe {
        let own = Token(token(GetCurrentProcess())?);
        let parent = Token(token(parent)?);
        for handle in [own.0, parent.0] {
            let elevation = token_info(handle, TokenElevation)?;
            if (*(elevation.as_ptr().cast::<TOKEN_ELEVATION>())).TokenIsElevated != 0 {
                return Ok(false);
            }
        }
        let own = token_info(own.0, TokenUser)?;
        let parent = token_info(parent.0, TokenUser)?;
        Ok(EqualSid(
            (*(own.as_ptr().cast::<TOKEN_USER>())).User.Sid,
            (*(parent.as_ptr().cast::<TOKEN_USER>())).User.Sid,
        ) != 0)
    }
}
