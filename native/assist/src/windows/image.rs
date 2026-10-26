//! A child launch re-verifies the protected build manifest and keeps the image
//! and ancestors locked against replacement through CreateProcess.
use super::files::ParentGuard;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::Read,
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::*;
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageManifest {
    pub filename: String,
    pub protocol: String,
    pub version: u32,
    pub sha256: String,
    pub size: u64,
}
pub(crate) struct VerifiedImage {
    path: PathBuf,
    _file: File,
    _parents: ParentGuard,
}
impl VerifiedImage {
    pub(crate) fn open(manifest: &ImageManifest) -> Result<Self, &'static str> {
        if manifest.filename != "Palisade-Assist.exe"
            || manifest.protocol != "palisade.assist.guardian"
            || manifest.version != 1
            || manifest.size == 0
            || manifest.size > 20 * 1024 * 1024
            || manifest.sha256.len() != 64
        {
            return Err("helper-integrity");
        }
        let path = std::env::current_exe().map_err(|_| "helper-image")?;
        let parents = ParentGuard::open(&path)?;
        let mut file = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&path)
            .map_err(|_| "helper-image")?;
        let stat = file.metadata().map_err(|_| "helper-image")?;
        if !stat.is_file()
            || stat.len() != manifest.size
            || stat.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err("helper-integrity");
        }
        let mut hash = Sha256::new();
        let mut bytes = [0u8; 65536];
        loop {
            let count = file.read(&mut bytes).map_err(|_| "helper-image")?;
            if count == 0 {
                break;
            }
            hash.update(&bytes[..count]);
        }
        let actual = hash
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if actual != manifest.sha256 {
            return Err("helper-integrity");
        }
        Ok(Self {
            path,
            _file: file,
            _parents: parents,
        })
    }
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}
