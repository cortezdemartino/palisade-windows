//! Private, reparse-resistant staging. Handles retain directory identity through handoff.
use std::{
    ffi::OsString,
    fs::File,
    mem::{size_of, zeroed},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        io::{AsRawHandle, FromRawHandle},
    },
    path::{Component, Path, PathBuf, Prefix},
    ptr,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    Storage::FileSystem::*,
    System::{Com::CoTaskMemFree, Threading::*},
    UI::Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath},
};

type Result<T> = std::result::Result<T, &'static str>;
use windows_sys::Wdk::Storage::FileSystem::{FILE_CREATE, FILE_OPEN, FILE_OVERWRITE_IF};
pub(crate) fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
pub(crate) struct Handle(pub HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub struct Directories {
    _files: Vec<File>,
}
impl Directories {
    fn overlaps(&self, other: &Self) -> Result<bool> {
        for file in &self._files {
            if same_file(file, other.last()?)? {
                return Ok(true);
            }
        }
        for file in &other._files {
            if same_file(file, self.last()?)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(crate) fn same_location(&self, other: &Self) -> Result<bool> {
        same_file(self.last()?, other.last()?)
    }
    pub fn hold(path: &Path) -> Result<Self> {
        Self::open(path, false)
    }
    /// Seal updater-owned directories against in-place reparse mutation. This also
    /// prevents child rename and must never be used for an installation directory.
    pub fn seal(path: &Path) -> Result<Self> {
        if path.components().count() < 3 {
            return Err("stage-location");
        }
        Self::open(path, true)
    }
    fn open(path: &Path, sealed: bool) -> Result<Self> {
        let mut parts = path.components();
        if !matches!(parts.next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_)))
            || !matches!(parts.next(), Some(Component::RootDir))
        {
            return Err("local-absolute-path-required");
        }
        let mut current = PathBuf::new();
        let mut files = Vec::new();
        for part in path.components() {
            if matches!(part, Component::CurDir | Component::ParentDir) {
                return Err("unsafe-path-component");
            }
            current.push(part);
            if matches!(part, Component::Prefix(_)) {
                continue;
            }
            let name = wide(&current);
            let leaf = sealed && current == path;
            // Ancestors retain no-delete identity without blocking unrelated file replacement.
            // Only the updater-owned leaf denies directory data mutation.
            let raw = unsafe {
                CreateFileW(
                    name.as_ptr(),
                    FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | READ_CONTROL,
                    FILE_SHARE_READ | if leaf { 0 } else { FILE_SHARE_WRITE },
                    ptr::null(),
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                    ptr::null_mut(),
                )
            };
            if raw == INVALID_HANDLE_VALUE {
                return Err("stage-directory-open");
            }
            let file = unsafe { File::from_raw_handle(raw) };
            let info = information(&file)?;
            if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
                || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            {
                return Err("stage-directory-reparse");
            }
            files.push(file);
        }
        Ok(Self { _files: files })
    }
    fn last(&self) -> Result<&File> {
        self._files.last().ok_or("stage-directory-open")
    }
}

fn information(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION> {
    let mut info = unsafe { zeroed() };
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err("stage-file-info");
    }
    Ok(info)
}

pub(crate) fn same_file(left: &File, right: &File) -> Result<bool> {
    let left = information(left)?;
    let right = information(right)?;
    Ok((
        left.dwVolumeSerialNumber,
        left.nFileIndexHigh,
        left.nFileIndexLow,
    ) == (
        right.dwVolumeSerialNumber,
        right.nFileIndexHigh,
        right.nFileIndexLow,
    ))
}

pub(crate) fn lock_regular(path: &Path) -> Result<File> {
    let path = wide(path);
    let raw = unsafe {
        CreateFileW(
            path.as_ptr(),
            GENERIC_READ | READ_CONTROL,
            FILE_SHARE_READ,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err("stage-file-open");
    }
    let file = unsafe { File::from_raw_handle(raw) };
    let info = information(&file)?;
    if info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0
        || info.nNumberOfLinks != 1
    {
        return Err("stage-file-reparse");
    }
    Ok(file)
}

struct User {
    _token: Handle,
    words: Vec<usize>,
}
pub(crate) fn user_sid() -> Result<String> {
    User::current()?.text()
}
impl User {
    fn current() -> Result<Self> {
        let mut raw = ptr::null_mut();
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } == 0 {
            return Err("stage-user");
        }
        let token = Handle(raw);
        let mut size = 0;
        unsafe {
            GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut size);
        }
        if size == 0 || size > 16384 {
            return Err("stage-user");
        }
        let mut words = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        if unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                words.as_mut_ptr().cast(),
                size,
                &mut size,
            )
        } == 0
        {
            return Err("stage-user");
        }
        Ok(Self {
            _token: token,
            words,
        })
    }
    fn sid(&self) -> PSID {
        unsafe { (*self.words.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }
    fn text(&self) -> Result<String> {
        let mut raw = ptr::null_mut();
        if unsafe { ConvertSidToStringSidW(self.sid(), &mut raw) } == 0 {
            return Err("stage-user");
        }
        let mut length = 0;
        unsafe {
            while *raw.add(length) != 0 {
                length += 1;
            }
        }
        let result = unsafe { String::from_utf16(std::slice::from_raw_parts(raw, length)) };
        unsafe {
            LocalFree(raw.cast());
        }
        result.map_err(|_| "stage-user")
    }
}

fn create_private(path: &Path, user: &User, allow_existing: bool) -> Result<()> {
    let sid = user.text()?;
    let sddl: Vec<u16> = format!("O:{sid}D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;{sid})")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut descriptor = ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err("stage-security");
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    let name = wide(path);
    let made = unsafe { CreateDirectoryW(name.as_ptr(), &attributes) };
    let error = unsafe { GetLastError() };
    unsafe {
        LocalFree(descriptor);
    }
    if made == 0 && (!allow_existing || error != ERROR_ALREADY_EXISTS) {
        return Err("stage-create");
    }
    Ok(())
}

fn allowed_sid(ace: &[u8]) -> Result<PSID> {
    if ace.len() < 16
        || ace[0] != 0
        || ace[1] != (OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE) as u8
        || u16::from_le_bytes([ace[2], ace[3]]) as usize != ace.len()
        || u32::from_le_bytes(ace[4..8].try_into().map_err(|_| "stage-security")?)
            != FILE_ALL_ACCESS
    {
        return Err("stage-security");
    }
    let sid = &ace[8..];
    if sid[0] != 1 || sid[1] > 15 || 8 + 4 * sid[1] as usize != sid.len() {
        return Err("stage-security");
    }
    let raw = sid.as_ptr().cast_mut().cast();
    // SID header/count were bounded within this ACE before calling Windows SID APIs.
    if unsafe { IsValidSid(raw) } == 0 {
        return Err("stage-security");
    }
    Ok(raw)
}

fn private(file: &File, user: &User) -> Result<()> {
    let mut owner = ptr::null_mut();
    let mut acl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    if unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut descriptor,
        )
    } != ERROR_SUCCESS
    {
        return Err("stage-security");
    }
    let checked = (|| {
        let mut control = 0;
        let mut revision = 0;
        if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0
            || control & SE_DACL_PROTECTED == 0
            || owner.is_null()
            || acl.is_null()
            || unsafe { IsValidSid(owner) } == 0
            || unsafe { EqualSid(owner, user.sid()) } == 0
            || unsafe { (*acl).AceCount } != 2
        {
            return Err("stage-security");
        }
        let mut size_info: ACL_SIZE_INFORMATION = unsafe { zeroed() };
        if unsafe {
            GetAclInformation(
                acl,
                (&mut size_info as *mut ACL_SIZE_INFORMATION).cast(),
                size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        } == 0
            || size_info.AclBytesInUse < size_of::<ACL>() as u32
            || size_info.AclBytesInUse > unsafe { (*acl).AclSize as u32 }
        {
            return Err("stage-security");
        }
        let acl_bytes = unsafe {
            std::slice::from_raw_parts(acl.cast::<u8>(), size_info.AclBytesInUse as usize)
        };
        let mut identities = 0;
        for index in 0..2 {
            let mut raw = ptr::null_mut();
            if unsafe { GetAce(acl, index, &mut raw) } == 0 || raw.is_null() {
                return Err("stage-security");
            }
            let start = (raw as usize)
                .checked_sub(acl as usize)
                .ok_or("stage-security")?;
            if start < size_of::<ACL>() {
                return Err("stage-security");
            }
            let header = acl_bytes
                .get(start..start.checked_add(4).ok_or("stage-security")?)
                .ok_or("stage-security")?;
            let length = u16::from_le_bytes([header[2], header[3]]) as usize;
            let bytes = acl_bytes
                .get(start..start.checked_add(length).ok_or("stage-security")?)
                .ok_or("stage-security")?;
            let sid = allowed_sid(bytes)?;
            let identity = if unsafe { EqualSid(sid, user.sid()) } != 0 {
                1
            } else if unsafe { IsWellKnownSid(sid, WinLocalSystemSid) } != 0 {
                2
            } else {
                return Err("stage-security");
            };
            if identities & identity != 0 {
                return Err("stage-security");
            }
            identities |= identity;
        }
        if identities != 3 {
            return Err("stage-security");
        }
        Ok(())
    })();
    unsafe {
        LocalFree(descriptor);
    }
    checked
}

