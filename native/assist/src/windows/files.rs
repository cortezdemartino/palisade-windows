//! Native-selected, session-scoped files. No caller-selected source/destination path API.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    mem::{size_of, zeroed},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        fs::{MetadataExt, OpenOptionsExt},
        io::{AsRawHandle, FromRawHandle},
    },
    path::{Component, Path, PathBuf, Prefix},
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use unicode_normalization::UnicodeNormalization;
use windows_sys::Win32::System::WindowsProgramming::DRIVE_FIXED;
use windows_sys::{
    Win32::{
        Foundation::*,
        Security::{Authorization::*, Cryptography::*, *},
        Storage::FileSystem::*,
        UI::{Controls::Dialogs::*, WindowsAndMessaging::*},
    },
    core::w,
};
mod bound;
mod receive;
pub use bound::{FileRequest, FileRevoker, FileSelection, SessionFiles};
const FILE_LIMIT: u64 = 1024 * 1024 * 1024;
const SESSION_LIMIT: u64 = 4 * FILE_LIMIT;
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}
impl Metadata {
    pub fn validate(&self) -> Result<(), &'static str> {
        let name = &self.name;
        if name.is_empty()
            || name.encode_utf16().count() > 180
            || name.trim() != name
            || !name.nfc().eq(name.chars())
            || name.ends_with(['.', ' '])
            || name.chars().any(|c| {
                c.is_control()
                    || "<>:\"/\\|?*".contains(c)
                    || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
            })
            || name.chars().all(|c| c == '.')
            || self.size > FILE_LIMIT
            || self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err("invalid-file");
        }
        let stem = name.split('.').next().unwrap().to_ascii_lowercase();
        if ["con", "prn", "aux", "nul"].contains(&stem.as_str())
            || ["com", "lpt"].iter().any(|p| {
                stem.strip_prefix(p).is_some_and(|v| {
                    v.chars().count() == 1
                        && v.chars().all(|c| c.is_ascii_digit() || "¹²³".contains(c))
                })
            })
        {
            return Err("invalid-file");
        }
        Ok(())
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn clear_delete_on_close(file: &File) -> Result<(), &'static str> {
    let info = FILE_DISPOSITION_INFO_EX { Flags: 8 };
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfoEx,
            (&info as *const FILE_DISPOSITION_INFO_EX).cast(),
            size_of::<FILE_DISPOSITION_INFO_EX>() as u32,
        )
    } == 0
    {
        return Err("commit-unsupported");
    }
    Ok(())
}
fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
fn identifier() -> Result<String, &'static str> {
    let mut data = [0u8; 24];
    if unsafe {
        BCryptGenRandom(
            ptr::null_mut(),
            data.as_mut_ptr(),
            data.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    } < 0
    {
        return Err("random-failed");
    };
    Ok(data.iter().map(|b| format!("{b:02x}")).collect())
}
struct Handle(HANDLE);
// SAFETY: these are owned directory or token handles, not thread-affine UI
// handles. Their only shared use is serialized by the owning file-store mutex.
unsafe impl Send for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn handle_path(handle: HANDLE) -> Result<PathBuf, &'static str> {
    let mut bytes = vec![0u16; 32768];
    let count = unsafe {
        GetFinalPathNameByHandleW(
            handle,
            bytes.as_mut_ptr(),
            bytes.len() as u32,
            VOLUME_NAME_GUID,
        )
    };
    if count == 0 || count as usize >= bytes.len() {
        return Err("destination-unavailable");
    }
    let parent = PathBuf::from(std::ffi::OsString::from_wide(&bytes[..count as usize]));
    if !parent
        .as_os_str()
        .to_string_lossy()
        .starts_with(r"\\?\Volume{")
    {
        return Err("unsupported-path");
    }
    Ok(parent)
}
pub(crate) struct ParentGuard {
    _directories: Vec<Handle>,
}
impl ParentGuard {
    fn destination(&self, name: &std::ffi::OsStr) -> Result<PathBuf, &'static str> {
        Ok(handle_path(self._directories.last().ok_or("destination-unavailable")?.0)?.join(name))
    }
    pub(crate) fn verify_file(
        &self,
        file: &File,
        name: &std::ffi::OsStr,
    ) -> Result<(), &'static str> {
        // Denying directory rename does not prevent an empty directory from
        // gaining a junction in place. Bind the actual opened file, not another
        // pathname attribute check, before the first content read or write.
        let stat = file.metadata().map_err(|_| "destination-unavailable")?;
        if !stat.is_file() || stat.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err("reparse-refused");
        }
        if handle_path(file.as_raw_handle())? != self.destination(name)? {
            return Err("path-binding-mismatch");
        }
        for directory in &self._directories {
            let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { zeroed() };
            if unsafe { GetFileInformationByHandle(directory.0, &mut info) } == 0
                || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
                || info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
            {
                return Err("reparse-refused");
            }
        }
        // The held file cannot be unlinked, and every held ancestor has a held
        // child. NTFS refuses setting a reparse point on a nonempty directory.
        Ok(())
    }
    pub(crate) fn open(path: &Path) -> Result<Self, &'static str> {
        let mut components = path.components();
        if !matches!(components.next(),Some(Component::Prefix(p)) if matches!(p.kind(),Prefix::Disk(_)))
            || !path.is_absolute()
            || path.as_os_str().encode_wide().count() > 4096
            || path.components().count() > 64
            || path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err("unsupported-path");
        }
        let parent = path.parent().ok_or("invalid-destination")?.to_path_buf();
        let root = path.ancestors().last().ok_or("unsupported-path")?;
        let root = wide(root);
        let mut filesystem = [0u16; 32];
        let mut flags = 0;
        if unsafe { GetDriveTypeW(root.as_ptr()) } != DRIVE_FIXED
            || unsafe {
                GetVolumeInformationW(
                    root.as_ptr(),
                    ptr::null_mut(),
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut flags,
                    filesystem.as_mut_ptr(),
                    filesystem.len() as u32,
                )
            } == 0
            || String::from_utf16_lossy(
                &filesystem[..filesystem.iter().position(|&c| c == 0).unwrap_or(32)],
            ) != "NTFS"
        {
            return Err("unsupported-filesystem");
        }
        let mut directories: Vec<Handle> = Vec::new();
        let ancestors: Vec<_> = parent.ancestors().collect();
        for dir in ancestors.into_iter().rev() {
            let resolved = if let Some(previous) = directories.last() {
                handle_path(previous.0)?.join(dir.file_name().ok_or("unsupported-path")?)
            } else {
                dir.to_path_buf()
            };
            let name = wide(&resolved);
            let handle = unsafe {
                CreateFileW(
                    name.as_ptr(),
                    FILE_READ_ATTRIBUTES | FILE_LIST_DIRECTORY,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    ptr::null(),
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                    ptr::null_mut(),
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                return Err("destination-unavailable");
            }
            let handle = Handle(handle);
            let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { zeroed() };
            if unsafe { GetFileInformationByHandle(handle.0, &mut info) } == 0
                || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
                || info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
            {
                return Err("reparse-refused");
            }
            if !directories.is_empty() && handle_path(handle.0)? != resolved {
                return Err("path-binding-mismatch");
            }
            directories.push(handle);
        }
        Ok(Self {
            _directories: directories,
        })
    }
}
enum Transfer {
    Send {
        file: File,
        _parent: ParentGuard,
        metadata: Metadata,
        offset: u64,
    },
    Receive(Box<Receive>),
}
struct Receive {
    temporary: PathBuf,
    file: File,
    parent: ParentGuard,
    metadata: Metadata,
    name: std::ffi::OsString,
    hash: Sha256,
    written: u64,
    verified: bool,
    destination: Option<PathBuf>,
    committed: bool,
}
impl Drop for Receive {
    fn drop(&mut self) {
        if !self.committed {
            let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
            unsafe {
                SetFileInformationByHandle(
                    self.file.as_raw_handle(),
                    FileDispositionInfo,
                    (&disposition as *const FILE_DISPOSITION_INFO).cast(),
                    size_of::<FILE_DISPOSITION_INFO>() as u32,
                );
            }
        }
    }
}
struct FileStore {
    transfers: BTreeMap<String, Transfer>,
    quota: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
}
impl FileStore {
    #[cfg(test)]
    fn new(cancel: Arc<AtomicBool>) -> Self {
        Self::with_quota(cancel, Arc::new(AtomicU64::new(0)))
    }
    fn with_quota(cancel: Arc<AtomicBool>, quota: Arc<AtomicU64>) -> Self {
        Self {
            transfers: BTreeMap::new(),
            quota,
            cancel,
        }
    }
    fn charge(&self, bytes: u64) -> Result<(), &'static str> {
        self.quota
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
                used.checked_add(bytes)
                    .filter(|total| *total <= SESSION_LIMIT)
            })
            .map(|_| ())
            .map_err(|_| "transfer-quota")
    }
    fn live(&self) -> Result<(), &'static str> {
        if self.cancel.load(Ordering::SeqCst) {
            Err("transfer-cancelled")
        } else {
            Ok(())
        }
    }
    pub fn select_send(
        &mut self,
        operation: Arc<AtomicBool>,
        authority: crate::cancellation::SessionStop,
        mut authorize: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<(String, Metadata), &'static str> {
        if !cfg!(feature = "native-resources") {
            return Err("native-resources-disabled");
        }
        self.live()?;
        authorize()?;
        let cancellation = Cancellation {
            session: self.cancel.clone(),
            operation,
            authority,
        };
        let mut check = || {
            cancellation.live()?;
            authorize()
        };
        check()?;
        let path = dialog(false, None, &cancellation)?.ok_or("transfer-cancelled")?;
        self.live()?;
        check()?;
        // A common-dialog FileOK can be queued or synthetic. A separate,
        // app-owned fresh physical decision must precede any selected file read.
        if !super::consent::confirm_selected_file(
            &path,
            false,
            cancellation.operation.clone(),
            cancellation.authority.clone(),
        )? {
            return Err("file-selection-denied");
        }
        check()?;
        self.open_send(&path, &mut check)
    }
    pub fn select_receive(
        &mut self,
        metadata: Metadata,
        operation: Arc<AtomicBool>,
        authority: crate::cancellation::SessionStop,
        mut authorize: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<String, &'static str> {
        if !cfg!(feature = "native-resources") {
            return Err("native-resources-disabled");
        }
        self.live()?;
        metadata.validate()?;
        authorize()?;
        let cancellation = Cancellation {
            session: self.cancel.clone(),
            operation,
            authority,
        };
        let mut check = || {
            cancellation.live()?;
            authorize()
        };
        check()?;
        // Unit tests use explicit disposable roots, never the user's AppData.
        if cfg!(test) {
            return Err("file-test-ui-disabled");
        }
        let root = super::identity::storage()?;
        self.open_staged_receive(&root, metadata, &mut check)
    }
    fn open_staged_receive(
        &mut self,
        root: &Path,
        metadata: Metadata,
        authorize: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<String, &'static str> {
        self.live()?;
        authorize()?;
        metadata.validate()?;
        if self
            .transfers
            .values()
            .any(|v| matches!(v, Transfer::Receive(_)))
        {
            return Err("transfer-busy");
        }
        self.charge(metadata.size)?;
        let token = identifier()?;
        let path = root.join(format!(".palisade-receive-{token}.partial"));
        let parent = ParentGuard::open(&path)?;
        let temporary = parent.destination(path.file_name().ok_or("invalid-file")?)?;
        authorize()?;
        self.live()?;
        let file = private_file(&temporary, &parent)?;
        self.transfers.insert(
            token.clone(),
            Transfer::Receive(Box::new(Receive {
                temporary,
                file,
                parent,
                name: metadata.name.clone().into(),
                metadata,
                hash: Sha256::new(),
                written: 0,
                verified: false,
                destination: None,
                committed: false,
            })),
        );
        Ok(token)
    }

    fn open_send(
        &mut self,
        path: &Path,
        authorize: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<(String, Metadata), &'static str> {
        self.live()?;
        authorize()?;
        if self
            .transfers
            .values()
            .any(|v| matches!(v, Transfer::Send { .. }))
        {
            return Err("transfer-busy");
        }
        let parent = ParentGuard::open(path)?;
        self.live()?;
        authorize()?;
        let mut file = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(parent.destination(path.file_name().ok_or("invalid-file")?)?)
            .map_err(|_| "source-unavailable")?;
        parent.verify_file(&file, path.file_name().ok_or("invalid-file")?)?;
        let stat = file.metadata().map_err(|_| "source-unavailable")?;
        if !stat.is_file()
            || stat.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || stat.len() > FILE_LIMIT
        {
            return Err("invalid-file");
        }
        // Failed and cancelled attempts remain charged to the session quota.
        self.charge(stat.len())?;
        let mut hash = Sha256::new();
        let mut bytes = [0u8; 65536];
        loop {
            self.live()?;
            authorize()?;
            let count = file.read(&mut bytes).map_err(|_| "source-read")?;
            if count == 0 {
                break;
            }
            hash.update(&bytes[..count]);
        }
        file.seek(SeekFrom::Start(0)).map_err(|_| "source-read")?;
        let metadata = Metadata {
            name: path
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or("unsupported-name")?
                .into(),
            size: stat.len(),
            sha256: hex(&hash.finalize()),
        };
        metadata.validate()?;
        let token = identifier()?;
        self.transfers.insert(
            token.clone(),
            Transfer::Send {
                file,
                _parent: parent,
                metadata: metadata.clone(),
                offset: 0,
            },
        );
        Ok((token, metadata))
    }
    #[cfg(test)]
    fn open_receive(
        &mut self,
        path: &Path,
        metadata: Metadata,
        authorize: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<String, &'static str> {
        self.live()?;
        authorize()?;
        metadata.validate()?;
        if self
            .transfers
            .values()
            .any(|v| matches!(v, Transfer::Receive(_)))
        {
            return Err("transfer-busy");
        }
        let parent = ParentGuard::open(path)?;
        let destination = path.file_name().ok_or("invalid-destination")?;
        Metadata {
            name: destination.to_str().ok_or("unsupported-name")?.into(),
            size: metadata.size,
            sha256: metadata.sha256.clone(),
        }
        .validate()?;
        if path.try_exists().map_err(|_| "destination-unavailable")? {
            return Err("destination-exists");
        }
        self.charge(metadata.size)?;
        let token = identifier()?;
        let temp =
            parent.destination(std::ffi::OsStr::new(&format!(".palisade-{token}.partial")))?;
        let file = private_file(&temp, &parent)?;
        self.transfers.insert(
            token.clone(),
            Transfer::Receive(Box::new(Receive {
                temporary: temp,
                file,
                parent,
                metadata,
                name: destination.into(),
                hash: Sha256::new(),
                written: 0,
                verified: false,
                destination: Some(path.into()),
                committed: false,
            })),
        );
        Ok(token)
    }
    pub fn read(
        &mut self,
        token: &str,
        offset: u64,
        mut authorize: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Vec<u8>, &'static str> {
        let result = self.read_inner(token, offset, &mut authorize);
        if result.is_err() {
            self.cancel(token);
        }
        result
    }
    fn read_inner(
        &mut self,
        token: &str,
        offset: u64,
        mut authorize: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Vec<u8>, &'static str> {
        self.live()?;
        authorize()?;
        let Some(Transfer::Send {
            file,
            metadata,
            offset: position,
            ..
        }) = self.transfers.get_mut(token)
        else {
            return Err("transfer-not-found");
        };
        if *position != offset {
            return Err("transfer-replay");
        };
        let mut bytes = vec![0u8; 65536];
        let count = file.read(&mut bytes).map_err(|_| "source-read")?;
        if position.saturating_add(count as u64) > metadata.size {
            return Err("source-changed");
        };
        *position += count as u64;
        bytes.truncate(count);
        authorize()?;
        self.live()?;
        Ok(bytes)
    }
    pub fn write(
        &mut self,
        token: &str,
        offset: u64,
        bytes: &[u8],
        mut authorize: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<u64, &'static str> {
        let result = self.write_inner(token, offset, bytes, &mut authorize);
        if result.is_err() {
            self.cancel(token);
        }
        result
    }
    fn write_inner(
        &mut self,
        token: &str,
        offset: u64,
        bytes: &[u8],
        mut authorize: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<u64, &'static str> {
        self.live()?;
        authorize()?;
        let Some(Transfer::Receive(receiver)) = self.transfers.get_mut(token) else {
            return Err("transfer-not-found");
        };
        if receiver.verified
            || bytes.is_empty()
            || bytes.len() > 65536
            || receiver.written != offset
            || receiver.written.saturating_add(bytes.len() as u64) > receiver.metadata.size
        {
            return Err("invalid-chunk");
        }
        receiver
            .file
            .write_all(bytes)
            .map_err(|_| "destination-write")?;
        receiver.hash.update(bytes);
        receiver.written += bytes.len() as u64;
        let total = receiver.written;
        authorize()?;
        self.live()?;
        Ok(total)
    }
    fn verify_receive(
        &mut self,
        token: &str,
        mut authorize: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Metadata, &'static str> {
        let result = (|| {
            self.live()?;
            authorize()?;
            let Some(Transfer::Receive(receiver)) = self.transfers.get_mut(token) else {
                return Err("transfer-not-found");
            };
            receiver.verify(&mut authorize)?;
            Ok(receiver.metadata.clone())
        })();
        if result.is_err() {
            self.cancel(token);
        }
        result
    }
    fn select_commit(
        &mut self,
        token: &str,
        operation: Arc<AtomicBool>,
        authority: crate::cancellation::SessionStop,
        mut authorize: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Metadata, &'static str> {
        let cancellation = Cancellation {
            session: self.cancel.clone(),
            operation,
            authority,
        };
        let mut check = || {
            cancellation.live()?;
            authorize()
        };
        let result = (|| {
            let metadata = self.verify_receive(token, &mut check)?;
            let Some(Transfer::Receive(receiver)) = self.transfers.get(token) else {
                return Err("transfer-not-found");
            };
            let path = match &receiver.destination {
                Some(path) => path.clone(),
                None => {
                    let path = dialog(true, Some(&metadata.name), &cancellation)?
                        .ok_or("transfer-cancelled")?;
                    check()?;
                    // The earlier Receive approval authorizes private staging,
                    // never a later destination chosen by a common dialog.
                    if !super::consent::confirm_selected_file(
                        &path,
                        true,
                        cancellation.operation.clone(),
                        cancellation.authority.clone(),
                    )? {
                        return Err("file-selection-denied");
                    }
                    check()?;
                    path
                }
            };
            check()?;
            self.commit_to(token, &path, &mut check)
        })();
        if result.is_err() {
            self.cancel(token);
        }
        result
    }
    fn commit_to(
        &mut self,
        token: &str,
        path: &Path,
        mut authorize: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Metadata, &'static str> {
        let Some(Transfer::Receive(mut receiver)) = self.transfers.remove(token) else {
            return Err("transfer-not-found");
        };
        self.live()?;
        authorize()?;
        receiver.save_to(path, &mut || {
            self.live()?;
            authorize()
        })
    }
    #[cfg(test)]
    fn commit(
        &mut self,
        token: &str,
        authorize: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Metadata, &'static str> {
        let path = match self.transfers.get(token) {
            Some(Transfer::Receive(receiver)) => {
                receiver.destination.clone().ok_or("destination-required")?
            }
            _ => return Err("transfer-not-found"),
        };
        self.commit_to(token, &path, authorize)
    }
    pub fn cancel(&mut self, token: &str) {
        self.transfers.remove(token);
    }
}
impl Drop for FileStore {
    fn drop(&mut self) {
        self.transfers.clear();
    }
}
fn private_file(path: &Path, parent: &ParentGuard) -> Result<File, &'static str> {
    unsafe {
        let mut token = ptr::null_mut();
        if windows_sys::Win32::System::Threading::OpenProcessToken(
            windows_sys::Win32::System::Threading::GetCurrentProcess(),
            TOKEN_QUERY,
            &mut token,
        ) == 0
        {
            return Err("file-security");
        }
        let token = Handle(token);
        let mut size = 0;
        GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut size);
        if size == 0 || size > 16384 {
            return Err("file-security");
        };
        let mut words = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        if GetTokenInformation(
            token.0,
            TokenUser,
            words.as_mut_ptr().cast(),
            size,
            &mut size,
        ) == 0
        {
            return Err("file-security");
        }
        let user = &*words.as_ptr().cast::<TOKEN_USER>();
        let mut sid = ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut sid) == 0 {
            return Err("file-security");
        }
        let mut length = 0;
        while *sid.add(length) != 0 {
            length += 1
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(sid, length));
        LocalFree(sid.cast());
        let sddl: Vec<u16> = format!("D:P(A;;FA;;;SY)(A;;FA;;;{text})")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut descriptor = ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        ) == 0
        {
            return Err("file-security");
        }
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let name = wide(path);
        let handle = CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE | DELETE,
            0,
            &attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_TEMPORARY | FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_DELETE_ON_CLOSE,
            ptr::null_mut(),
        );
        LocalFree(descriptor);
        if handle == INVALID_HANDLE_VALUE {
            return Err("destination-create");
        };
        let file = File::from_raw_handle(handle);
        // Even a redirected CREATE_NEW contains no application data yet and
        // remains delete-on-close when this actual-handle binding is refused.
        parent.verify_file(&file, path.file_name().ok_or("invalid-file")?)?;
        Ok(file)
    }
}
struct Cancellation {
    session: Arc<AtomicBool>,
    operation: Arc<AtomicBool>,
    authority: crate::cancellation::SessionStop,
}
impl Cancellation {
    fn live(&self) -> Result<(), &'static str> {
        if self.authority.is_stopped()
            || self.session.load(Ordering::SeqCst)
            || self.operation.load(Ordering::SeqCst)
        {
            Err("transfer-cancelled")
        } else {
            Ok(())
        }
    }
}
unsafe extern "system" fn dialog_hook(
    hwnd: HWND,
    message: u32,
    _: WPARAM,
    lparam: LPARAM,
) -> usize {
    unsafe {
        if message == WM_INITDIALOG {
            let request = &*(lparam as *const OPENFILENAMEW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, request.lCustData);
            SetTimer(hwnd, 1, 50, None);
        }
        let flag = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Cancellation;
        if message == WM_TIMER && !flag.is_null() && (*flag).live().is_err() {
            PostMessageW(GetParent(hwnd), WM_COMMAND, 2, 0);
        }
        if message == WM_DESTROY {
            KillTimer(hwnd, 1);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        }
        0
    }
}
fn dialog(
    save: bool,
    default_name: Option<&str>,
    cancel: &Cancellation,
) -> Result<Option<PathBuf>, &'static str> {
    cancel.live()?;
    if cfg!(test) {
        return Err("file-test-ui-disabled");
    }
    super::input::ordinary_desktop()?;
    let _input_exclusion = super::input_owner::InputOwner::acquire()?;
    unsafe {
        let mut buffer = vec![0u16; 32768];
        if let Some(name) = default_name {
            let text: Vec<u16> = name.encode_utf16().collect();
            if text.len() > 180 {
                return Err("invalid-file");
            };
            buffer[..text.len()].copy_from_slice(&text);
        }
        let mut request: OPENFILENAMEW = zeroed();
        request.lStructSize = size_of::<OPENFILENAMEW>() as u32;
        request.lpstrFile = buffer.as_mut_ptr();
        request.nMaxFile = buffer.len() as u32;
        request.lpstrTitle = if save {
            w!("Save received Palisade file")
        } else {
            w!("Select a file to send with Palisade")
        };
        request.Flags = OFN_NODEREFERENCELINKS
            | OFN_EXPLORER
            | OFN_NOCHANGEDIR
            | OFN_PATHMUSTEXIST
            | OFN_DONTADDTORECENT
            | OFN_ENABLEHOOK
            | if save {
                OFN_NOREADONLYRETURN
            } else {
                OFN_FILEMUSTEXIST
            };
        request.lCustData = (cancel as *const Cancellation) as isize;
        request.lpfnHook = Some(dialog_hook);
        let result = if save {
            GetSaveFileNameW(&mut request)
        } else {
            GetOpenFileNameW(&mut request)
        };
        cancel.live()?;
        if result == 0 {
            return if CommDlgExtendedError() == 0 {
                Ok(None)
            } else {
                Err("file-dialog-failed")
            };
        }
        let length = buffer.iter().position(|&c| c == 0).ok_or("invalid-path")?;
        Ok(Some(PathBuf::from(std::ffi::OsString::from_wide(
            &buffer[..length],
        ))))
    }
}

