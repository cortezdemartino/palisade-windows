//! Authenticate an actual installed Palisade creator, then retain the exact process object.
use super::{
    image,
    stage::{Directories, Handle, lock_regular, same_file},
};
use std::{
    fs::File,
    mem::{size_of, zeroed},
    path::{Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::*,
    Storage::FileSystem::{FILE_TYPE_PIPE, GetFileType},
    System::{
        Console::{GetStdHandle, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE},
        Diagnostics::ToolHelp::*,
        Pipes::*,
        Registry::*,
        RemoteDesktop::ProcessIdToSessionId,
        Threading::*,
    },
};
type Result<T> = std::result::Result<T, &'static str>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    User,
    Machine,
}
pub struct Parent {
    process: Handle,
    image: PathBuf,
    image_lock: Option<File>,
    directory: PathBuf,
    directories: Directories,
    scope: Option<Scope>,
}

fn token_info(process: HANDLE, kind: TOKEN_INFORMATION_CLASS) -> Result<Vec<usize>> {
    let mut raw = ptr::null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut raw) } == 0 {
        return Err("parent-token");
    }
    let token = Handle(raw);
    let mut length = 0;
    unsafe {
        GetTokenInformation(token.0, kind, ptr::null_mut(), 0, &mut length);
    }
    if length == 0 || length > 16384 {
        return Err("parent-token");
    }
    let mut buffer = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            token.0,
            kind,
            buffer.as_mut_ptr().cast(),
            length,
            &mut length,
        )
    } == 0
    {
        return Err("parent-token");
    }
    Ok(buffer)
}

fn same_user(process: HANDLE) -> Result<()> {
    let own = unsafe { GetCurrentProcess() };
    for process in [own, process] {
        let info = token_info(process, TokenElevation)?;
        if unsafe { (*info.as_ptr().cast::<TOKEN_ELEVATION>()).TokenIsElevated } != 0 {
            return Err("parent-elevation");
        }
    }
    let own = token_info(own, TokenUser)?;
    let parent = token_info(process, TokenUser)?;
    if unsafe {
        EqualSid(
            (*own.as_ptr().cast::<TOKEN_USER>()).User.Sid,
            (*parent.as_ptr().cast::<TOKEN_USER>()).User.Sid,
        )
    } == 0
    {
        return Err("parent-user");
    }
    Ok(())
}

fn created(process: HANDLE) -> Result<u64> {
    let mut times: [FILETIME; 4] = unsafe { zeroed() };
    if unsafe {
        GetProcessTimes(
            process,
            &mut times[0],
            &mut times[1],
            &mut times[2],
            &mut times[3],
        )
    } == 0
    {
        return Err("parent-creation");
    }
    Ok((u64::from(times[0].dwHighDateTime) << 32) | u64::from(times[0].dwLowDateTime))
}