pub fn known_root() -> Result<PathBuf> {
    let mut raw = ptr::null_mut();
    if unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, ptr::null_mut(), &mut raw) } < 0 {
        return Err("stage-location");
    }
    let mut length = 0;
    unsafe {
        while *raw.add(length) != 0 {
            length += 1;
        }
    }
    let local = PathBuf::from(OsString::from_wide(unsafe {
        std::slice::from_raw_parts(raw, length)
    }));
    unsafe {
        CoTaskMemFree(raw.cast());
    }
    let _local = Directories::hold(&local)?;
    // Separate from the roaming Electron profile and the Assist identity store.
    Ok(local.join("Palisade-Updates-v1"))
}

pub struct Stage {
    path: PathBuf,
    _directories: Directories,
    _root: Directories,
    _anchor: Directories,
}
impl Stage {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn overlaps_installation(&self, directory: &Directories) -> Result<bool> {
        self._root.overlaps(directory)
    }
    pub fn create(id: &str) -> Result<Self> {
        Self::create_at(&known_root()?, id)
    }
    pub fn open(id: &str) -> Result<Self> {
        Self::open_at(&known_root()?, id)
    }
    pub fn open_existing(id: &str) -> Result<Option<Self>> {
        if !crate::stage_id(id) {
            return Err("stage-id");
        }
        let root = known_root()?;
        // An absent stage is already discarded. All existing stages must pass
        // the same handle, reparse and ownership checks as an inspect operation.
        match std::fs::symlink_metadata(root.join(id)) {
            Ok(_) => Self::open_at(&root, id).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err("stage-directory-open"),
        }
    }
    fn create_at(root: &Path, id: &str) -> Result<Self> {
        if !crate::stage_id(id) {
            return Err("stage-id");
        }
        let parent = root.parent().ok_or("stage-location")?;
        let anchor = Directories::hold(parent)?;
        let user = User::current()?;
        create_private(root, &user, true)?;
        let root_hold = Directories::seal(root)?;
        private(root_hold.last()?, &user)?;
        if std::fs::read_dir(root)
            .map_err(|_| "stage-list")?
            .take(4)
            .count()
            >= 3
        {
            return Err("stage-quota");
        }
        let path = root.join(id);
        create_private(&path, &user, false)?;
        let directories = Directories::seal(&path)?;
        private(directories.last()?, &user)?;
        Ok(Self {
            path,
            _directories: directories,
            _root: root_hold,
            _anchor: anchor,
        })
    }
    fn open_at(root: &Path, id: &str) -> Result<Self> {
        if !crate::stage_id(id) {
            return Err("stage-id");
        }
        let user = User::current()?;
        let anchor = Directories::hold(root.parent().ok_or("stage-location")?)?;
        let root_hold = Directories::seal(root)?;
        private(root_hold.last()?, &user)?;
        let path = root.join(id);
        let directories = Directories::seal(&path)?;
        private(directories.last()?, &user)?;
        Ok(Self {
            path,
            _directories: directories,
            _root: root_hold,
            _anchor: anchor,
        })
    }
    pub fn locked_file(&self, name: &str) -> Result<File> {
        check_name(name)?;
        self.open_relative(
            name,
            FILE_GENERIC_READ | READ_CONTROL,
            FILE_OPEN,
            FILE_SHARE_READ,
        )
    }
    pub fn create_new(&self, name: &str) -> Result<File> {
        check_name(name)?;
        if std::fs::read_dir(&self.path)
            .map_err(|_| "stage-list")?
            .take(65)
            .count()
            >= 64
        {
            return Err("stage-file-quota");
        }
        self.open_relative(name, FILE_GENERIC_READ | FILE_GENERIC_WRITE, FILE_CREATE, 0)
    }
    /// Shallow, handle-bound deletion only. Never traverse a nested directory or reparse point.
    pub fn discard(self) -> Result<()> {
        let entries = std::fs::read_dir(&self.path)
            .map_err(|_| "stage-list")?
            .take(65)
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|_| "stage-list")?;
        if entries.len() > 64 {
            return Err("stage-file-quota");
        }
        let mut files = Vec::new();
        for entry in entries {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "stage-filename")?;
            check_name(&name)?;
            files.push(self.open_relative(&name, DELETE | SYNCHRONIZE, FILE_OPEN, 0)?);
        }
        // All entries have first been checked and exclusively opened. Partial failure remains retryable.
        for file in files {
            dispose(&file)?;
        }
        let Stage {
            path,
            _directories,
            _root,
            _anchor,
        } = self;
        drop(_directories);
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("stage-id")?;
        let directory = relative(
            _root.last()?,
            name,
            DELETE | SYNCHRONIZE,
            FILE_OPEN,
            0,
            true,
        )?;
        dispose(&directory)?;
        drop(directory);
        drop(_root);
        drop(_anchor);
        Ok(())
    }
    pub fn mark_internet(&self, release: &crate::VerifiedRelease) -> Result<()> {
        use std::io::Write;
        let payload = release.payload();
        let mut file = self.open_relative(
            &format!("{}:Zone.Identifier", payload.filename),
            FILE_GENERIC_WRITE,
            FILE_OVERWRITE_IF,
            FILE_SHARE_READ,
        )?;
        file.write_all(
            format!(
                "[ZoneTransfer]\r\nZoneId=3\r\nHostUrl={}\r\n",
                payload.download_url
            )
            .as_bytes(),
        )
        .map_err(|_| "zone-write")?;
        file.sync_all().map_err(|_| "zone-write")
    }
    fn open_relative(
        &self,
        name: &str,
        access: u32,
        disposition: u32,
        sharing: u32,
    ) -> Result<File> {
        relative(
            self._directories.last()?,
            name,
            access,
            disposition,
            sharing,
            false,
        )
    }
}

