use super::*;
use ed25519_dalek::{Signer, SigningKey};
use std::io::Cursor;

#[test]
fn native_verification_matches_the_actual_javascript_signing_format() {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/node-vector.cjs");
    let result = std::process::Command::new("node")
        .arg(script)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let vector: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(vector["uninstallGuid"].as_str(), Some(UNINSTALL_GUID));
    let history: Vec<Delegation> = serde_json::from_value(vector["history"].clone()).unwrap();
    let bytes = serde_json::to_vec(&vector["release"]).unwrap();
    let verified =
        verify_with(&bytes, vector["root"].as_str().unwrap(), &history, "0.3.3").unwrap();
    assert!(verify_installer(&mut Cursor::new(b"x"), &verified).is_ok());
    assert!(verify_packaged(&bytes).is_err());
}

// Fresh disposable test keys, never production credentials or persisted key files.
fn key() -> SigningKey {
    let mut seed = [0; 32];
    #[cfg(windows)]
    assert_eq!(
        unsafe {
            windows_sys::Win32::Security::Cryptography::BCryptGenRandom(
                std::ptr::null_mut(),
                seed.as_mut_ptr(),
                32,
                windows_sys::Win32::Security::Cryptography::BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        },
        0
    );
    #[cfg(not(windows))]
    std::fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut seed)
        .unwrap();
    SigningKey::from_bytes(&seed)
}
fn pem(key: &SigningKey) -> String {
    let mut der = SPKI_PREFIX.to_vec();
    der.extend(key.verifying_key().as_bytes());
    format!(
        "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----\n",
        STANDARD.encode(der)
    )
}
fn payload() -> Payload {
    let data = b"disposable installer";
    Payload { schema: 3, channel: "stable".into(), version: "0.3.4".into(),
        filename: "Palisade-Setup-0.3.4-x64.exe".into(), size: data.len() as u64,
        sha256: to_hex(Sha256::digest(data).as_ref()), sha512: STANDARD.encode(Sha512::digest(data)),
        published_at: "2026-11-10T00:00:00.000Z".into(),
        download_url: "https://github.com/cortezdemartino/palisade-windows/releases/download/v0.3.4/Palisade-Setup-0.3.4-x64.exe".into() }
}
fn envelope(payload: &Payload, key: &SigningKey, transitions: &[Delegation]) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({ "payload": payload, "transitions": transitions,
        "signature": STANDARD.encode(key.sign(&signed("PALISADE-DESKTOP-RELEASE-V3", payload).unwrap()).to_bytes()) })).unwrap()
}
fn delegation(root: &SigningKey, successor: &SigningKey, version: &str) -> Delegation {
    let payload = Transition {
        schema: 1,
        from: fingerprint(&root.verifying_key()),
        to_public_key: pem(successor),
        min_version: version.into(),
    };
    Delegation {
        signature: STANDARD.encode(
            root.sign(&signed("PALISADE-DESKTOP-KEY-TRANSITION-V1", &payload).unwrap())
                .to_bytes(),
        ),
        payload,
    }
}

#[test]
fn valid_signature_and_both_installer_hashes_are_required() {
    let key = key();
    let payload = payload();
    let bytes = envelope(&payload, &key, &[]);
    let verified = verify_with(&bytes, &pem(&key), &[], "0.3.3").unwrap();
    assert_eq!(verified.payload(), &payload);
    assert!(
        verify_packaged(&bytes).is_err(),
        "test authority cannot override compiled trust"
    );
    assert!(verify_installer(&mut Cursor::new(b"disposable installer"), &verified).is_ok());
    for data in [
        b"disposable installeX".as_slice(),
        b"short",
        b"disposable installer extra",
    ] {
        assert!(verify_installer(&mut Cursor::new(data), &verified).is_err());
    }
    let mut changed = payload.clone();
    changed.sha256 = "0".repeat(64);
    let authorized = verify_with(&envelope(&changed, &key, &[]), &pem(&key), &[], "0.3.3").unwrap();
    assert!(verify_installer(&mut Cursor::new(b"disposable installer"), &authorized).is_err());
    changed = payload.clone();
    changed.sha512 = STANDARD.encode([0; 64]);
    let authorized = verify_with(&envelope(&changed, &key, &[]), &pem(&key), &[], "0.3.3").unwrap();
    assert!(verify_installer(&mut Cursor::new(b"disposable installer"), &authorized).is_err());
}

#[test]
fn installed_version_is_an_immutable_strict_floor() {
    let key = key();
    let bytes = envelope(&payload(), &key, &[]);
    for current in ["0.3.4", "0.3.5", "0.10.0", "1.0.0"] {
        assert_eq!(
            verify_with(&bytes, &pem(&key), &[], current),
            Err("version-rollback")
        );
    }
    for malformed in [
        "0.3.4-rc.1",
        "00.3.4",
        "1.2",
        "1.2.3.4",
        "1.2.3+tag",
        "1.2.9007199254740992",
        "1.2.-1",
    ] {
        assert!(stable_version(malformed).is_err());
    }
    assert!(stable_version("9007199254740991.0.0").is_ok());
}

