//! Qualified issuer records. A fetched key or legacy first-use record is never
//! a production trust anchor. The native UI/storage layer owns confirmation.
use crate::grant::decode;
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub const ORIGIN: &str = "https://palisade.onyxops.org";
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Issuer {
    protocol: String,
    version: u32,
    public_key: String,
    fingerprint: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    key: [u8; 32],
    fingerprint: String,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    Release,
    Confirmed,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Record {
    version: u32,
    origin: String,
    public_key: String,
    fingerprint: String,
    method: Method,
}
fn key_fingerprint(key: &[u8; 32]) -> String {
    Sha256::digest(key)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
impl Candidate {
    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() > 2048 {
            return Err("invalid-issuer");
        }
        let issuer: Issuer = serde_json::from_slice(bytes).map_err(|_| "invalid-issuer")?;
        let key = decode::<32>(&issuer.public_key)?;
        if issuer.protocol != "palisade.assist.control"
            || issuer.version != 1
            || key_fingerprint(&key) != issuer.fingerprint
            || VerifyingKey::from_bytes(&key)
                .map_err(|_| "invalid-issuer")?
                .is_weak()
        {
            return Err("invalid-issuer");
        }
        Ok(Self {
            key,
            fingerprint: issuer.fingerprint,
        })
    }
    pub fn key(&self) -> [u8; 32] {
        self.key
    }
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    pub(crate) fn record(&self, method: Method) -> Record {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        Record {
            version: 1,
            origin: ORIGIN.into(),
            public_key: URL_SAFE_NO_PAD.encode(self.key),
            fingerprint: self.fingerprint.clone(),
            method,
        }
    }
    pub fn qualify(
        &self,
        stored: Option<&Record>,
        release_pin: Option<&str>,
    ) -> Result<Method, &'static str> {
        if let Some(record) = stored {
            if record.version != 1
                || record.origin != ORIGIN
                || decode::<32>(&record.public_key)? != self.key
                || record.fingerprint != self.fingerprint
            {
                return Err("issuer-changed");
            }
            if record.method == Method::Release && release_pin != Some(self.fingerprint.as_str()) {
                return Err("issuer-unqualified");
            }
            return Ok(record.method);
        }
        if release_pin == Some(self.fingerprint.as_str()) {
            Ok(Method::Release)
        } else {
            Err("issuer-unqualified")
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn candidate() -> Candidate {
        let f: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/assist-native-v1.json"
        ))
        .unwrap();
        Candidate::parse(&serde_json::to_vec(&serde_json::json!({"protocol":"palisade.assist.control","version":1,"publicKey":f["issuerPublicKey"],"fingerprint":f["issuerFingerprint"]})).unwrap()).unwrap()
    }
    #[test]
    fn https_first_use_and_legacy_formats_cannot_qualify_trust() {
        let key = candidate();
        assert_eq!(key.qualify(None, None), Err("issuer-unqualified"));
        assert_eq!(
            key.qualify(None, Some(&"a".repeat(64))),
            Err("issuer-unqualified")
        );
        for method in ["tofu", "first-use", "https", "none"] {
            let mut record = serde_json::to_value(key.record(Method::Confirmed)).unwrap();
            record["method"] = method.into();
            assert!(serde_json::from_value::<Record>(record).is_err());
        }
        assert!(serde_json::from_slice::<Record>(&key.key).is_err());
    }
    #[test]
    fn qualified_records_bind_origin_key_fingerprint_and_release_policy() {
        let key = candidate();
        assert_eq!(
            key.qualify(None, Some(key.fingerprint())),
            Ok(Method::Release)
        );
        assert_eq!(
            key.qualify(Some(&key.record(Method::Confirmed)), None),
            Ok(Method::Confirmed)
        );
        assert_eq!(
            key.qualify(Some(&key.record(Method::Release)), None),
            Err("issuer-unqualified")
        );
        for field in ["version", "origin", "publicKey", "fingerprint"] {
            let mut value = serde_json::to_value(key.record(Method::Confirmed)).unwrap();
            value[field] = match field {
                "version" => 2.into(),
                "publicKey" => "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into(),
                _ => "unexpected".into(),
            };
            let record: Record = serde_json::from_value(value).unwrap();
            assert!(
                key.qualify(Some(&record), Some(key.fingerprint())).is_err(),
                "{field}"
            );
        }
    }
    #[test]
    fn issuer_document_rejects_extra_fields_bad_fingerprint_and_weak_keys() {
        let key = candidate();
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        let base = serde_json::json!({"protocol":"palisade.assist.control","version":1,"publicKey":URL_SAFE_NO_PAD.encode(key.key),"fingerprint":key.fingerprint});
        for change in [
            serde_json::json!({"approved":true}),
            serde_json::json!({"version":2}),
            serde_json::json!({"fingerprint":"f".repeat(64)}),
            serde_json::json!({"publicKey":URL_SAFE_NO_PAD.encode([0;32]),"fingerprint":key_fingerprint(&[0;32])}),
        ] {
            let mut value = base.clone();
            value
                .as_object_mut()
                .unwrap()
                .extend(change.as_object().unwrap().clone());
            assert!(Candidate::parse(&serde_json::to_vec(&value).unwrap()).is_err());
        }
    }
}
