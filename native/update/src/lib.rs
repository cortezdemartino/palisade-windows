//! Offline installer authority. No cache, CLI argument or network response can supply trust.
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::io::{Read, Seek, SeekFrom};

#[cfg(windows)]
pub mod windows;

pub const MAX_ENVELOPE: usize = 32768;
pub const ROOT_PEM: &str = include_str!("../../../assets/update-public-key.pem");
pub const APP_ID: &str = "org.onyxops.palisade";
pub const UNINSTALL_GUID: &str = "df46b9fb-34da-5b35-beb4-b21c48ce542f";
const PACKAGED_TRANSITIONS: &str = include_str!("../../../assets/update-key-transitions.json");
const PACKAGE: &str = include_str!("../../../package.json");
const SPKI_PREFIX: [u8; 12] = [0x30, 0x2a, 0x30, 5, 6, 3, 0x2b, 0x65, 0x70, 3, 0x21, 0];
type Result<T> = std::result::Result<T, &'static str>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Payload {
    pub schema: u8,
    pub channel: String,
    pub version: String,
    pub filename: String,
    pub sha512: String,
    pub size: u64,
    pub published_at: String,
    pub sha256: String,
    pub download_url: String,
}

/// Only authenticated release verification can construct installation authority.
#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedRelease {
    payload: Payload,
}
impl VerifiedRelease {
    pub fn payload(&self) -> &Payload {
        &self.payload
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Transition {
    schema: u8,
    from: String,
    to_public_key: String,
    min_version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Delegation {
    payload: Transition,
    signature: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: Payload,
    signature: String,
    transitions: Vec<Delegation>,
}

pub fn stable_version(value: &str) -> Result<[u64; 3]> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() != 3 {
        return Err("version-format");
    }
    let mut result = [0; 3];
    for (index, part) in parts.into_iter().enumerate() {
        if part.is_empty()
            || part.len() > 16
            || !part.bytes().all(|b| b.is_ascii_digit())
            || (part.starts_with('0') && part.len() != 1)
        {
            return Err("version-format");
        }
        result[index] = part.parse::<u64>().map_err(|_| "version-format")?;
        if result[index] > 9_007_199_254_740_991 {
            return Err("version-range");
        }
    }
    Ok(result)
}

pub fn packaged_version() -> Result<String> {
    let package: serde_json::Value =
        serde_json::from_str(PACKAGE).map_err(|_| "packaged-version")?;
    if package["build"]["appId"].as_str() != Some(APP_ID) {
        return Err("packaged-identity");
    }
    let version = package["version"].as_str().ok_or("packaged-version")?;
    stable_version(version)?;
    Ok(version.to_owned())
}

fn decode<const N: usize>(value: &str) -> Result<[u8; N]> {
    let bytes = STANDARD.decode(value).map_err(|_| "base64-format")?;
    if STANDARD.encode(&bytes) != value {
        return Err("base64-format");
    }
    bytes.try_into().map_err(|_| "base64-length")
}

fn public_key(pem: &str) -> Result<(VerifyingKey, String)> {
    if pem.len() > 512 {
        return Err("public-key-format");
    }
    let pem = pem.replace("\r\n", "\n");
    let body = pem
        .trim()
        .strip_prefix("-----BEGIN PUBLIC KEY-----\n")
        .and_then(|value| value.strip_suffix("\n-----END PUBLIC KEY-----"))
        .ok_or("public-key-format")?;
    let der: [u8; 44] = decode(body)?;
    if der[..12] != SPKI_PREFIX {
        return Err("public-key-algorithm");
    }
    let key = VerifyingKey::from_bytes(der[12..].try_into().map_err(|_| "public-key-length")?)
        .map_err(|_| "public-key-value")?;
    Ok((
        key,
        format!(
            "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----\n",
            STANDARD.encode(der)
        ),
    ))
}

fn fingerprint(key: &VerifyingKey) -> String {
    let mut hash = Sha256::new();
    hash.update(SPKI_PREFIX);
    hash.update(key.as_bytes());
    to_hex(hash.finalize().as_ref())
}

fn signed(domain: &str, value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = format!("{domain}\n").into_bytes();
    bytes.extend(serde_json::to_vec(value).map_err(|_| "canonical-json")?);
    Ok(bytes)
}

fn authenticate(key: &VerifyingKey, bytes: &[u8], signature: &str) -> Result<()> {
    key.verify_strict(bytes, &Signature::from_bytes(&decode(signature)?))
        .map_err(|_| "invalid-signature")
}

fn normalise_transition(value: &Transition) -> Result<Transition> {
    if value.schema != 1 || !hex(&value.from, 64) {
        return Err("transition-format");
    }
    stable_version(&value.min_version)?;
    let mut result = value.clone();
    result.to_public_key = public_key(&value.to_public_key)?.1;
    Ok(result)
}

fn trust(root: &str, history: &[Delegation], version: &str) -> Result<VerifyingKey> {
    if history.len() > 8 {
        return Err("transition-limit");
    }
    let (mut key, _) = public_key(root)?;
    let mut seen = vec![fingerprint(&key)];
    let mut floor = [0; 3];
    let version = stable_version(version)?;
    for entry in history {
        let payload = normalise_transition(&entry.payload)?;
        let activation = stable_version(&payload.min_version)?;
        let (successor, _) = public_key(&payload.to_public_key)?;
        let next = fingerprint(&successor);
        if payload.from != fingerprint(&key)
            || seen.contains(&next)
            || activation <= floor
            || activation > version
        {
            return Err("unauthorized-transition");
        }
        authenticate(
            &key,
            &signed("PALISADE-DESKTOP-KEY-TRANSITION-V1", &payload)?,
            &entry.signature,
        )?;
        key = successor;
        floor = activation;
        seen.push(next);
    }
    Ok(key)
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|b| {
            [
                DIGITS[(b >> 4) as usize] as char,
                DIGITS[(b & 15) as usize] as char,
            ]
        })
        .collect()
}

fn timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 24
        || !bytes.is_ascii()
        || [4, 7, 10, 13, 16, 19, 23]
            .into_iter()
            .zip(b"--T::.Z".iter())
            .any(|(i, b)| bytes[i] != *b)
        || bytes
            .iter()
            .enumerate()
            .any(|(i, b)| ![4, 7, 10, 13, 16, 19, 23].contains(&i) && !b.is_ascii_digit())
    {
        return false;
    }
    let number = |start, end| value[start..end].parse::<u32>().unwrap_or(u32::MAX);
    let year = number(0, 4);
    let month = number(5, 7);
    let day = number(8, 10);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    };
    day > 0 && day <= days && number(11, 13) < 24 && number(14, 16) < 60 && number(17, 19) < 60
}

