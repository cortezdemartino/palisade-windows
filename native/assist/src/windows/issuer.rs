//! First-use local confirmation, pinned for subsequent sessions. A key change
//! requires an authenticated migration/reviewed recovery, never a page override.
use super::{https, identity};
use crate::grant::decode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    ptr,
};
use windows_sys::{
    Win32::{Storage::FileSystem::*, UI::WindowsAndMessaging::*},
    core::w,
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Issuer {
    protocol: String,
    version: u32,
    public_key: String,
    fingerprint: String,
}
pub struct TrustedIssuer([u8; 32]);
impl TrustedIssuer {
    pub(crate) fn key(&self) -> [u8; 32] {
        self.0
    }
    pub fn load() -> Result<Self, &'static str> {
        let response = https::request("/api/v1/assist/issuer", None)?;
        let record: Issuer = serde_json::from_slice(&response).map_err(|_| "invalid-issuer")?;
        let key = decode::<32>(&record.public_key)?;
        let fingerprint = Sha256::digest(key)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if record.protocol != "palisade.assist.control"
            || record.version != 1
            || fingerprint != record.fingerprint
        {
            return Err("invalid-issuer");
        }
        let path = identity::storage()?.join("issuer-v2.dpapi");
        match OpenOptions::new()
            .read(true)
            .share_mode(0)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&path)
        {
            Ok(file) => {
                let stat = file.metadata().map_err(|_| "issuer-read")?;
                if !stat.is_file()
                    || stat.len() > 4096
                    || stat.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
                {
                    return Err("issuer-corrupt");
                }
                let mut bytes = Vec::new();
                file.take(4097)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "issuer-read")?;
                if identity::unprotect(&bytes)?.as_slice() != key {
                    return Err("issuer-changed");
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let text=format!("Trust this Palisade Assist authority for this Windows account?\n\nOrigin: https://palisade.onyxops.org\nSHA-256: {fingerprint}\n\nOnly locally approved, session-bound permissions can operate this device. A future key change will be refused. Choose No if this fingerprint is unexpected.").encode_utf16().chain(Some(0)).collect::<Vec<_>>();
                if unsafe {
                    MessageBoxW(
                        ptr::null_mut(),
                        text.as_ptr(),
                        w!("Palisade Assist authority"),
                        MB_YESNO | MB_DEFBUTTON2 | MB_ICONWARNING | MB_TOPMOST,
                    )
                } != IDYES
                {
                    return Err("issuer-not-approved");
                }
                let encrypted = identity::protect(&key)?;
                let mut file = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .share_mode(0)
                    .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
                    .open(path)
                    .map_err(|_| "issuer-create")?;
                file.write_all(&encrypted)
                    .and_then(|_| file.sync_all())
                    .map_err(|_| "issuer-write")?;
            }
            Err(_) => return Err("issuer-read"),
        }
        Ok(Self(key))
    }
}