fn actual_parent() -> Result<u32> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err("parent-snapshot");
    }
    let snapshot = Handle(snapshot);
    let mut entry: PROCESSENTRY32W = unsafe { zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut more = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    while more != 0 {
        if entry.th32ProcessID == unsafe { GetCurrentProcessId() } {
            return Ok(entry.th32ParentProcessID);
        }
        more = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    Err("parent-snapshot")
}

fn registered_string(hive: HKEY, key: &str, value: Option<&str>) -> Result<Option<String>> {
    let key: Vec<u16> = key.encode_utf16().chain(Some(0)).collect();
    let value: Option<Vec<u16>> = value.map(|s| s.encode_utf16().chain(Some(0)).collect());
    let value = value.as_ref().map_or(ptr::null(), |value| value.as_ptr());
    let flags = RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY;
    let mut length = 0;
    let result = unsafe {
        RegGetValueW(
            hive,
            key.as_ptr(),
            value,
            flags,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut length,
        )
    };
    if result == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if result != ERROR_SUCCESS || !(2..=65536).contains(&length) || length % 2 != 0 {
        return Err("installation-registry");
    }
    let mut buffer = vec![0u16; length as usize / 2];
    if unsafe {
        RegGetValueW(
            hive,
            key.as_ptr(),
            value,
            flags,
            ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut length,
        )
    } != ERROR_SUCCESS
        || length < 2
        || length as usize > buffer.len() * 2
        || length % 2 != 0
    {
        return Err("installation-registry");
    }
    buffer.truncate(length as usize / 2);
    if buffer.pop() != Some(0) || buffer.is_empty() || buffer.contains(&0) {
        return Err("installation-registry");
    }
    Ok(Some(
        String::from_utf16(&buffer).map_err(|_| "installation-registry")?,
    ))
}

fn registered_image_matches(
    executable: &str,
    image: &File,
    directory: &Directories,
) -> Result<bool> {
    let path = Path::new(executable);
    let Some(parent) = path.parent() else {
        return Ok(false);
    };
    let Ok(held) = Directories::hold(parent) else {
        return Ok(false);
    };
    if !held.same_location(directory)? {
        return Ok(false);
    }
    let Ok(registered) = lock_regular(path) else {
        return Ok(false);
    };
    same_file(image, &registered)
}

#[derive(Clone)]
struct Registration {
    scope: Scope,
    executable: Option<String>,
    folder: Option<String>,
    display_name: Option<String>,
    publisher: Option<String>,
    version: Option<String>,
}

fn select_scope(
    records: &[Registration; 2],
    image: &File,
    directory: &Directories,
    version: &str,
) -> Result<Scope> {
    if records[0].scope != Scope::User || records[1].scope != Scope::Machine {
        return Err("installation-scope");
    }
    let mut matches = Vec::new();
    let mut app_path_matches = false;
    for record in records {
        if let Some(executable) = &record.executable {
            app_path_matches |= registered_image_matches(executable, image, directory)?;
        }
        if record.display_name.as_deref() != Some("Palisade")
            || record.publisher.as_deref() != Some("Onyx Operations Group")
            || record.version.as_deref() != Some(version)
        {
            continue;
        }
        let Some(folder) = &record.folder else {
            continue;
        };
        let Ok(registered_directory) = Directories::hold(Path::new(folder)) else {
            continue;
        };
        if registered_directory.same_location(directory)? {
            matches.push(record.scope);
        }
    }
    if matches.len() != 1 || !app_path_matches {
        return Err("installation-scope");
    }
    Ok(matches[0])
}

fn scope(image: &File, directory: &Directories) -> Result<Scope> {
    const APP_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\App Paths\\Palisade.exe";
    let uninstall = format!(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}",
        crate::UNINSTALL_GUID
    );
    let read = |hive, scope| -> Result<Registration> {
        // Start-menu repair may register HKCU App Paths for a machine install.
        // App Paths corroborates the image; the fixed NSIS uninstall key owns scope.
        Ok(Registration {
            scope,
            executable: registered_string(hive, APP_PATH, None)?,
            folder: registered_string(hive, &uninstall, Some("InstallLocation"))?,
            display_name: registered_string(hive, &uninstall, Some("DisplayName"))?,
            publisher: registered_string(hive, &uninstall, Some("Publisher"))?,
            version: registered_string(hive, &uninstall, Some("DisplayVersion"))?,
        })
    };
    let records = [
        read(HKEY_CURRENT_USER, Scope::User)?,
        read(HKEY_LOCAL_MACHINE, Scope::Machine)?,
    ];
    select_scope(&records, image, directory, &crate::packaged_version()?)
}

pub(crate) fn private_pipes(parent: u32) -> Result<()> {
    for kind in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE] {
        let pipe = unsafe { GetStdHandle(kind) };
        if pipe.is_null()
            || pipe == INVALID_HANDLE_VALUE
            || unsafe { GetFileType(pipe) } != FILE_TYPE_PIPE
        {
            return Err("parent-private-pipe");
        }
        let mut server = 0;
        let mut client = 0;
        let server_known = unsafe { GetNamedPipeServerProcessId(pipe, &mut server) } != 0;
        let client_known = unsafe { GetNamedPipeClientProcessId(pipe, &mut client) } != 0;
        if !(server_known && server == parent || client_known && client == parent) {
            return Err("parent-pipe-owner");
        }
        if unsafe { SetHandleInformation(pipe, HANDLE_FLAG_INHERIT, 0) } == 0 {
            return Err("parent-private-pipe");
        }
    }
    Ok(())
}