fn validate(payload: &Payload) -> Result<()> {
    stable_version(&payload.version)?;
    let filename = format!("Palisade-Setup-{}-x64.exe", payload.version);
    if payload.schema != 3
        || payload.channel != "stable"
        || payload.filename != filename
        || payload.size == 0
        || payload.size > 1024 * 1024 * 1024
        || !hex(&payload.sha256, 64)
        || !timestamp(&payload.published_at)
        || payload.download_url
            != format!(
                "https://github.com/cortezdemartino/palisade-windows/releases/download/v{}/{filename}",
                payload.version
            )
    {
        return Err("manifest-identity");
    }
    decode::<64>(&payload.sha512)?;
    Ok(())
}

fn verify_with(
    bytes: &[u8],
    root: &str,
    installed: &[Delegation],
    installed_version: &str,
) -> Result<VerifiedRelease> {
    if bytes.len() > MAX_ENVELOPE {
        return Err("manifest-limit");
    }
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| "manifest-format")?;
    validate(&envelope.payload)?;
    if stable_version(&envelope.payload.version)? <= stable_version(installed_version)? {
        return Err("version-rollback");
    }
    trust(root, installed, &envelope.payload.version)?;
    if envelope.transitions.len() < installed.len() {
        return Err("transition-rollback");
    }
    for (required, supplied) in installed.iter().zip(&envelope.transitions) {
        if normalise_transition(&required.payload)? != normalise_transition(&supplied.payload)? {
            return Err("transition-replacement");
        }
    }
    let key = trust(root, &envelope.transitions, &envelope.payload.version)?;
    authenticate(
        &key,
        &signed("PALISADE-DESKTOP-RELEASE-V3", &envelope.payload)?,
        &envelope.signature,
    )?;
    Ok(VerifiedRelease {
        payload: envelope.payload,
    })
}

/// Trust anchors, transition floor and current version are compiled into this binary.
pub fn verify_packaged(bytes: &[u8]) -> Result<VerifiedRelease> {
    let history: Vec<Delegation> =
        serde_json::from_str(PACKAGED_TRANSITIONS).map_err(|_| "packaged-trust")?;
    verify_with(bytes, ROOT_PEM, &history, &packaged_version()?)
}

/// Caller must retain its deny-write/deny-delete native handle until installer completion.
pub fn verify_installer(file: &mut (impl Read + Seek), release: &VerifiedRelease) -> Result<()> {
    let payload = release.payload();
    validate(payload)?;
    file.seek(SeekFrom::Start(0))
        .map_err(|_| "installer-read")?;
    let mut sha256 = Sha256::new();
    let mut sha512 = Sha512::new();
    let mut size = 0u64;
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|_| "installer-read")?;
        if count == 0 {
            break;
        }
        size = size.checked_add(count as u64).ok_or("installer-size")?;
        if size > payload.size {
            return Err("installer-size");
        }
        sha256.update(&buffer[..count]);
        sha512.update(&buffer[..count]);
    }
    if size != payload.size
        || to_hex(sha256.finalize().as_ref()) != payload.sha256
        || STANDARD.encode(sha512.finalize()) != payload.sha512
    {
        return Err("installer-integrity");
    }
    Ok(())
}

pub fn stage_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            14 => b == b'4',
            19 => b"89ab".contains(&b),
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}

#[cfg(test)]
mod tests;