fn dispose(file: &File) -> Result<()> {
    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            (&info as *const FILE_DISPOSITION_INFO).cast(),
            size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    } == 0
    {
        return Err("stage-delete");
    }
    Ok(())
}

fn relative(
    directory: &File,
    name: &str,
    access: u32,
    disposition: u32,
    sharing: u32,
    is_directory: bool,
) -> Result<File> {
    if name.is_empty()
        || name.chars().any(char::is_control)
        || name.contains(['/', '\\'])
        || name.starts_with('.')
        || name.ends_with(['.', ' '])
    {
        return Err("stage-filename");
    }
    use windows_sys::{
        Wdk::{
            Foundation::OBJECT_ATTRIBUTES,
            Storage::FileSystem::{
                FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN_REPARSE_POINT,
                FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile,
            },
        },
        Win32::{Foundation::UNICODE_STRING, System::IO::IO_STATUS_BLOCK},
    };
    let mut words: Vec<u16> = name.encode_utf16().collect();
    if words.len() > 255 {
        return Err("stage-filename");
    }
    let mut unicode = UNICODE_STRING {
        Length: (words.len() * 2) as u16,
        MaximumLength: (words.len() * 2) as u16,
        Buffer: words.as_mut_ptr(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: directory.as_raw_handle(),
        ObjectName: &mut unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: ptr::null_mut(),
        SecurityQualityOfService: ptr::null_mut(),
    };
    let mut status: IO_STATUS_BLOCK = unsafe { zeroed() };
    let mut raw = ptr::null_mut();
    // Documented user-mode NtCreateFile binds one filename to the retained directory;
    // verified-file access never re-traverses ancestor pathnames. No driver is used.
    if unsafe {
        NtCreateFile(
            &mut raw,
            access | FILE_READ_ATTRIBUTES,
            &attributes,
            &mut status,
            ptr::null(),
            FILE_ATTRIBUTE_NORMAL,
            sharing,
            disposition,
            FILE_OPEN_REPARSE_POINT
                | if is_directory {
                    FILE_DIRECTORY_FILE
                } else {
                    FILE_NON_DIRECTORY_FILE
                }
                | FILE_SYNCHRONOUS_IO_NONALERT,
            ptr::null(),
            0,
        )
    } < 0
    {
        return Err("stage-file-open");
    }
    let file = unsafe { File::from_raw_handle(raw) };
    let info = information(&file)?;
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) != is_directory
        || (!is_directory && info.nNumberOfLinks != 1)
    {
        return Err("stage-file-reparse");
    }
    Ok(file)
}