#[test]
fn unsigned_tampered_foreign_and_duplicate_manifests_are_rejected() {
    let key = key();
    let bytes = envelope(&payload(), &key, &[]);
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["payload"]["sha256"] = "0".repeat(64).into();
    assert!(
        verify_with(
            &serde_json::to_vec(&value).unwrap(),
            &pem(&key),
            &[],
            "0.3.3"
        )
        .is_err()
    );
    value = serde_json::from_slice(&bytes).unwrap();
    value["signature"] = "".into();
    assert!(
        verify_with(
            &serde_json::to_vec(&value).unwrap(),
            &pem(&key),
            &[],
            "0.3.3"
        )
        .is_err()
    );
    assert!(verify_with(&bytes, &pem(&super::tests::key()), &[], "0.3.3").is_err());
    value = serde_json::from_slice(&bytes).unwrap();
    value["extra"] = true.into();
    assert!(
        verify_with(
            &serde_json::to_vec(&value).unwrap(),
            &pem(&key),
            &[],
            "0.3.3"
        )
        .is_err()
    );
    let duplicate =
        String::from_utf8(bytes)
            .unwrap()
            .replacen("\"schema\":3", "\"schema\":3,\"schema\":3", 1);
    assert!(verify_with(duplicate.as_bytes(), &pem(&key), &[], "0.3.3").is_err());
    assert!(verify_with(&vec![0; MAX_ENVELOPE + 1], &pem(&key), &[], "0.3.3").is_err());
}

#[test]
fn identity_channel_location_size_and_date_are_exact() {
    let key = key();
    let base = payload();
    for field in [
        "schema",
        "channel",
        "version",
        "filename",
        "size",
        "publishedAt",
        "downloadUrl",
    ] {
        let mut value = serde_json::to_value(&base).unwrap();
        value[field] = match field {
            "schema" => 2.into(),
            "size" => 0.into(),
            "channel" => "beta".into(),
            "version" => "0.3.4-rc.1".into(),
            "filename" => "../evil.exe".into(),
            "publishedAt" => "2026-02-30T00:00:00.000Z".into(),
            _ => "https://evil.example/installer.exe".into(),
        };
        let changed: Payload = serde_json::from_value(value).unwrap();
        assert!(verify_with(&envelope(&changed, &key, &[]), &pem(&key), &[], "0.3.3").is_err());
    }
    for date in [
        "2026-02-29T00:00:00.000Z",
        "2026-11-10T24:00:00.000Z",
        "2026-11-10T00:60:00.000Z",
        "2026-11-10T00:00:60.000Z",
        "now",
    ] {
        assert!(!timestamp(date));
    }
    assert!(timestamp("2024-02-29T23:59:59.999Z"));
    assert!(timestamp("0000-01-01T00:00:00.000Z"));
}

#[test]
fn rotation_requires_predecessor_prefix_activation_and_fresh_authority() {
    let root = key();
    let successor = key();
    let foreign = key();
    let transition = delegation(&root, &successor, "0.3.4");
    let history = vec![transition.clone()];
    let bytes = envelope(&payload(), &successor, &history);
    assert!(verify_with(&bytes, &pem(&root), &history, "0.3.3").is_ok());
    assert!(
        verify_with(
            &envelope(&payload(), &root, &[]),
            &pem(&root),
            &history,
            "0.3.3"
        )
        .is_err()
    );
    assert!(
        verify_with(
            &envelope(&payload(), &root, &history),
            &pem(&root),
            &history,
            "0.3.3"
        )
        .is_err()
    );
    let alternative = vec![delegation(&root, &foreign, "0.3.4")];
    assert!(
        verify_with(
            &envelope(&payload(), &foreign, &alternative),
            &pem(&root),
            &history,
            "0.3.3"
        )
        .is_err()
    );
    let too_early = vec![delegation(&root, &successor, "0.3.5")];
    assert!(
        verify_with(
            &envelope(&payload(), &successor, &too_early),
            &pem(&root),
            &[],
            "0.3.3"
        )
        .is_err()
    );
    let mut modified = history.clone();
    modified[0].payload.min_version = "0.3.3".into();
    assert!(
        verify_with(
            &envelope(&payload(), &successor, &modified),
            &pem(&root),
            &[],
            "0.3.3"
        )
        .is_err()
    );
    let cyclic = vec![transition, delegation(&successor, &root, "0.3.4")];
    assert!(
        verify_with(
            &envelope(&payload(), &root, &cyclic),
            &pem(&root),
            &[],
            "0.3.3"
        )
        .is_err()
    );
}

#[test]
fn stage_names_cannot_address_arbitrary_paths() {
    assert!(stage_id("77e14877-832f-44bc-9e92-b27eb6ba77ad"));
    for value in [
        "..",
        "C:\\Temp",
        "77E14877-832f-44bc-9e92-b27eb6ba77ad",
        "77e14877-832f-54bc-9e92-b27eb6ba77ad",
        "77e14877-832f-44bc-1e92-b27eb6ba77ad",
        "77e14877-832f-44bc-9e92-b27eb6ba77ad/../x",
    ] {
        assert!(!stage_id(value));
    }
    assert!(packaged_version().is_ok());
}
