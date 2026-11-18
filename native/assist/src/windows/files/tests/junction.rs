//! Actual NTFS reparse races, confined to the disposable file-test fixture.
use super::*;

// Test-only synchronous Win32 entry point; no dependency or privileged service.
#[link(name = "kernel32")]
unsafe extern "system" {
    fn DeviceIoControl(
        device: HANDLE,
        code: u32,
        input: *const std::ffi::c_void,
        input_size: u32,
        output: *mut std::ffi::c_void,
        output_size: u32,
        returned: *mut u32,
        overlapped: *mut std::ffi::c_void,
    ) -> i32;
}
// Win32 winioctl.h FSCTL values and mount-point tag, used only by this fixture.
const SET_REPARSE: u32 = 0x0009_00a4;
const DELETE_REPARSE: u32 = 0x0009_00ac;
const MOUNT_POINT: u32 = 0xa000_0003;

fn fixture_path(path: &Path) {
    let root = path.parent().unwrap().canonicalize().unwrap();
    let temp = std::env::temp_dir().canonicalize().unwrap();
    assert!(root.starts_with(temp));
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("palisade-native-file-test-")
    );
    assert!(!path.symlink_metadata().unwrap().file_type().is_symlink());
}
fn directory(path: &Path, access: u32) -> Result<Handle, std::io::Error> {
    let name = wide(path);
    // SAFETY: synchronous open of an explicitly validated disposable directory.
    let raw = unsafe {
        CreateFileW(
            name.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(Handle(raw))
    }
}
fn pinned(path: &Path) -> bool {
    fixture_path(path);
    match directory(path, DELETE) {
        Ok(handle) => {
            drop(handle);
            false
        }
        Err(error) => {
            assert_eq!(error.raw_os_error(), Some(ERROR_SHARING_VIOLATION as i32));
            true
        }
    }
}
struct Junction(PathBuf);
impl Junction {
    fn set(path: &Path, target: &Path) -> Result<Self, std::io::Error> {
        fixture_path(path);
        fixture_path(target);
        let handle = directory(path, GENERIC_WRITE)?;
        let print: Vec<u16> = target.as_os_str().encode_wide().collect();
        let substitute: Vec<u16> = format!(r"\??\{}", target.display())
            .encode_utf16()
            .collect();
        let path_bytes = (substitute.len() + print.len() + 2) * 2;
        let mut bytes = Vec::with_capacity(16 + path_bytes);
        bytes.extend(MOUNT_POINT.to_le_bytes());
        for value in [
            8 + path_bytes as u16,
            0,
            0,
            (substitute.len() * 2) as u16,
            ((substitute.len() + 1) * 2) as u16,
            (print.len() * 2) as u16,
        ] {
            bytes.extend(value.to_le_bytes());
        }
        for value in substitute.into_iter().chain([0]).chain(print).chain([0]) {
            bytes.extend(value.to_le_bytes());
        }
        let mut returned = 0;
        // SAFETY: the bounded mount-point buffer contains both terminated names;
        // the synchronous API copies it before return. Both targets are fixtures.
        if unsafe {
            DeviceIoControl(
                handle.0,
                SET_REPARSE,
                bytes.as_ptr().cast(),
                bytes.len() as u32,
                ptr::null_mut(),
                0,
                &mut returned,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self(path.into()))
    }
}
impl Drop for Junction {
    fn drop(&mut self) {
        // Clear the junction itself before recursive fixture cleanup. Never walk
        // the substituted target while deleting the source directory.
        let handle = directory(&self.0, GENERIC_WRITE).unwrap();
        let mut bytes = [0u8; 8];
        bytes[..4].copy_from_slice(&MOUNT_POINT.to_le_bytes());
        let mut returned = 0;
        assert_ne!(
            unsafe {
                DeviceIoControl(
                    handle.0,
                    DELETE_REPARSE,
                    bytes.as_ptr().cast(),
                    8,
                    ptr::null_mut(),
                    0,
                    &mut returned,
                    ptr::null_mut(),
                )
            },
            0
        );
    }
}
fn directories(fixture: &Fixture) -> (PathBuf, PathBuf) {
    let selected = fixture.0.join("selected-directory");
    let target = fixture.0.join("redirected-directory");
    std::fs::create_dir(&selected).unwrap();
    std::fs::create_dir(&target).unwrap();
    (selected, target)
}

#[test]
fn source_junction_after_parent_guard_is_refused_before_content_hash_or_selection() {
    let fixture = Fixture::new();
    let (selected, target) = directories(&fixture);
    let selected_file = selected.join("private-fixture.txt");
    let target_file = target.join("private-fixture.txt");
    std::fs::write(
        &target_file,
        b"fixture-only content not selected by the recipient",
    )
    .unwrap();
    let mut junction = None;
    let mut store = fixture.store();
    let result = store.open_send(&selected_file, &mut || {
        if junction.is_none() && pinned(&selected) {
            junction = Some(Junction::set(&selected, &target).unwrap());
        }
        Ok(())
    });
    assert!(
        junction.is_some(),
        "attack must occur after the directory guard"
    );
    assert!(matches!(result, Err("path-binding-mismatch")));
    assert!(store.transfers.is_empty());
    assert_eq!(
        std::fs::read(&target_file).unwrap(),
        b"fixture-only content not selected by the recipient"
    );
}

#[test]
fn receive_junction_after_parent_guard_leaves_no_content_or_stray_temporary() {
    let fixture = Fixture::new();
    let (selected, target) = directories(&fixture);
    let mut junction = None;
    let mut store = fixture.store();
    let result =
        store.open_staged_receive(&selected, metadata("offered.txt", b"bytes"), &mut || {
            if junction.is_none() && pinned(&selected) {
                junction = Some(Junction::set(&selected, &target).unwrap());
            }
            Ok(())
        });
    assert!(junction.is_some());
    assert!(matches!(result, Err("path-binding-mismatch")));
    assert!(store.transfers.is_empty());
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
}

#[test]
fn save_junction_after_parent_guard_never_copies_received_content_into_target() {
    let fixture = Fixture::new();
    let (selected, target) = directories(&fixture);
    let mut store = fixture.store();
    let token = store
        .open_staged_receive(
            &fixture.0,
            metadata("offered.txt", b"fixture data"),
            &mut || Ok(()),
        )
        .unwrap();
    store.write(&token, 0, b"fixture data", || Ok(())).unwrap();
    let mut junction = None;
    let result = store.commit_to(&token, &selected.join("chosen.txt"), || {
        if junction.is_none() && pinned(&selected) {
            junction = Some(Junction::set(&selected, &target).unwrap());
        }
        Ok(())
    });
    assert!(junction.is_some());
    assert!(matches!(result, Err("path-binding-mismatch")));
    assert!(store.transfers.is_empty());
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
}

#[test]
fn verified_owned_child_keeps_native_parent_nonempty_and_refuses_in_place_junction() {
    let fixture = Fixture::new();
    let (selected, target) = directories(&fixture);
    let mut store = fixture.store();
    let token = store
        .open_staged_receive(&selected, metadata("offered.txt", b"bytes"), &mut || Ok(()))
        .unwrap();
    let error = match Junction::set(&selected, &target) {
        Ok(_) => panic!("NTFS must refuse reparse conversion of a nonempty held parent"),
        Err(error) => error,
    };
    assert_eq!(error.raw_os_error(), Some(ERROR_DIR_NOT_EMPTY as i32));
    store.write(&token, 0, b"bytes", || Ok(())).unwrap();
    store.verify_receive(&token, || Ok(())).unwrap();
    store.cancel(&token);
    assert_eq!(std::fs::read_dir(&selected).unwrap().count(), 0);
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
}