impl Parent {
    pub fn authenticate(expected: u32) -> Result<Self> {
        Self::bind(expected, true)
    }
    pub(crate) fn authenticate_status(expected: u32) -> Result<Self> {
        Self::bind(expected, false)
    }
    fn bind(expected: u32, installation: bool) -> Result<Self> {
        if expected == 0 || actual_parent()? != expected {
            return Err("parent-creator");
        }
        let raw = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                expected,
            )
        };
        if raw.is_null() {
            return Err("parent-open");
        }
        let process = Handle(raw);
        if created(process.0)? > created(unsafe { GetCurrentProcess() })? {
            return Err("parent-reused");
        }
        same_user(process.0)?;
        let mut own_session = 0;
        let mut parent_session = 0;
        if unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut own_session) } == 0
            || unsafe { ProcessIdToSessionId(expected, &mut parent_session) } == 0
            || own_session == 0
            || own_session != parent_session
        {
            return Err("parent-session");
        }
        let mut buffer = vec![0u16; 32768];
        let mut length = buffer.len() as u32;
        if unsafe { QueryFullProcessImageNameW(process.0, 0, buffer.as_mut_ptr(), &mut length) }
            == 0
        {
            return Err("parent-image");
        }
        let image = PathBuf::from(
            String::from_utf16(&buffer[..length as usize]).map_err(|_| "parent-image")?,
        );
        if !image
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("Palisade.exe"))
        {
            return Err("parent-image");
        }
        let directory = image.parent().ok_or("parent-image")?.to_owned();
        let directories = Directories::hold(&directory)?;
        let image_lock = lock_regular(&image)?;
        image::validate(&image, &crate::packaged_version()?)?;
        let scope = if installation {
            Some(scope(&image_lock, &directories)?)
        } else {
            None
        };
        private_pipes(expected)?;
        let parent = Self {
            process,
            image,
            directory,
            directories,
            image_lock: Some(image_lock),
            scope,
        };
        if !parent.alive() {
            return Err("parent-ended");
        }
        Ok(parent)
    }
    pub fn alive(&self) -> bool {
        unsafe { WaitForSingleObject(self.process.0, 0) == WAIT_TIMEOUT }
    }
    pub fn image(&self) -> &Path {
        &self.image
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn scope(&self) -> Result<Scope> {
        self.scope.ok_or("installation-not-authorized")
    }
    pub fn directory_guard(&self) -> &Directories {
        &self.directories
    }
    pub fn wait_for_exit(&mut self, milliseconds: u32) -> Result<()> {
        if unsafe { WaitForSingleObject(self.process.0, milliseconds) } != WAIT_OBJECT_0 {
            return Err("parent-exit-pending");
        }
        // The exact process object remains retained, but installation must replace its image.
        self.image_lock.take();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scope_fixtures_preserve_machine_identity_despite_user_app_paths_and_reject_conflicts() {
        let base = crate::windows::stage::tests::fixture();
        let path = base.join("Palisade.exe");
        std::fs::write(&path, b"metadata fixture, not executable").unwrap();
        let directory = Directories::hold(&base).unwrap();
        let image = lock_regular(&path).unwrap();
        let record = |scope| Registration {
            scope,
            executable: Some(path.to_str().unwrap().into()),
            folder: Some(base.to_str().unwrap().into()),
            display_name: Some("Palisade".into()),
            publisher: Some("Onyx Operations Group".into()),
            version: Some("0.3.3".into()),
        };
        let absent = |scope| Registration {
            scope,
            executable: None,
            folder: None,
            display_name: None,
            publisher: None,
            version: None,
        };
        let user = record(Scope::User);
        let machine = record(Scope::Machine);
        assert_eq!(
            select_scope(
                &[user.clone(), absent(Scope::Machine)],
                &image,
                &directory,
                "0.3.3"
            ),
            Ok(Scope::User)
        );
        let mut app_path_only = absent(Scope::User);
        app_path_only.executable = user.executable.clone();
        assert_eq!(
            select_scope(
                &[app_path_only, machine.clone()],
                &image,
                &directory,
                "0.3.3"
            ),
            Ok(Scope::Machine)
        );
        assert!(select_scope(&[user.clone(), machine], &image, &directory, "0.3.3").is_err());
        for delta in ["version", "publisher", "directory", "image"] {
            let mut invalid = user.clone();
            match delta {
                "version" => invalid.version = Some("0.3.2".into()),
                "publisher" => invalid.publisher = Some("Other".into()),
                "directory" => invalid.folder = Some(base.join("missing").to_str().unwrap().into()),
                _ => invalid.executable = Some(base.join("missing.exe").to_str().unwrap().into()),
            }
            assert!(
                select_scope(
                    &[invalid, absent(Scope::Machine)],
                    &image,
                    &directory,
                    "0.3.3"
                )
                .is_err()
            );
        }
        drop(image);
        drop(directory);
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn pipe_owner_is_bound_to_the_actual_node_creator() {
        if std::env::var("PALISADE_UPDATE_PIPE_CHILD").as_deref() == Ok("yes") {
            let expected = std::env::var("PALISADE_UPDATE_PIPE_PARENT")
                .unwrap()
                .parse()
                .unwrap();
            assert_eq!(actual_parent().unwrap(), expected);
            assert!(private_pipes(expected).is_ok());
            assert!(private_pipes(unsafe { GetCurrentProcessId() }).is_err());
            return;
        }
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pipes.cjs");
        let result = std::process::Command::new("node")
            .arg(script)
            .arg(std::env::current_exe().unwrap())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    #[test]
    fn updater_rejects_self_missing_and_wrong_creator_without_authority() {
        assert!(Parent::authenticate(0).is_err());
        assert!(Parent::authenticate(unsafe { GetCurrentProcessId() }).is_err());
        let parent = actual_parent().unwrap();
        assert!(
            Parent::authenticate(parent).is_err(),
            "cargo/node are not the installed Palisade parent"
        );
    }
}
