use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signer, SigningKey};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::windows::{
        ffi::OsStringExt,
        fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::Cryptography::*,
    Storage::FileSystem::{FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT},
    System::Com::CoTaskMemFree,
    UI::Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath},
};
use zeroize::{Zeroize, Zeroizing};

const ENTROPY: &[u8] = b"Palisade Assist endpoint identity v1";
pub struct Identity(SigningKey);
impl Identity {
    pub fn load_current_user() -> Result<Self, &'static str> {
        Self::load(&storage()?.join("endpoint-v1.dpapi"))
    }
    pub fn public_key(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0.verifying_key().as_bytes())
    }
    pub(crate) fn acknowledge(
        &self,
        grant: &crate::stream::StreamGrant,
    ) -> Result<String, &'static str> {
        Ok(URL_SAFE_NO_PAD.encode(
            self.0
                .sign(&crate::statements::acknowledgement(grant)?)
                .to_bytes(),
        ))
    }
}
pub(crate) fn storage() -> Result<PathBuf, &'static str> {
    let mut raw = ptr::null_mut();
    // SAFETY: out pointer is owned by the shell, freed using CoTaskMemFree.
    let path = unsafe {
        if SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, ptr::null_mut(), &mut raw) < 0 {
            return Err("identity-location");
        }
        let mut len = 0;
        while *raw.add(len) != 0 {
            len += 1;
        }
        let path = PathBuf::from(std::ffi::OsString::from_wide(std::slice::from_raw_parts(
            raw, len,
        )));
        CoTaskMemFree(raw.cast());
        path
    };
    let base = path.join("Palisade").join("Assist");
    fs::create_dir_all(&base).map_err(|_| "identity-location")?;
    for directory in [&path, &path.join("Palisade"), &base] {
        refuse_reparse(directory)?;
    }
    Ok(base)
}
impl Identity {
    // Private to the native module; no path-taking operation is exposed over IPC.
    fn load(path: &Path) -> Result<Self, &'static str> {
        match OpenOptions::new()
            .read(true)
            .share_mode(0)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
        {
            Ok(file) => {
                refuse_reparse(path)?;
                let meta = file.metadata().map_err(|_| "identity-read")?;
                if !meta.is_file()
                    || meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
                    || meta.len() > 4096
                    || meta.len() < 32
                {
                    return Err("identity-corrupt");
                }
                let mut encrypted = Vec::new();
                file.take(4097)
                    .read_to_end(&mut encrypted)
                    .map_err(|_| "identity-read")?;
                let plaintext = unprotect(&encrypted)?;
                let seed: &[u8; 32] = plaintext
                    .as_slice()
                    .try_into()
                    .map_err(|_| "identity-corrupt")?;
                Ok(Self(SigningKey::from_bytes(seed)))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut seed = Zeroizing::new([0u8; 32]);
                // SAFETY: valid 32-byte mutable buffer, OS system random provider.
                if unsafe {
                    BCryptGenRandom(
                        ptr::null_mut(),
                        seed.as_mut_ptr(),
                        32,
                        BCRYPT_USE_SYSTEM_PREFERRED_RNG,
                    )
                } < 0
                {
                    return Err("identity-random");
                }
                let identity = Self(SigningKey::from_bytes(&seed));
                let protected = protect(seed.as_slice())?;
                // Exclusive creation: never overwrite, reset or replace an existing identity.
                let mut file = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .share_mode(0)
                    .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
                    .open(path)
                    .map_err(|_| "identity-create")?;
                file.write_all(&protected)
                    .and_then(|_| file.sync_all())
                    .map_err(|_| "identity-write")?;
                Ok(identity)
            }
            Err(_) => Err("identity-read"),
        }
    }
    pub fn sign_lease_request(
        &self,
        endpoint: &str,
        session: &str,
        issued_at: u64,
    ) -> Result<String, &'static str> {
        Ok(URL_SAFE_NO_PAD.encode(
            self.0
                .sign(&crate::statements::lease(endpoint, session, issued_at)?)
                .to_bytes(),
        ))
    }
}
fn refuse_reparse(path: &Path) -> Result<(), &'static str> {
    if fs::symlink_metadata(path)
        .map_err(|_| "identity-location")?
        .file_attributes()
        & FILE_ATTRIBUTE_REPARSE_POINT
        != 0
    {
        return Err("identity-reparse");
    }
    Ok(())
}
pub(crate) fn protect(data: &[u8]) -> Result<Vec<u8>, &'static str> {
    dpapi(data, false).map(|v| v.to_vec())
}
pub(crate) fn unprotect(data: &[u8]) -> Result<Zeroizing<Vec<u8>>, &'static str> {
    dpapi(data, true)
}
fn dpapi(data: &[u8], decrypt: bool) -> Result<Zeroizing<Vec<u8>>, &'static str> {
    if data.len() > 4096 {
        return Err("identity-size");
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr().cast_mut(),
    };
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: ENTROPY.len() as u32,
        pbData: ENTROPY.as_ptr().cast_mut(),
    };
    let mut output: CRYPT_INTEGER_BLOB = unsafe { std::mem::zeroed() };
    // SAFETY: DPAPI owns output allocation. UI is forbidden; LOCAL_MACHINE is never set.
    unsafe {
        let ok = if decrypt {
            CryptUnprotectData(
                &input,
                ptr::null_mut(),
                &entropy,
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptProtectData(
                &input,
                ptr::null(),
                &entropy,
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err("identity-protection");
        }
        let allocation = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let copy = Zeroizing::new(allocation.to_vec());
        allocation.zeroize();
        LocalFree(output.pbData.cast());
        Ok(copy)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::grant::decode;
    use ed25519_dalek::{Signature, VerifyingKey};
    #[test]
    fn real_current_user_dpapi_persistence_and_corruption_refusal() {
        let dir = std::env::temp_dir().join(format!(
            "palisade-assist-identity-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("fixture.dpapi");
        let first = Identity::load(&path).unwrap();
        let public = first.public_key();
        let second = Identity::load(&path).unwrap();
        assert_eq!(second.public_key(), public);
        let key = VerifyingKey::from_bytes(&decode::<32>(&public).unwrap()).unwrap();
        key.verify_strict(
            &[9; 32],
            &Signature::from_bytes(&second.0.sign(&[9; 32]).to_bytes()),
        )
        .unwrap();
        fs::write(&path, vec![1; 200]).unwrap();
        assert!(Identity::load(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), vec![1; 200]);
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&dir).unwrap();
    }
}
