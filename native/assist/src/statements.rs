//! Exact docs/49 endpoint statements. Encoding is not authorization; only native
//! enrollment, consent and owned-peer coordinators may request their signatures.
use crate::{
    grant::{decode, fingerprint, mxid, token},
    stream::StreamGrant,
};
fn id(value: &str) -> bool {
    token(value, 8, 128)
}
pub fn enrollment(
    issuer: &str,
    endpoint: &str,
    account: &str,
    device: &str,
    challenge: &str,
) -> Result<Vec<u8>, &'static str> {
    if issuer.len() != 64
        || !issuer
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        || !id(endpoint)
        || !mxid(account)
        || !token(device, 1, 64)
        || decode::<32>(challenge).is_err()
    {
        return Err("invalid-statement");
    }
    Ok(format!("palisade.assist.control/endpoint-proof/v2\n{issuer}\n{endpoint}\n{account}\n{device}\n{challenge}").into_bytes())
}
pub fn acknowledgement(grant: &StreamGrant) -> Result<Vec<u8>, &'static str> {
    grant.validate()?;
    let mut bytes = b"palisade.assist.control/endpoint-ack/v2\n".to_vec();
    bytes.extend(grant.canonical());
    Ok(bytes)
}
pub fn lease(endpoint: &str, session: &str, issued_at: u64) -> Result<Vec<u8>, &'static str> {
    if !id(endpoint) || !id(session) || issued_at > 8_640_000_000_000_000 {
        return Err("invalid-statement");
    }
    Ok(
        format!("palisade.assist.control/endpoint-lease/v2\n{endpoint}\n{session}\n{issued_at}")
            .into_bytes(),
    )
}
pub fn transport(
    session: &str,
    endpoint: &str,
    own_fingerprint: &str,
    helper_transport: &str,
) -> Result<Vec<u8>, &'static str> {
    if !id(session)
        || !id(endpoint)
        || !fingerprint(own_fingerprint)
        || !fingerprint(helper_transport)
    {
        return Err("invalid-statement");
    }
    Ok(format!("palisade.assist.control/endpoint-transport/v2\n{session}\n{endpoint}\n{own_fingerprint}\n{helper_transport}").into_bytes())
}
#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature, VerifyingKey};
    use serde_json::Value;
    fn fixtures() -> Value {
        serde_json::from_str(include_str!(
            "../../../tests/fixtures/assist-native-v1.json"
        ))
        .unwrap()
    }
    #[test]
    fn all_labelled_statement_bytes_and_signatures_match_the_canonical_fixture() {
        let f = fixtures();
        let grant: StreamGrant = serde_json::from_value(f["nativeGrant"]["grant"].clone()).unwrap();
        let proof = &f["requests"]["valid"][2];
        let expected = [
            enrollment(
                f["issuerFingerprint"].as_str().unwrap(),
                proof["endpointId"].as_str().unwrap(),
                proof["account"].as_str().unwrap(),
                proof["device"].as_str().unwrap(),
                proof["challenge"].as_str().unwrap(),
            )
            .unwrap(),
            acknowledgement(&grant).unwrap(),
            lease(
                grant.endpoint.as_deref().unwrap(),
                &grant.session,
                1_800_000_300_000,
            )
            .unwrap(),
            transport(
                &grant.session,
                grant.endpoint.as_deref().unwrap(),
                f["endpointFingerprint"].as_str().unwrap(),
                &grant.helper_transport,
            )
            .unwrap(),
        ];
        let key = VerifyingKey::from_bytes(
            &decode::<32>(f["endpointPublicKey"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        for (index, case) in f["statements"].as_array().unwrap().iter().enumerate() {
            let encoded = base64::Engine::decode(
                &base64::engine::general_purpose::URL_SAFE_NO_PAD,
                case["statement"].as_str().unwrap(),
            )
            .unwrap();
            assert_eq!(expected[index], encoded, "{}", case["name"]);
            assert_eq!(expected[index], case["text"].as_str().unwrap().as_bytes());
            let signature =
                Signature::from_bytes(&decode::<64>(case["signature"].as_str().unwrap()).unwrap());
            key.verify_strict(&expected[index], &signature).unwrap();
            for (other, statement) in expected.iter().enumerate() {
                if other != index {
                    assert!(key.verify_strict(statement, &signature).is_err());
                }
            }
            let mut tampered = expected[index].clone();
            tampered.push(b'x');
            assert!(key.verify_strict(&tampered, &signature).is_err());
        }
    }
    #[test]
    fn malformed_or_delimiter_injecting_fields_never_produce_signable_statements() {
        let f = fixtures();
        let proof = &f["requests"]["valid"][2];
        let issuer = f["issuerFingerprint"].as_str().unwrap();
        let challenge = proof["challenge"].as_str().unwrap();
        for (endpoint, account, device) in [
            ("endpoint_ok\nother", "@a:palisade.test", "DEV"),
            ("endpoint_ok", "@a\nperson:palisade.test", "DEV"),
            ("endpoint_ok", "@a:palisade.test", "DEV\nother"),
        ] {
            assert!(enrollment(issuer, endpoint, account, device, challenge).is_err());
        }
        assert!(
            enrollment(
                "not-a-fingerprint",
                "endpoint_ok",
                "@a:palisade.test",
                "DEV",
                challenge
            )
            .is_err()
        );
        assert!(
            enrollment(
                issuer,
                "endpoint_ok",
                "@a:palisade.test",
                "DEV",
                "arbitrary bytes"
            )
            .is_err()
        );
        assert!(lease("endpoint_ok", "session_ok", u64::MAX).is_err());
        assert!(
            transport(
                "session_ok",
                "endpoint_ok",
                "page declared fingerprint",
                "other"
            )
            .is_err()
        );
    }
}
