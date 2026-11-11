//! Cross-session update exclusion. Product-wide exclusion also covers installation aliases.
use super::stage::{Handle, user_sid};
use std::{mem::size_of, ptr};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    System::Threading::*,
};
type Result<T> = std::result::Result<T, &'static str>;
pub const INSTALL_GATE: &str = "Global\\Palisade-Update-org.onyxops.palisade-v1";
pub const PERMIT_PREFIX: &str = "Global\\Palisade-Update-Installer-v1-";

fn secured<T>(
    shared_read: bool,
    create: impl FnOnce(&SECURITY_ATTRIBUTES) -> Result<T>,
) -> Result<T> {
    let sid = user_sid()?;
    let sddl: Vec<u16> = format!(
        "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;{sid}){}",
        if shared_read {
            "(A;;0x00100000;;;AU)"
        } else {
            ""
        }
    )
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
        return Err("update-lock-security");
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    let result = create(&attributes);
    unsafe {
        LocalFree(descriptor);
    }
    result
}

pub struct UpdateLock(Handle);
impl UpdateLock {
    fn acquire(name: &str, shared_read: bool) -> Result<Self> {
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        secured(shared_read, |attributes| {
            let raw = unsafe {
                CreateMutexExW(
                    attributes,
                    name.as_ptr(),
                    CREATE_MUTEX_INITIAL_OWNER,
                    MUTEX_MODIFY_STATE | SYNCHRONIZATION_SYNCHRONIZE,
                )
            };
            if raw.is_null() {
                return Err("update-busy");
            }
            let handle = Handle(raw);
            // Existing means an owner or observer still retains this operation, even after a crash.
            if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                return Err("update-busy");
            }
            Ok(Self(handle))
        })
    }
    pub fn install() -> Result<Self> {
        Self::acquire(INSTALL_GATE, true)
    }
    pub fn cache() -> Result<Self> {
        Self::acquire(
            &format!("Global\\Palisade-Update-Cache-v1-{}", user_sid()?),
            false,
        )
    }
}
impl Drop for UpdateLock {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0.0);
        }
    }
}

pub fn installing() -> Result<bool> {
    present(INSTALL_GATE)
}
fn present(name: &str) -> Result<bool> {
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let raw = unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr()) };
    if !raw.is_null() {
        drop(Handle(raw));
        return Ok(true);
    }
    match unsafe { GetLastError() } {
        ERROR_FILE_NOT_FOUND => Ok(false),
        ERROR_ACCESS_DENIED => Ok(true),
        _ => Err("update-lock-query"),
    }
}

pub struct InstallerPermit {
    _handle: Handle,
}
impl InstallerPermit {
    pub fn create(nonce: &str) -> Result<Self> {
        if nonce.len() != 64
            || !nonce
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("handoff-nonce");
        }
        Self::named(&format!("{PERMIT_PREFIX}{nonce}"))
    }
    fn named(name: &str) -> Result<Self> {
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        secured(true, |attributes| {
            let raw = unsafe {
                CreateEventExW(
                    attributes,
                    name.as_ptr(),
                    CREATE_EVENT_INITIAL_SET,
                    EVENT_ALL_ACCESS,
                )
            };
            if raw.is_null() {
                return Err("installer-permit");
            }
            let handle = Handle(raw);
            if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                return Err("installer-permit-replay");
            }
            Ok(Self { _handle: handle })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn name() -> String {
        let fixture = super::super::stage::tests::fixture();
        let name = format!("Global\\{}", fixture.file_name().unwrap().to_string_lossy());
        std::fs::remove_dir(fixture).unwrap();
        name
    }
    #[test]
    fn retained_observer_prevents_replacement_after_owner_release() {
        let name = name();
        assert!(!present(&name).unwrap());
        let owner = UpdateLock::acquire(&name, true).unwrap();
        assert!(present(&name).unwrap());
        assert!(UpdateLock::acquire(&name, true).is_err());
        let words: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let observer =
            Handle(unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, words.as_ptr()) });
        assert!(!observer.0.is_null());
        drop(owner);
        assert!(present(&name).unwrap());
        assert!(UpdateLock::acquire(&name, true).is_err());
        drop(observer);
        assert!(!present(&name).unwrap());
        assert!(UpdateLock::acquire(&name, true).is_ok());
    }
    #[test]
    fn installer_permit_is_single_use_and_cannot_be_recreated_while_retained() {
        let name = name();
        let permit = InstallerPermit::named(&name).unwrap();
        let words: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let observer =
            Handle(unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, words.as_ptr()) });
        assert!(!observer.0.is_null());
        assert_eq!(unsafe { WaitForSingleObject(observer.0, 0) }, WAIT_OBJECT_0);
        assert_eq!(unsafe { WaitForSingleObject(observer.0, 0) }, WAIT_TIMEOUT);
        assert!(InstallerPermit::named(&name).is_err());
        drop(permit);
        assert!(InstallerPermit::named(&name).is_err());
        drop(observer);
        assert!(InstallerPermit::named(&name).is_ok());
    }
}