#[cfg(test)]
fn fixture_directory(prefix: &str) -> PathBuf {
    assert!(matches!(
        prefix,
        "palisade-native-file-test-" | "palisade-bound-file-test-"
    ));
    // GitHub's Windows TEMP can use RUNNER~1. The production guard deliberately
    // compares actual long names exactly; fixtures must supply those same names.
    // Expand only the existing test base, before creating the owned directory.
    let input = wide(&std::env::temp_dir());
    let mut buffer = [0u16; 32768];
    let count =
        unsafe { GetLongPathNameW(input.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32) };
    assert!(
        count > 0 && (count as usize) < buffer.len(),
        "fixture TEMP must have a long absolute path"
    );
    let mut base = PathBuf::from(std::ffi::OsString::from_wide(&buffer[..count as usize]))
        .canonicalize()
        .expect("fixture TEMP must resolve to its actual directory name");
    if matches!(base.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::VerbatimDisk(_)))
    {
        // std::fs canonicalization may yield this prefix; ParentGuard intentionally
        // accepts ordinary drive paths only. This conversion is test-only.
        let output: Vec<u16> = base.as_os_str().encode_wide().collect();
        base = PathBuf::from(std::ffi::OsString::from_wide(&output[4..]));
    }
    assert!(base.is_absolute());
    assert!(
        matches!(base.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_)))
    );
    let path = base.join(format!("{prefix}{}", identifier().unwrap()));
    std::fs::create_dir(&path).unwrap();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    mod junction;
    #[test]
    fn file_dialog_cancellation_observes_launch_without_operation_watcher() {
        let launch = Arc::new(AtomicBool::new(false));
        let cancellation = Cancellation {
            session: Arc::new(AtomicBool::new(false)),
            operation: Arc::new(AtomicBool::new(false)),
            authority: launch.clone().into(),
        };
        assert!(cancellation.live().is_ok());
        launch.store(true, Ordering::SeqCst);
        assert_eq!(cancellation.live(), Err("transfer-cancelled"));
        assert!(!cancellation.operation.load(Ordering::SeqCst));
        assert!(!cancellation.session.load(Ordering::SeqCst));
    }
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            Self(fixture_directory("palisade-native-file-test-"))
        }
        fn store(&self) -> FileStore {
            FileStore::new(Arc::new(AtomicBool::new(false)))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let path = self.0.canonicalize().unwrap();
            let base = std::env::temp_dir().canonicalize().unwrap();
            assert!(
                path.starts_with(&base)
                    && path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("palisade-native-file-test-")
            );
            std::fs::remove_dir_all(path).unwrap();
        }
    }
    fn metadata(name: &str, bytes: &[u8]) -> Metadata {
        Metadata {
            name: name.into(),
            size: bytes.len() as u64,
            sha256: hex(&Sha256::digest(bytes)),
        }
    }
    #[test]
    fn fixture_temp_alias_is_expanded_before_testing_native_path_binding() {
        const ENV: &str = "PALISADE_NATIVE_FILE_ALIAS_FIXTURE";
        if let Some(base) = std::env::var_os(ENV) {
            let base = PathBuf::from(base).canonicalize().unwrap();
            assert_eq!(std::env::temp_dir().canonicalize().unwrap(), base);
            assert_eq!(base.file_name().unwrap(), "long-temporary-directory");
            assert!(
                base.parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("palisade-native-file-test-")
            );
            let fixture = Fixture::new();
            let mut store = fixture.store();
            let token = store
                .open_staged_receive(
                    &fixture.0,
                    metadata("received.txt", b"fixture bytes"),
                    &mut || Ok(()),
                )
                .unwrap();
            store.write(&token, 0, b"fixture bytes", || Ok(())).unwrap();
            store
                .commit_to(&token, &fixture.0.join("saved.txt"), || Ok(()))
                .unwrap();
            assert_eq!(
                std::fs::read(fixture.0.join("saved.txt")).unwrap(),
                b"fixture bytes"
            );
            return;
        }
        let fixture = Fixture::new();
        let base = fixture.0.join("long-temporary-directory");
        std::fs::create_dir(&base).unwrap();
        let input = wide(&base);
        let mut buffer = [0u16; 32768];
        let count =
            unsafe { GetShortPathNameW(input.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32) };
        assert!(count > 0 && (count as usize) < buffer.len());
        let short = PathBuf::from(std::ffi::OsString::from_wide(&buffer[..count as usize]));
        let alias = if short == base {
            eprintln!(
                "8.3 aliases are disabled here; exercising the same normalization boundary with a real case alias."
            );
            base.with_file_name("LONG-TEMPORARY-DIRECTORY")
        } else {
            eprintln!("Exercising disposable 8.3 TEMP alias: {}", short.display());
            short
        };
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "windows::files::tests::fixture_temp_alias_is_expanded_before_testing_native_path_binding", "--nocapture"])
            .env(ENV, &base).env("TEMP", &alias).env("TMP", &alias)
            .output().unwrap();
        assert!(
            result.status.success(),
            "alias fixture failed: {}\nstdout:\n{}\nstderr:\n{}",
            result.status,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(std::fs::read_dir(&base).unwrap().count(), 0);
    }
    #[test]
    fn process_exit_removes_owned_uncommitted_staging_file() {
        const ENV: &str = "PALISADE_NATIVE_FILE_EXIT_FIXTURE";
        if let Some(path) = std::env::var_os(ENV) {
            let source = PathBuf::from(path);
            let root = source.canonicalize().unwrap();
            let temp = std::env::temp_dir().canonicalize().unwrap();
            assert!(
                root.starts_with(&temp)
                    && root
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("palisade-native-file-test-")
            );
            let mut store = FileStore::new(Arc::new(AtomicBool::new(false)));
            let token = store
                .open_staged_receive(
                    &source,
                    metadata("never-completed.txt", b"fixture data"),
                    &mut || Ok(()),
                )
                .unwrap();
            store.write(&token, 0, b"fixture", || Ok(())).unwrap();
            // ExitProcess skips Rust Drop; the OS must delete the open staging file.
            std::process::exit(0);
        }
        let fixture = Fixture::new();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "windows::files::tests::process_exit_removes_owned_uncommitted_staging_file",
                "--nocapture",
            ])
            .env(ENV, &fixture.0)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 0);
    }
    #[test]
    fn selected_source_is_immutable_while_held_and_denied_before_read() {
        let fixture = Fixture::new();
        let path = fixture.0.join("source.txt");
        std::fs::write(&path, b"fixture data").unwrap();
        let mut store = fixture.store();
        assert!(store.open_send(&path, &mut || Err("revoked")).is_err());
        let (token, meta) = store.open_send(&path, &mut || Ok(())).unwrap();
        assert_eq!(meta.size, 12);
        assert!(OpenOptions::new().write(true).open(&path).is_err());
        assert_eq!(store.read(&token, 0, || Ok(())).unwrap(), b"fixture data");
        assert!(store.read(&token, 0, || Ok(())).is_err());
        store.cancel(&token);
    }
    #[test]
    fn receive_integrity_motw_atomic_commit_and_no_overwrite() {
        let fixture = Fixture::new();
        let destination = fixture.0.join("received.txt");
        let mut store = fixture.store();
        let meta = metadata("received.txt", b"fixture data");
        let token = store
            .open_receive(&destination, meta.clone(), &mut || Ok(()))
            .unwrap();
        store.write(&token, 0, b"fixture ", || Ok(())).unwrap();
        store.write(&token, 8, b"data", || Ok(())).unwrap();
        store.commit(&token, || Ok(())).unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"fixture data");
        let zone =
            std::fs::read_to_string(format!("{}:Zone.Identifier", destination.display())).unwrap();
        assert!(zone.contains("ZoneId=3"));
        assert!(
            store
                .open_receive(&destination, meta, &mut || Ok(()))
                .is_err()
        );
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
    #[test]
    fn private_receive_verifies_before_final_save_and_keeps_guest_name_out_of_staging() {
        let fixture = Fixture::new();
        let destination = fixture.0.join("chosen-by-recipient.txt");
        let mut store = fixture.store();
        let meta = metadata("offered-by-helper.txt", b"private fixture bytes");
        let token = store
            .open_staged_receive(&fixture.0, meta.clone(), &mut || Ok(()))
            .unwrap();
        assert!(!destination.exists());
        assert!(!fixture.0.join(&meta.name).exists());
        let staging: Vec<_> = std::fs::read_dir(&fixture.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(staging.len(), 1);
        assert!(
            staging[0]
                .to_string_lossy()
                .starts_with(".palisade-receive-")
        );
        store
            .write(&token, 0, b"private fixture bytes", || Ok(()))
            .unwrap();
        assert!(store.verify_receive(&token, || Ok(())).unwrap() == meta);
        assert!(!destination.exists());
        assert!(
            OpenOptions::new()
                .read(true)
                .open(fixture.0.join(&staging[0]))
                .is_err()
        );
        store.commit_to(&token, &destination, || Ok(())).unwrap();
        assert_eq!(
            std::fs::read(&destination).unwrap(),
            b"private fixture bytes"
        );
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
        assert!(
            std::fs::read_to_string(format!("{}:Zone.Identifier", destination.display()))
                .unwrap()
                .contains("ZoneId=3")
        );
    }
    #[test]
    fn whole_held_file_hash_rejects_tamper_even_when_network_hash_was_correct() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let token = store
            .open_staged_receive(&fixture.0, metadata("offered.txt", b"correct"), &mut || {
                Ok(())
            })
            .unwrap();
        store.write(&token, 0, b"correct", || Ok(())).unwrap();
        let Some(Transfer::Receive(receiver)) = store.transfers.get_mut(&token) else {
            panic!("fixture receive missing")
        };
        assert_eq!(
            hex(&receiver.hash.clone().finalize()),
            receiver.metadata.sha256
        );
        receiver.file.seek(SeekFrom::Start(0)).unwrap();
        receiver.file.write_all(b"changed").unwrap();
        assert!(matches!(
            store.verify_receive(&token, || Ok(())),
            Err("integrity-failure")
        ));
        assert!(store.transfers.is_empty());
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 0);
    }
    #[test]
    fn private_stage_cancel_or_expired_authority_never_creates_final_destination() {
        let fixture = Fixture::new();
        let destination = fixture.0.join("not-saved.txt");
        let mut store = fixture.store();
        for cancel in [false, true] {
            let token =
                store
                    .open_staged_receive(&fixture.0, metadata("offered.txt", b"bytes"), &mut || {
                        Ok(())
                    })
                    .unwrap();
            store.write(&token, 0, b"bytes", || Ok(())).unwrap();
            store.verify_receive(&token, || Ok(())).unwrap();
            if cancel {
                store.cancel(&token);
            }
            assert!(
                store
                    .commit_to(&token, &destination, || Err("revoked"))
                    .is_err()
            );
            assert!(!destination.exists());
            assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 0);
        }
    }
    #[test]
    fn final_save_race_preserves_existing_file_and_discards_owned_temporaries() {
        let fixture = Fixture::new();
        let destination = fixture.0.join("existing.txt");
        let mut store = fixture.store();
        let token = store
            .open_staged_receive(
                &fixture.0,
                metadata("offered.txt", b"bytes"),
                &mut || Ok(()),
            )
            .unwrap();
        store.write(&token, 0, b"bytes", || Ok(())).unwrap();
        store.verify_receive(&token, || Ok(())).unwrap();
        // Arrive after the early existence check, before the atomic publication.
        let mut collision_created = false;
        let result = store.commit_to(&token, &destination, || {
            if !collision_created
                && std::fs::read_dir(&fixture.0).unwrap().any(|entry| {
                    entry
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".palisade-save-")
                })
            {
                std::fs::write(&destination, b"existing").unwrap();
                collision_created = true;
            }
            Ok(())
        });
        assert!(
            result.is_err(),
            "late destination collision must refuse replacement"
        );
        assert!(collision_created);
        assert_eq!(std::fs::read(&destination).unwrap(), b"existing");
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
    #[test]
    fn final_save_revocation_during_copy_discards_both_private_handles() {
        let fixture = Fixture::new();
        let destination = fixture.0.join("never-saved.txt");
        let mut store = fixture.store();
        let bytes = vec![7; 65537];
        let token = store
            .open_staged_receive(&fixture.0, metadata("offered.txt", &bytes), &mut || Ok(()))
            .unwrap();
        store.write(&token, 0, &bytes[..65536], || Ok(())).unwrap();
        store
            .write(&token, 65536, &bytes[65536..], || Ok(()))
            .unwrap();
        let mut copy_started = false;
        assert!(
            store
                .commit_to(&token, &destination, || {
                    copy_started = std::fs::read_dir(&fixture.0).unwrap().any(|entry| {
                        entry
                            .unwrap()
                            .file_name()
                            .to_string_lossy()
                            .starts_with(".palisade-save-")
                    });
                    if copy_started { Err("revoked") } else { Ok(()) }
                })
                .is_err()
        );
        assert!(copy_started);
        assert!(!destination.exists());
        assert!(store.transfers.is_empty());
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 0);
    }
    #[test]
    fn unit_file_dialog_never_opens_user_interface() {
        let cancellation = Cancellation {
            session: Arc::new(AtomicBool::new(false)),
            operation: Arc::new(AtomicBool::new(false)),
            authority: Arc::new(AtomicBool::new(false)).into(),
        };
        assert_eq!(
            dialog(true, Some("fixture.txt"), &cancellation),
            Err("file-test-ui-disabled")
        );
        assert_eq!(
            dialog(false, None, &cancellation),
            Err("file-test-ui-disabled")
        );
    }
    #[test]
    fn collision_tamper_and_cancel_remove_only_the_owned_staging_file() {
        let fixture = Fixture::new();
        let destination = fixture.0.join("collision.txt");
        let mut store = fixture.store();
        let token = store
            .open_receive(&destination, metadata("collision.txt", b"ok"), &mut || {
                Ok(())
            })
            .unwrap();
        store.write(&token, 0, b"ok", || Ok(())).unwrap();
        std::fs::write(&destination, b"existing").unwrap();
        assert!(store.commit(&token, || Ok(())).is_err());
        assert_eq!(std::fs::read(&destination).unwrap(), b"existing");
        let other = fixture.0.join("other.txt");
        let token = store
            .open_receive(&other, metadata("other.txt", b"ok"), &mut || Ok(()))
            .unwrap();
        store.write(&token, 0, b"no", || Ok(())).unwrap();
        assert!(store.commit(&token, || Ok(())).is_err());
        assert!(!other.exists());
        let token = store
            .open_receive(&other, metadata("other.txt", b"ok"), &mut || Ok(()))
            .unwrap();
        store.cancel(&token);
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
    #[test]
    fn zero_bytes_final_revocation_quotas_and_parent_rename() {
        let fixture = Fixture::new();
        let destination = fixture.0.join("empty.txt");
        let mut store = fixture.store();
        let token = store
            .open_receive(&destination, metadata("empty.txt", b""), &mut || Ok(()))
            .unwrap();
        store.commit(&token, || Ok(())).unwrap();
        assert!(destination.exists());
        let second = fixture.0.join("second.txt");
        let token = store
            .open_receive(&second, metadata("second.txt", b"x"), &mut || Ok(()))
            .unwrap();
        store.write(&token, 0, b"x", || Ok(())).unwrap();
        assert!(store.commit(&token, || Err("revoked")).is_err());
        store.cancel(&token);
        assert!(!second.exists());
        store.quota.store(SESSION_LIMIT, Ordering::SeqCst);
        assert!(
            store
                .open_receive(&second, metadata("second.txt", b"x"), &mut || Ok(()))
                .is_err()
        );
        let nested = fixture.0.join("directory");
        std::fs::create_dir(&nested).unwrap();
        let guarded = nested.join("file.txt");
        let guard = ParentGuard::open(&guarded).unwrap();
        let renamed = fixture.0.join("renamed");
        assert!(nested.starts_with(&fixture.0) && renamed.starts_with(&fixture.0));
        assert!(std::fs::rename(&nested, &renamed).is_err());
        drop(guard);
    }
    #[test]
    fn malicious_names_are_refused() {
        for name in [
            "../bad",
            "C:bad",
            "NUL.txt",
            "COM1",
            "bad\u{202e}.exe",
            "trailing.",
            "space ",
            "bad:stream",
            "..",
            "e\u{301}.txt",
        ] {
            assert!(metadata(name, b"x").validate().is_err(), "{name}");
        }
    }
}
