//! Frozen native extension operations. Separate negotiation adds no authority.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize, Serialize)]
#[serde(tag = "op", deny_unknown_fields)]
pub enum Operation {
    #[serde(rename = "ext.hello")]
    Hello { versions: Vec<u64> },
    #[serde(rename = "resource.send-text", rename_all = "camelCase")]
    SendText { session: String, grant_id: String },
    #[serde(rename = "resource.offer-file", rename_all = "camelCase")]
    OfferFile { session: String, grant_id: String },
    #[serde(rename = "resource.send-image", rename_all = "camelCase")]
    SendImage { session: String, grant_id: String },
    #[serde(rename = "resource.offer-files", rename_all = "camelCase")]
    OfferFiles { session: String, grant_id: String },
    #[serde(rename = "resource.drop-surface", rename_all = "camelCase")]
    DropSurface { session: String, grant_id: String },
    #[serde(rename = "resource.resume")]
    Resume { session: String, transfer: String },
    #[serde(rename = "resource.sync", rename_all = "camelCase")]
    Sync {
        session: String,
        grant_id: String,
        state: SyncState,
    },
    #[serde(rename = "resource.cancel")]
    Cancel { session: String, transfer: String },
}
#[derive(Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SyncState {
    On,
    Off,
}
impl Operation {
    pub fn safety(&self) -> bool {
        matches!(
            self,
            Self::Cancel { .. }
                | Self::Sync {
                    state: SyncState::Off,
                    ..
                }
        )
    }
    pub fn validate(&self, version: u8) -> Result<(), Error> {
        if !(1..=3).contains(&version) {
            return Err(Error::UnsupportedVersion);
        }
        let token = |v: &str| crate::grant::token(v, 8, 128);
        match self {
            Self::Hello { versions } => {
                if version != 1
                    || versions.is_empty()
                    || versions.len() > 4
                    || versions
                        .iter()
                        .any(|v| *v == 0 || *v > 9_007_199_254_740_991)
                {
                    return Err(Error::InvalidMessage);
                }
            }
            Self::SendText { session, grant_id } | Self::OfferFile { session, grant_id } => {
                if !token(session) || !token(grant_id) {
                    return Err(Error::InvalidMessage);
                }
            }
            Self::Cancel { session, transfer } => {
                if !token(session) || !token(transfer) {
                    return Err(Error::InvalidMessage);
                }
            }
            Self::SendImage { session, grant_id }
            | Self::OfferFiles { session, grant_id }
            | Self::DropSurface { session, grant_id }
            | Self::Sync {
                session, grant_id, ..
            } => {
                if version < 2 {
                    return Err(Error::InvalidMessage);
                }
                if !token(session) || !token(grant_id) {
                    return Err(Error::InvalidMessage);
                }
            }
            Self::Resume { session, transfer } => {
                if version < 2 {
                    return Err(Error::InvalidMessage);
                }
                if !token(session) || !token(transfer) {
                    return Err(Error::InvalidMessage);
                }
            }
        }
        Ok(())
    }
}
#[derive(Deserialize, Serialize)]
#[serde(tag = "outcome", deny_unknown_fields, rename_all = "lowercase")]
pub enum Answer {
    Ok { result: Value },
    Error { code: Error },
}
impl Answer {
    pub fn from_result(result: Result<Value, Error>) -> Self {
        match result {
            Ok(result) => Self::Ok { result },
            Err(code) => Self::Error { code },
        }
    }
}
#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Error {
    UnsupportedVersion,
    InvalidMessage,
    Busy,
    ConsentDenied,
    GrantInvalid,
    NotActive,
    Unsupported,
    TooLarge,
    Cancelled,
    StaleOperation,
    Timeout,
    Stopped,
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_extension_requests_retain_their_typed_native_fields_and_versions() {
        for fixture in [
            include_str!("../../../tests/fixtures/assist-native-ext-v1.json"),
            include_str!("../../../tests/fixtures/assist-native-ext-v2.json"),
        ] {
            let fixture: Value = serde_json::from_str(fixture).unwrap();
            for request in fixture["requests"]["valid"].as_array().unwrap() {
                let mut request = request.clone();
                let object = request.as_object_mut().unwrap();
                object.remove("p");
                object.remove("id");
                let version = object.remove("v").unwrap().as_u64().unwrap() as u8;
                let op: Operation = serde_json::from_value(request.clone()).unwrap();
                op.validate(version).unwrap();
                request["approved"] = true.into();
                assert!(serde_json::from_value::<Operation>(request).is_err());
            }
        }
    }
    #[test]
    fn closed_extension_operations_reject_approval_paths_unknown_fields_and_v2_under_v1() {
        for text in [
            r#"{"op":"resource.send-text","session":"session_01","grantId":"grant_0001","approved":true}"#,
            r#"{"op":"resource.offer-file","session":"session_01","grantId":"grant_0001","path":"C:\\private"}"#,
            r#"{"op":"resource.sync","session":"session_01","grantId":"grant_0001","state":"always"}"#,
            r#"{"op":"execute","command":"anything"}"#,
        ] {
            assert!(serde_json::from_str::<Operation>(text).is_err());
        }
        let op: Operation = serde_json::from_str(
            r#"{"op":"resource.sync","session":"session_01","grantId":"grant_0001","state":"off"}"#,
        )
        .unwrap();
        assert!(op.safety());
        assert!(op.validate(1).is_err());
        assert!(op.validate(2).is_ok());
        let op = Operation::Hello {
            versions: vec![9_007_199_254_740_991, 3, 2, 1],
        };
        assert!(op.validate(1).is_ok());
        assert!(op.validate(3).is_err());
    }
}
