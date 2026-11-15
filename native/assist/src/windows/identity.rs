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
    sync::Mutex,
    time::Instant,
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
/// Operational endpoint signer. Create once per native authority launch and
/// retain that exact instance for enrollment, ACK, lease and transport purposes.
/// It is never loaded from the installation's persistent DPAPI identity.
pub struct EndpointIdentity(SigningKey, String, LeaseLane);
#[derive(Default)]
struct RequestClock {
    wall: u64,
    issued: u64,
}
#[derive(Default)]
struct LeaseLane(Mutex<RequestClock>);
impl LeaseLane {
    fn request<T>(
        &self,
        cancelled: impl Fn() -> bool,
        clock: impl Fn() -> Result<u64, &'static str>,
        send: impl FnOnce(u64, Instant) -> Result<T, &'static str>,
    ) -> Result<T, &'static str> {
        if cancelled() {
            return Err("revoked");
        }
        // The service's replay floors are per endpoint, not per Assist session.
        // Lease and ICE use this same launch-owned lane and purpose-limited
        // statements; neither route can reuse a stamp after uncertain delivery.
        // Retain this lock through delivery so concurrent sessions cannot send
        // higher timestamps ahead of an earlier authenticated request.
        let mut previous = self.0.lock().map_err(|_| "lease-request-failed")?;
        if cancelled() {
            return Err("revoked");
        }
        let now = clock()?;
        if now < previous.wall {
            return Err("clock-not-increasing");
        }
        let issued = now.max(previous.issued.checked_add(1).ok_or("invalid-time")?);
        if issued > 8_640_000_000_000_000 || issued.saturating_sub(now) > 1000 {
            return Err("clock-not-increasing");
        }
        previous.wall = now;
        previous.issued = issued; // a failed delivery must never reuse its stamp
        let result = send(issued, Instant::now())?;
        if cancelled() {
            return Err("revoked");
        }
        Ok(result)
    }
}
pub(crate) struct LeaseResponse {
    pub bytes: Vec<u8>,
    pub started: Instant,
}
impl Identity {
    pub fn load_current_user() -> Result<Self, &'static str> {
        Self::load(&storage()?.join("endpoint-v1.dpapi"))
    }
    pub fn public_key(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0.verifying_key().as_bytes())
    }
    #[cfg(test)]
    pub(crate) fn fixture(path: &Path) -> Result<Self, &'static str> {
        Self::load(path)
    }
}
impl EndpointIdentity {
    pub fn for_launch(installation: &Identity) -> Result<Self, &'static str> {
        let seed = random_seed()?;
        Ok(Self(
            SigningKey::from_bytes(&seed),
            installation.public_key(),
            LeaseLane::default(),
        ))
    }
    pub fn public_key(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0.verifying_key().as_bytes())
    }
    pub(crate) fn installation_reference(&self) -> &str {
        &self.1
    }
    pub(crate) fn prove_enrollment(
        &self,
        record: &super::enrollment::Enrollment,
        challenge: &str,
    ) -> Result<String, &'static str> {
        Ok(URL_SAFE_NO_PAD.encode(self.0.sign(&record.statement(self, challenge)?).to_bytes()))
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
    pub(crate) fn transport(
        &self,
        binding: &crate::stream::Binding,
        own: &str,
    ) -> Result<String, &'static str> {
        Ok(URL_SAFE_NO_PAD.encode(
            self.0
                .sign(&crate::statements::transport(
                    &binding.session,
                    &binding.endpoint,
                    own,
                    &binding.helper_transport,
                )?)
                .to_bytes(),
        ))
    }
}
pub(crate) fn peer_token() -> Result<String, &'static str> {
    Ok(URL_SAFE_NO_PAD.encode(random_seed()?.as_ref()))
}
fn random_seed() -> Result<Zeroizing<[u8; 32]>, &'static str> {
    let mut seed = Zeroizing::new([0u8; 32]);
    // SAFETY: valid fixed mutable buffer and the Windows system RNG. Failure
    // has no persisted-key fallback and cannot advertise an operational key.
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
    Ok(seed)
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
    let product = match crate::qualification::namespace()? {
        Some(namespace) => path.join(format!("Palisade-Assist-Qualification-{namespace}")),
        None => path.join("Palisade"),
    };
    let base = product.join("Assist");
    fs::create_dir_all(&base).map_err(|_| "identity-location")?;
    for directory in [&path, &product, &base] {
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
                let seed = random_seed()?;
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
}
impl EndpointIdentity {
    pub(crate) fn request_ice(
        &self,
        endpoint: &str,
        cancelled: impl Fn() -> bool,
    ) -> Result<super::ice::Configuration, &'static str> {
        self.2.request(&cancelled, super::lease::now, |issued_at, started| {
            let signature=self.sign_ice_request(endpoint,issued_at)?;
            let body=serde_json::to_vec(&serde_json::json!({"endpointId":endpoint,"issuedAt":issued_at,"signature":signature})).map_err(|_| "invalid-message")?;
            if cancelled() { return Err("revoked"); }
            let bytes=Zeroizing::new(super::https::request("/api/v1/assist/streams/endpoint-ice",Some(&body))?);
            if cancelled() { return Err("revoked"); }
            super::ice::Configuration::parse(&bytes,started,super::lease::now()?,Instant::now())
        })
    }
    fn sign_ice_request(&self, endpoint: &str, issued_at: u64) -> Result<String, &'static str> {
        Ok(URL_SAFE_NO_PAD.encode(
            self.0
                .sign(&crate::statements::ice(endpoint, issued_at)?)
                .to_bytes(),
        ))
    }
    pub(crate) fn request_lease(
        &self,
        endpoint: &str,
        session: &str,
        cancelled: impl Fn() -> bool,
    ) -> Result<LeaseResponse, &'static str> {
        self.2.request(cancelled, super::lease::now, |issued_at, started| {
            let signature=self.sign_lease_request(endpoint,session,issued_at)?;
            let body=serde_json::to_vec(&serde_json::json!({"endpointId":endpoint,"session":session,"issuedAt":issued_at,"signature":signature})).map_err(|_| "invalid-message")?;
            let bytes=super::https::request("/api/v1/assist/streams/endpoint-lease",Some(&body))?;
            Ok(LeaseResponse { bytes, started })
        })
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
    fn endpoint_ice_signature_is_purpose_limited_to_the_current_operational_key() {
        let installation = Identity(SigningKey::from_bytes(&random_seed().unwrap()));
        let signer = EndpointIdentity::for_launch(&installation).unwrap();
        let other = EndpointIdentity::for_launch(&installation).unwrap();
        let endpoint = "endpoint_ice_fixture";
        let time = 1_800_000_000_000;
        let signature = Signature::from_bytes(
            &decode::<64>(&signer.sign_ice_request(endpoint, time).unwrap()).unwrap(),
        );
        let key = VerifyingKey::from_bytes(&decode::<32>(&signer.public_key()).unwrap()).unwrap();
        key.verify_strict(&crate::statements::ice(endpoint, time).unwrap(), &signature)
            .unwrap();
        assert!(
            key.verify_strict(
                &crate::statements::lease(endpoint, "session_fixture_01", time).unwrap(),
                &signature
            )
            .is_err()
        );
        let previous =
            VerifyingKey::from_bytes(&decode::<32>(&other.public_key()).unwrap()).unwrap();
        assert!(
            previous
                .verify_strict(&crate::statements::ice(endpoint, time).unwrap(), &signature)
                .is_err()
        );
        assert!(
            signer
                .sign_ice_request("endpoint_ice_fixture\nother", time)
                .is_err()
        );
    }
    #[test]
    fn canceled_endpoint_ice_request_never_allocates_a_stamp_or_starts_https() {
        let installation = Identity(SigningKey::from_bytes(&random_seed().unwrap()));
        let signer = EndpointIdentity::for_launch(&installation).unwrap();
        assert!(matches!(
            signer.request_ice("endpoint_ice_fixture", || true),
            Err("revoked")
        ));
        assert_eq!(signer.2.0.lock().unwrap().issued, 0);
    }
    #[test]
    fn endpoint_lease_requests_share_timestamp_and_delivery_order_across_sessions() {
        use std::sync::{Arc, mpsc};
        let lane = Arc::new(LeaseLane::default());
        let order = Arc::new(Mutex::new(Vec::new()));
        let (entered, seen) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let first_lane = lane.clone();
        let first_order = order.clone();
        let first = std::thread::spawn(move || {
            first_lane.request(
                || false,
                || Ok(1000),
                |stamp, _| {
                    first_order.lock().unwrap().push(stamp);
                    entered.send(()).unwrap();
                    wait.recv().unwrap();
                    Ok(stamp)
                },
            )
        });
        seen.recv().unwrap();
        assert!(matches!(
            lane.0.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        ));
        let second_lane = lane.clone();
        let second_order = order.clone();
        let second = std::thread::spawn(move || {
            second_lane.request(
                || false,
                || Ok(1000),
                |stamp, _| {
                    second_order.lock().unwrap().push(stamp);
                    Ok(stamp)
                },
            )
        });
        release.send(()).unwrap();
        assert_eq!(first.join().unwrap(), Ok(1000));
        assert_eq!(second.join().unwrap(), Ok(1001));
        assert_eq!(*order.lock().unwrap(), [1000, 1001]);
    }
    #[test]
    fn failed_or_canceled_delivery_never_reuses_an_endpoint_stamp() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let lane = LeaseLane::default();
        let cancel = AtomicBool::new(false);
        assert_eq!(
            lane.request(
                || false,
                || Ok(1000),
                |_, _| Err::<(), _>("network-failure")
            ),
            Err("network-failure")
        );
        assert_eq!(
            lane.request(
                || cancel.load(Ordering::SeqCst),
                || Ok(1000),
                |stamp, _| {
                    assert_eq!(stamp, 1001);
                    cancel.store(true, Ordering::SeqCst);
                    Ok(())
                }
            ),
            Err("revoked")
        );
        assert_eq!(
            lane.request(|| false, || Ok(1000), |stamp, _| Ok(stamp)),
            Ok(1002)
        );
        assert_eq!(
            lane.request(
                || true,
                || panic!("canceled work must not allocate time"),
                |_, _| Ok(())
            ),
            Err("revoked")
        );
    }
    #[test]
    fn canceled_waiter_cannot_start_network_work_after_a_sibling_response() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        };
        let lane = Arc::new(LeaseLane::default());
        let cancel = Arc::new(AtomicBool::new(false));
        let (entered, seen) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let first_lane = lane.clone();
        let first = std::thread::spawn(move || {
            first_lane.request(
                || false,
                || Ok(1000),
                |_, _| {
                    entered.send(()).unwrap();
                    wait.recv().unwrap();
                    Ok(())
                },
            )
        });
        seen.recv().unwrap();
        let next_lane = lane.clone();
        let next_cancel = cancel.clone();
        let (checked, observed_check) = mpsc::channel();
        let second = std::thread::spawn(move || {
            let first_check = AtomicBool::new(true);
            next_lane.request(
                || {
                    let stopped = next_cancel.load(Ordering::SeqCst);
                    if first_check.swap(false, Ordering::SeqCst) {
                        checked.send(()).unwrap();
                    }
                    stopped
                },
                || Ok(1000),
                |_, _| panic!("canceled waiter must not send"),
            )
        });
        observed_check.recv().unwrap(); // cancellation follows the initial false snapshot
        cancel.store(true, Ordering::SeqCst);
        release.send(()).unwrap();
        first.join().unwrap().unwrap();
        assert_eq!(second.join().unwrap(), Err::<(), _>("revoked"));
        assert_eq!(lane.0.lock().unwrap().issued, 1000);
    }
    #[test]
    fn native_request_time_refuses_clock_rollback_exhaustion_and_unbounded_future_stamps() {
        let lane = LeaseLane::default();
        lane.request(|| false, || Ok(1000), |_, _| Ok(())).unwrap();
        assert_eq!(
            lane.request(|| false, || Ok(999), |_, _| Ok(())),
            Err("clock-not-increasing")
        );
        lane.0.lock().unwrap().issued = 2000;
        assert_eq!(
            lane.request(|| false, || Ok(1000), |_, _| Ok(())),
            Err("clock-not-increasing")
        );
        assert_eq!(
            lane.request(|| false, || Ok(8_640_000_000_000_001), |_, _| Ok(())),
            Err("clock-not-increasing")
        );
    }
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
    #[test]
    fn operational_launch_keys_are_fresh_without_replacing_the_dpapi_installation() {
        let dir = std::env::temp_dir().join(format!(
            "palisade-operational-key-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("installation.dpapi");
        let installation = Identity::load(&path).unwrap();
        let original = fs::read(&path).unwrap();
        let first = EndpointIdentity::for_launch(&installation).unwrap();
        let next = EndpointIdentity::for_launch(&installation).unwrap();
        assert_ne!(first.public_key(), next.public_key());
        assert_ne!(first.public_key(), installation.public_key());
        assert_ne!(next.public_key(), installation.public_key());
        assert_eq!(first.installation_reference(), installation.public_key());
        assert_eq!(next.installation_reference(), installation.public_key());
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(
            Identity::load(&path).unwrap().public_key(),
            installation.public_key()
        );
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&dir).unwrap();
    }
    #[test]
    fn old_endpoint_public_key_refuses_a_new_launch_lease_authentication() {
        // A validated installation object is required by the production API;
        // the existing persistence test separately verifies its DPAPI loader.
        let installation = Identity(SigningKey::from_bytes(&random_seed().unwrap()));
        let old = EndpointIdentity::for_launch(&installation).unwrap();
        let current = EndpointIdentity::for_launch(&installation).unwrap();
        let endpoint = "endpoint_restart_fixture";
        let session = "sassist_restart_fixture";
        let at = 1_800_000_300_000;
        let bytes = crate::statements::lease(endpoint, session, at).unwrap();
        let signature = current.sign_lease_request(endpoint, session, at).unwrap();
        let signature = Signature::from_bytes(&decode::<64>(&signature).unwrap());
        let previous = VerifyingKey::from_bytes(&decode::<32>(&old.public_key()).unwrap()).unwrap();
        let published =
            VerifyingKey::from_bytes(&decode::<32>(&current.public_key()).unwrap()).unwrap();
        assert!(previous.verify_strict(&bytes, &signature).is_err());
        published.verify_strict(&bytes, &signature).unwrap();
        // A page cannot present a lease to a new gate: the native poller must
        // authenticate with this same operational key before native admission.
    }
}
