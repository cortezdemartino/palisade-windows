//! Closed, purpose-limited operations on the private application channel.
//! Web envelopes remain validated by the existing canonical main-process gate.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize, Serialize)]
#[serde(tag = "op", deny_unknown_fields)]
pub enum Operation {
    #[serde(rename = "hello")]
    Hello { versions: Vec<u32> },
    #[serde(rename = "enroll.key")]
    EnrollKey { origin: String },
    #[serde(rename = "enroll.prove", rename_all = "camelCase")]
    EnrollProve {
        endpoint_id: String,
        challenge: String,
        account: String,
        device: String,
    },
    #[serde(rename = "grant.offer")]
    GrantOffer { session: String, grant: String },
    #[serde(rename = "peer.offer", rename_all = "camelCase")]
    PeerOffer {
        session: String,
        grant_id: String,
        sdp: String,
    },
    #[serde(rename = "peer.candidate")]
    PeerCandidate {
        session: String,
        candidate: String,
        mid: Option<String>,
    },
    #[serde(rename = "peer.close")]
    PeerClose { session: String },
    #[serde(rename = "capture.start")]
    CaptureStart { operation: String, room: String },
    #[serde(rename = "capture.bind", rename_all = "camelCase")]
    CaptureBind {
        operation: String,
        track_sid: String,
    },
    #[serde(rename = "capture.cancel")]
    CaptureCancel { operation: String },
    #[serde(rename = "capture.stop")]
    CaptureStop { operation: String },
    #[serde(rename = "status")]
    Status {},
    // Stop is channel-terminal and travels on the independent shutdown path.
}
#[derive(Deserialize, Serialize)]
#[serde(tag = "outcome", deny_unknown_fields, rename_all = "lowercase")]
pub enum Answer {
    Ok { result: Value },
    Error { code: Error },
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Error {
    UnsupportedVersion,
    InvalidMessage,
    Busy,
    NotEnrolled,
    IssuerMismatch,
    ConsentDenied,
    GrantInvalid,
    BindingMismatch,
    TransportMismatch,
    CaptureUnavailable,
    Cancelled,
    StaleOperation,
    Timeout,
    Stopped,
    Internal,
}
impl Answer {
    pub fn ok(result: Value) -> Self {
        Self::Ok { result }
    }
    pub fn error(code: Error) -> Self {
        Self::Error { code }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_native_operations_keep_their_exact_fields_on_private_dispatch() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/assist-native-v1.json"
        ))
        .unwrap();
        for value in fixture["requests"]["valid"].as_array().unwrap() {
            let mut value = value.clone();
            let object = value.as_object_mut().unwrap();
            object.remove("p");
            object.remove("v");
            object.remove("id");
            if object["op"] == "stop" {
                continue;
            }
            assert!(
                serde_json::from_value::<Operation>(value.clone()).is_ok(),
                "{value}"
            );
            object_insert_extra(&mut value);
            assert!(serde_json::from_value::<Operation>(value).is_err());
        }
    }
    fn object_insert_extra(value: &mut Value) {
        value
            .as_object_mut()
            .unwrap()
            .insert("approved".into(), true.into());
    }
    #[test]
    fn private_operations_reject_commands_approval_flags_and_arbitrary_signing() {
        for text in [
            r#"{"op":"sign","data":"hello"}"#,
            r#"{"op":"enroll.prove","endpointId":"endpoint","challenge":"c","account":"a","device":"d","approved":true}"#,
            r#"{"op":"hello","versions":[1],"command":"exec"}"#,
            r#"{"op":"stop"}"#,
        ] {
            assert!(serde_json::from_str::<Operation>(text).is_err());
        }
        assert!(serde_json::from_str::<Operation>(r#"{"op":"hello","versions":[1]}"#).is_ok());
    }
    #[test]
    fn private_failure_vocabulary_cannot_carry_paths_or_native_diagnostics() {
        assert!(
            serde_json::from_str::<Answer>(r#"{"outcome":"error","code":"C:\\private"}"#).is_err()
        );
        assert!(
            serde_json::from_str::<Answer>(
                r#"{"outcome":"error","code":"internal","details":"secret"}"#
            )
            .is_err()
        );
    }
}
