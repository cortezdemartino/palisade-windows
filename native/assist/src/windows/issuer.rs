//! Native issuer qualification. Legacy issuer-v2.dpapi is never read as a
//! qualified record, replaced or silently deleted. Recovery is explicit.
use super::{https, identity};
use crate::issuer::{Candidate, Method, Record};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use windows_sys::Win32::Storage::FileSystem::*;
// A real dedicated Assist issuer pin must be reviewed and committed into a
// signed release. Never obtain this value from page IPC or a runtime response.
const RELEASE_ISSUER_FINGERPRINT: Option<&str> = None;
const RECORD: &str = "issuer-qualified-v1.dpapi";
pub struct TrustedIssuer {
    candidate: Candidate,
    method: Method,
}
impl TrustedIssuer {
    #[cfg(test)]
    pub(crate) fn fixture(candidate: Candidate) -> Self {
        Self {
            candidate,
            method: Method::Confirmed,
        }
    }
    pub(crate) fn key(&self) -> [u8; 32] {
        self.candidate.key()
    }
    pub fn fingerprint(&self) -> &str {
        self.candidate.fingerprint()
    }
    pub fn method(&self) -> Method {
        self.method
    }
    pub fn load() -> Result<Self, &'static str> {
        let candidate = current()?;
        let directory = identity::storage()?;
        let stored = read_record(&directory.join(RECORD))?;
        let method = candidate.qualify(stored.as_ref(), RELEASE_ISSUER_FINGERPRINT)?;
        if stored.is_none() {
            write_new(&directory.join(RECORD), &candidate.record(method))?;
        }
        Ok(Self { candidate, method })
    }
    // The trusted coordinator may request this narrow prompt, never supply an
    // "approved" boolean. This is not exposed on diagnostic v1 IPC.
    pub fn confirm_current(cancel: Arc<AtomicBool>) -> Result<Self, &'static str> {
        let candidate = current()?;
        let directory = identity::storage()?;
        let path = directory.join(RECORD);
        if let Some(record) = read_record(&path)? {
            let method = candidate.qualify(Some(&record), RELEASE_ISSUER_FINGERPRINT)?;
            return Ok(Self { candidate, method });
        }
        let text = format!(
            "Confirm the Palisade Assist signing authority\r\n\r\nOrigin: {}\r\nSHA-256: {}\r\n\r\nCompare this entire fingerprint with the Server Controller's Assist page using a separate trusted channel. Do not confirm a value supplied only by this connection or the person requesting help.\r\n\r\nAny earlier first-use trust record is retained but does not approve this authority. Changed qualified keys require reviewed recovery. This grants no remote permissions.",
            crate::issuer::ORIGIN,
            candidate.fingerprint()
        );
        if !super::consent::confirm_issuer(&text, cancel.clone())? {
            return Err("issuer-not-approved");
        }
        super::input::ordinary_desktop()?;
        if cancel.load(Ordering::SeqCst) || current()? != candidate {
            return Err("issuer-changed");
        }
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled");
        }
        // Exclusive creation refuses a concurrent winner and preserves old pins.
        write_new(&path, &candidate.record(Method::Confirmed))?;
        // A completed administrator trust choice may persist, but it cannot
        // deliver success or authorize an enrollment after this caller ends.
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled");
        }
        Ok(Self {
            candidate,
            method: Method::Confirmed,
        })
    }
}
fn current() -> Result<Candidate, &'static str> {
    Candidate::parse(&https::request("/api/v1/assist/issuer", None)?)
}
fn read_record(path: &Path) -> Result<Option<Record>, &'static str> {
    match OpenOptions::new()
        .read(true)
        .share_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
    {
        Ok(file) => {
            let stat = file.metadata().map_err(|_| "issuer-read")?;
            if !stat.is_file()
                || stat.len() > 4096
                || stat.len() == 0
                || stat.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            {
                return Err("issuer-corrupt");
            }
            let mut bytes = Vec::new();
            file.take(4097)
                .read_to_end(&mut bytes)
                .map_err(|_| "issuer-read")?;
            let record = serde_json::from_slice(&identity::unprotect(&bytes)?)
                .map_err(|_| "issuer-corrupt")?;
            Ok(Some(record))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("issuer-read"),
    }
}
fn write_new(path: &Path, record: &Record) -> Result<(), &'static str> {
    let bytes = serde_json::to_vec(record).map_err(|_| "invalid-issuer")?;
    let encrypted = identity::protect(&bytes)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .share_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| "issuer-create")?;
    file.write_all(&encrypted)
        .and_then(|_| file.sync_all())
        .map_err(|_| "issuer-write")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qualified_record_dpapi_never_overwrites_legacy_or_corrupt_state() {
        let directory = std::env::temp_dir().join(format!(
            "palisade-issuer-fixture-{}-{}",
            std::process::id(),
            super::super::lease::now().unwrap()
        ));
        std::fs::create_dir(&directory).unwrap();
        let legacy = directory.join("issuer-v2.dpapi");
        std::fs::write(&legacy, b"legacy-first-use").unwrap();
        let path = directory.join(RECORD);
        assert!(read_record(&path).unwrap().is_none());
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/assist-native-v1.json"
        ))
        .unwrap();
        let candidate = Candidate::parse(&serde_json::to_vec(&serde_json::json!({"protocol":"palisade.assist.control","version":1,"publicKey":fixture["issuerPublicKey"],"fingerprint":fixture["issuerFingerprint"]})).unwrap()).unwrap();
        let record = candidate.record(Method::Confirmed);
        write_new(&path, &record).unwrap();
        let stored = read_record(&path).unwrap().unwrap();
        assert_eq!(
            candidate.qualify(Some(&stored), None),
            Ok(Method::Confirmed)
        );
        assert!(write_new(&path, &record).is_err());
        assert_eq!(std::fs::read(&legacy).unwrap(), b"legacy-first-use");
        std::fs::write(&path, b"corrupt").unwrap();
        assert!(read_record(&path).is_err());
        assert!(write_new(&path, &record).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"corrupt");
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(&legacy).unwrap();
        std::fs::remove_dir(&directory).unwrap();
    }
}
