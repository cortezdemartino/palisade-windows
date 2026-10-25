use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

const SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const CAPABILITIES: [&str; 8] = [
    "pointer",
    "annotate",
    "control_mouse",
    "control_keyboard",
    "clipboard_to_host",
    "clipboard_to_guest",
    "file_to_host",
    "file_to_guest",
];

// Field order is the canonical v1 sorted-key order. Unknown/duplicate fields fail.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Grant {
    pub capability: String,
    #[serde(deserialize_with = "required_endpoint")]
    pub endpoint: Option<String>,
    pub expires_at: u64,
    pub generation: u32,
    pub grant_id: String,
    pub guest_account: String,
    pub guest_device: String,
    pub guest_transport: String,
    pub host_account: String,
    pub host_device: String,
    pub issued_at: u64,
    pub issuer: String,
    pub nonce: String,
    pub protocol: String,
    pub session: String,
    pub version: u32,
}

fn required_endpoint<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedGrant {
    pub grant: Grant,
    pub signature: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Expected {
    pub session: String,
    pub host_account: String,
    pub host_device: String,
    pub guest_account: String,
    pub guest_device: String,
    pub guest_transport: String,
    pub endpoint: Option<String>,
    pub session_deadline: u64,
    pub minimum_generation: u32,
}

fn token(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.len())
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn mxid(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('@') else {
        return false;
    };
    let Some((local, server)) = rest.split_once(':') else {
        return false;
    };
    [local, server]
        .iter()
        .all(|v| (1..=255).contains(&v.encode_utf16().count()) && !v.chars().any(js_whitespace))
}
fn js_whitespace(c: char) -> bool {
    matches!(c, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' |
        '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
pub fn fingerprint(s: &str) -> bool {
    s.strip_prefix("sha-256 ").is_some_and(|v| {
        v.len() == 95
            && v.bytes().enumerate().all(|(i, c)| {
                if i % 3 == 2 {
                    c == b':'
                } else {
                    c.is_ascii_digit() || (b'A'..=b'F').contains(&c)
                }
            })
    })
}
pub fn decode<const N: usize>(text: &str) -> Result<[u8; N], &'static str> {
    let bytes = URL_SAFE_NO_PAD
        .decode(text)
        .map_err(|_| "invalid-encoding")?;
    if URL_SAFE_NO_PAD.encode(&bytes) != text {
        return Err("invalid-encoding");
    }
    bytes.try_into().map_err(|_| "invalid-encoding")
}
impl Grant {
    pub fn canonical(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("bounded grant serialization")
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        let native = self.capability == "control_mouse" || self.capability == "control_keyboard";
        if self.issuer != "palisade"
            || self.protocol != "palisade.assist.control"
            || self.version != 1
        {
            return Err("incompatible-protocol");
        }
        if !CAPABILITIES.contains(&self.capability.as_str())
            || !token(&self.grant_id, 8, 128)
            || !token(&self.session, 8, 128)
            || !token(&self.nonce, 16, 128)
            || !token(&self.host_device, 1, 64)
            || !token(&self.guest_device, 1, 64)
            || !mxid(&self.host_account)
            || !mxid(&self.guest_account)
            || self.host_account == self.guest_account
            || !fingerprint(&self.guest_transport)
            || !(1..=1_000_000).contains(&self.generation)
            || self.issued_at > SAFE_INTEGER
            || self.expires_at > SAFE_INTEGER
            || self.expires_at <= self.issued_at
            || self.expires_at - self.issued_at > 900_000
            || native != self.endpoint.is_some()
            || self.endpoint.as_ref().is_some_and(|v| !token(v, 8, 128))
        {
            return Err("invalid-message");
        }
        Ok(())
    }
}

// This authenticates issuance only. It never establishes consent or current liveness.
pub fn verify(
    signed: &SignedGrant,
    issuer: &[u8; 32],
    expected: &Expected,
    now: u64,
) -> Result<(), &'static str> {
    let grant = &signed.grant;
    grant.validate()?;
    let key = VerifyingKey::from_bytes(issuer).map_err(|_| "untrusted-issuer")?;
    let signature = Signature::from_bytes(&decode::<64>(&signed.signature)?);
    key.verify_strict(&grant.canonical(), &signature)
        .map_err(|_| "binding-mismatch")?;
    if grant.session != expected.session
        || grant.host_account != expected.host_account
        || grant.host_device != expected.host_device
        || grant.guest_account != expected.guest_account
        || grant.guest_device != expected.guest_device
        || grant.guest_transport != expected.guest_transport
        || grant.endpoint != expected.endpoint
        || grant.expires_at > expected.session_deadline
        || expected.session_deadline > SAFE_INTEGER
        || now > SAFE_INTEGER
    {
        return Err("binding-mismatch");
    }
    if grant.expires_at <= now || grant.issued_at > now.saturating_add(30_000) {
        return Err("expired");
    }
    if grant.generation < expected.minimum_generation {
        return Err("stale-generation");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixtures() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../../tests/fixtures/assist-control-v1.json"
        ))
        .unwrap()
    }
    fn expected(f: &serde_json::Value, signed: &SignedGrant) -> Expected {
        let e = &f["expect"];
        Expected {
            session: e["session"].as_str().unwrap().into(),
            host_account: e["hostAccount"].as_str().unwrap().into(),
            host_device: e["hostDevice"].as_str().unwrap().into(),
            guest_account: e["guestAccount"].as_str().unwrap().into(),
            guest_device: e["guestDevice"].as_str().unwrap().into(),
            guest_transport: e["guestTransport"].as_str().unwrap().into(),
            endpoint: signed.grant.endpoint.clone(),
            session_deadline: e["sessionDeadline"].as_u64().unwrap(),
            minimum_generation: 1,
        }
    }
    #[test]
    fn real_companion_signatures_and_all_negative_fixtures() {
        let f = fixtures();
        let key = decode::<32>(f["issuerPublicKey"].as_str().unwrap()).unwrap();
        let now = f["expect"]["now"].as_u64().unwrap();
        for entry in f["grants"]["valid"].as_array().unwrap() {
            let signed: SignedGrant = serde_json::from_value(entry["signed"].clone()).unwrap();
            verify(&signed, &key, &expected(&f, &signed), now).unwrap();
        }
        for entry in f["grants"]["invalid"].as_array().unwrap() {
            let parsed = serde_json::from_value::<SignedGrant>(entry["signed"].clone());
            if let Ok(signed) = parsed {
                let mut context = expected(&f, &signed);
                if let Some(v) = entry["expect"].get("minimumGeneration") {
                    context.minimum_generation = v.as_u64().unwrap() as u32
                }
                let result = verify(&signed, &key, &context, now);
                assert!(
                    result.is_err(),
                    "accepted negative fixture: {}",
                    entry["name"]
                );
            }
        }
    }
    #[test]
    fn endpoint_pin_and_context_are_independent_of_signature() {
        let f = fixtures();
        let signed: SignedGrant =
            serde_json::from_value(f["grants"]["valid"][1]["signed"].clone()).unwrap();
        let key = decode::<32>(f["issuerPublicKey"].as_str().unwrap()).unwrap();
        let mut e = expected(&f, &signed);
        let now = f["expect"]["now"].as_u64().unwrap();
        assert!(verify(&signed, &[7; 32], &e, now).is_err());
        e.endpoint = Some("wrong_endpoint_00".into());
        assert!(verify(&signed, &key, &e, now).is_err());
        e = expected(&f, &signed);
        e.minimum_generation = 2;
        assert!(verify(&signed, &key, &e, now).is_err());
        e = expected(&f, &signed);
        e.session_deadline = now;
        assert!(verify(&signed, &key, &e, now).is_err());
    }
    #[test]
    fn duplicate_unknown_missing_fields_and_two_hour_v1_fail() {
        let f = fixtures();
        let text = f["grants"]["valid"][1]["signed"].to_string();
        let duplicate = text.replacen("\"version\":1", "\"version\":1,\"version\":1", 1);
        assert!(serde_json::from_str::<SignedGrant>(&duplicate).is_err());
        let mut missing = f["grants"]["valid"][1]["signed"].clone();
        missing["grant"].as_object_mut().unwrap().remove("endpoint");
        assert!(serde_json::from_value::<SignedGrant>(missing).is_err());
        let mut value = f["grants"]["valid"][1]["signed"].clone();
        value["grant"]["stream"] = "stream_unversioned".into();
        assert!(serde_json::from_value::<SignedGrant>(value).is_err());
        let mut value = f["grants"]["valid"][1]["signed"].clone();
        value["grant"]["expiresAt"] = (f["now"].as_u64().unwrap() + 7_200_000).into();
        let signed: SignedGrant = serde_json::from_value(value).unwrap();
        assert!(signed.grant.validate().is_err());
    }
    #[test]
    fn canonical_mxid_whitespace_uses_ecmascript_semantics() {
        assert!(mxid("@user\u{0085}:palisade.test"));
        for character in ['\u{feff}', '\u{00a0}', '\u{2028}', '\n', ' '] {
            assert!(!mxid(&format!("@user{character}:palisade.test")));
        }
    }
}