fn check_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.contains(['/', '\\', ':'])
        || name.chars().any(char::is_control)
        || name.starts_with('.')
        || name.ends_with(['.', ' '])
    {
        return Err("stage-filename");
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn fixture() -> PathBuf {
        use windows_sys::Win32::Security::Cryptography::*;
        let mut random = [0; 16];
        assert_eq!(
            unsafe {
                BCryptGenRandom(
                    ptr::null_mut(),
                    random.as_mut_ptr(),
                    16,
                    BCRYPT_USE_SYSTEM_PREFERRED_RNG,
                )
            },
            0
        );
        let name = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let root = std::env::temp_dir().join(format!("palisade-update-test-{name}"));
        std::fs::create_dir(&root).unwrap();
        root
    }
    #[test]
    fn quota_and_shallow_discard_preserve_unrelated_paths() {
        let base = fixture();
        let root = base.join("Updates-v1");
        let ids = [
            "77e14877-832f-44bc-9e92-b27eb6ba77ad",
            "77e14877-832f-44bc-9e92-b27eb6ba77ae",
            "77e14877-832f-44bc-9e92-b27eb6ba77af",
        ];
        for id in ids {
            drop(Stage::create_at(&root, id).unwrap());
        }
        assert!(Stage::create_at(&root, "77e14877-832f-44bc-9e92-b27eb6ba77aa").is_err());
        let stage = Stage::open_at(&root, ids[0]).unwrap();
        std::fs::write(stage.path().join("download.partial"), b"incomplete").unwrap();
        stage.discard().unwrap();
        assert!(!root.join(ids[0]).exists());
        assert!(root.join(ids[1]).exists());
        let stage = Stage::create_at(&root, ids[0]).unwrap();
        std::fs::create_dir(stage.path().join("unexpected")).unwrap();
        assert!(stage.discard().is_err());
        assert!(root.join(ids[0]).join("unexpected").exists());
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn private_staging_refuses_reuse_and_holds_file_and_directory_identity() {
        let base = fixture();
        let root = base.join("Updates-v1");
        let id = "77e14877-832f-44bc-9e92-b27eb6ba77ad";
        let stage = Stage::create_at(&root, id).unwrap();
        assert!(Stage::create_at(&root, id).is_err());
        let target = stage.path.join("fixture.exe");
        std::fs::write(&target, b"fixture").unwrap();
        let file = stage.locked_file("fixture.exe").unwrap();
        assert!(std::fs::write(&target, b"changed").is_err());
        assert!(std::fs::remove_file(&target).is_err());
        assert!(std::fs::rename(&stage.path, base.join("moved")).is_err());
        let path = wide(stage.path());
        let mutation = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                ptr::null_mut(),
            )
        };
        let error = unsafe { GetLastError() };
        assert_eq!(
            mutation, INVALID_HANDLE_VALUE,
            "a held directory cannot be opened for reparse/attribute mutation"
        );
        assert_eq!(error, ERROR_SHARING_VIOLATION);
        assert!(stage.locked_file("../fixture.exe").is_err());
        assert!(Stage::open_at(&root, id).is_ok());
        drop(file);
        drop(stage);
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn broad_existing_staging_acl_and_hardlinks_are_rejected() {
        let base = fixture();
        let root = base.join("Updates-v1");
        std::fs::create_dir(&root).unwrap();
        assert!(Stage::create_at(&root, "77e14877-832f-44bc-9e92-b27eb6ba77ad").is_err());
        std::fs::remove_dir(&root).unwrap();
        let stage = Stage::create_at(&root, "77e14877-832f-44bc-9e92-b27eb6ba77ad").unwrap();
        let target = stage.path.join("fixture.exe");
        std::fs::write(&target, b"fixture").unwrap();
        assert!(std::fs::hard_link(&target, stage.path.join("link.exe")).is_err());
        let destination = stage.path.join("link.exe");
        drop(stage);
        std::fs::hard_link(&target, destination).unwrap();
        let stage = Stage::open_at(&root, "77e14877-832f-44bc-9e92-b27eb6ba77ad").unwrap();
        assert!(stage.locked_file("fixture.exe").is_err());
        drop(stage);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn retained_installation_directory_permits_ordinary_child_replacement() {
        let base = fixture();
        let file = base.join("old.exe");
        std::fs::write(&file, b"old").unwrap();
        let guard = Directories::hold(&base).unwrap();
        std::fs::write(&file, b"new").unwrap();
        let renamed = base.join("replacement.exe");
        std::fs::rename(&file, &renamed).unwrap();
        std::fs::remove_file(renamed).unwrap();
        let directory = base.join("resources");
        std::fs::create_dir(&directory).unwrap();
        std::fs::remove_dir(&directory).unwrap();
        assert!(std::fs::rename(&base, base.with_extension("moved")).is_err());
        drop(guard);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn installation_overlap_is_detected_by_retained_objects_in_both_directions() {
        let base = fixture();
        let root = base.join("Updates-v1");
        let stage = Stage::create_at(&root, "77e14877-832f-44bc-9e92-b27eb6ba77ad").unwrap();
        for path in [&base, &root, stage.path()] {
            let directory = Directories::hold(path).unwrap();
            assert!(stage.overlaps_installation(&directory).unwrap());
        }
        let app = base.join("app");
        std::fs::create_dir(&app).unwrap();
        let directory = Directories::hold(&app).unwrap();
        assert!(!stage.overlaps_installation(&directory).unwrap());
        drop(directory);
        drop(stage);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn acl_parser_rejects_non_inheriting_inherit_only_and_malformed_sid_entries() {
        let user = User::current().unwrap();
        let length = unsafe { GetLengthSid(user.sid()) } as usize;
        let mut ace = vec![0; 8 + length];
        ace[1] = 3;
        ace[2..4].copy_from_slice(&((8 + length) as u16).to_le_bytes());
        ace[4..8].copy_from_slice(&FILE_ALL_ACCESS.to_le_bytes());
        ace[8..].copy_from_slice(unsafe {
            std::slice::from_raw_parts(user.sid().cast::<u8>(), length)
        });
        assert!(allowed_sid(&ace).is_ok());
        for (index, value) in [(0, 1), (1, 0), (1, 11), (2, 0), (4, 0), (8, 2), (9, 255)] {
            let mut invalid = ace.clone();
            invalid[index] = value;
            assert!(allowed_sid(&invalid).is_err());
        }
        for length in 0..ace.len() {
            assert!(allowed_sid(&ace[..length]).is_err());
        }
    }

    #[test]
    fn exclusive_stage_publication_and_internet_zone_preserve_the_locked_installer_bytes() {
        use std::io::Write;
        let base = fixture();
        let root = base.join("Updates-v1");
        let stage = Stage::create_at(&root, "77e14877-832f-44bc-9e92-b27eb6ba77ad").unwrap();
        let release = crate::tests::authorized_bytes(b"fixture");
        let name = &release.payload().filename;
        let mut file = stage.create_new(name).unwrap();
        file.write_all(b"fixture").unwrap();
        file.sync_all().unwrap();
        drop(file);
        assert!(stage.create_new(name).is_err());
        let mut locked = stage.locked_file(name).unwrap();
        stage.mark_internet(&release).unwrap();
        assert!(crate::verify_installer(&mut locked, &release).is_ok());
        let zone =
            std::fs::read_to_string(stage.path().join(format!("{name}:Zone.Identifier"))).unwrap();
        assert!(zone.contains("ZoneId=3\r\n"));
        assert!(zone.contains(&release.payload().download_url));
        drop(locked);
        drop(stage);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_preexisting_junction_cannot_redirect_the_retained_stage() {
        use windows_sys::Win32::System::{IO::DeviceIoControl, Ioctl::FSCTL_SET_REPARSE_POINT};
        let base = fixture();
        let root = base.join("Updates-v1");
        let id = "77e14877-832f-44bc-9e92-b27eb6ba77ad";
        let stage = Stage::create_at(&root, id).unwrap();
        let original = stage.path().to_owned();
        drop(stage);
        let destination = base.join("original");
        std::fs::rename(&original, &destination).unwrap();
        std::fs::create_dir(&original).unwrap();
        let substitute: Vec<u16> = format!("\\??\\{}", destination.to_str().unwrap())
            .encode_utf16()
            .collect();
        let display: Vec<u16> = destination.as_os_str().encode_wide().collect();
        let mut bytes = vec![0u8; 16];
        bytes[0..4].copy_from_slice(&0xa0000003u32.to_le_bytes());
        bytes[10..12].copy_from_slice(&((substitute.len() * 2) as u16).to_le_bytes());
        bytes[12..14].copy_from_slice(&((substitute.len() * 2 + 2) as u16).to_le_bytes());
        bytes[14..16].copy_from_slice(&((display.len() * 2) as u16).to_le_bytes());
        for word in substitute
            .iter()
            .chain(Some(&0))
            .chain(display.iter())
            .chain(Some(&0))
        {
            bytes.extend(word.to_le_bytes());
        }
        let data_length = bytes.len() as u16 - 8;
        bytes[4..6].copy_from_slice(&data_length.to_le_bytes());
        let name = wide(&original);
        let raw = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_WRITE,
                0,
                ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                ptr::null_mut(),
            )
        };
        assert_ne!(raw, INVALID_HANDLE_VALUE);
        let handle = Handle(raw);
        let mut returned = 0;
        assert_ne!(
            unsafe {
                DeviceIoControl(
                    handle.0,
                    FSCTL_SET_REPARSE_POINT,
                    bytes.as_ptr().cast(),
                    bytes.len() as u32,
                    ptr::null_mut(),
                    0,
                    &mut returned,
                    ptr::null_mut(),
                )
            },
            0
        );
        drop(handle);
        assert!(Stage::open_at(&root, id).is_err());
        assert!(Directories::hold(&original).is_err());
        std::fs::remove_dir(&original).unwrap();
        std::fs::remove_dir_all(base).unwrap();
    }
}
