//! Trusted-main peer operations. Never accepted by the page-facing bridge.
use serde::{Deserialize, Serialize};
#[derive(Deserialize, Serialize)]
#[serde(tag = "action", deny_unknown_fields, rename_all = "kebab-case")]
pub enum Operation {
    #[serde(rename_all = "camelCase")]
    Prepare {
        session: String,
        grant_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Seal {
        session: String,
        attempt: String,
        own_fingerprint: String,
    },
    Observed {
        session: String,
        attempt: String,
        fingerprint: String,
    },
    Check {
        session: String,
        attempt: String,
    },
    #[serde(rename_all = "camelCase")]
    Packet {
        session: String,
        attempt: String,
        text: String,
        ingress_ns: String,
    },
}
impl Operation {
    pub fn session(&self) -> &str {
        match self {
            Self::Prepare { session, .. }
            | Self::Seal { session, .. }
            | Self::Observed { session, .. }
            | Self::Check { session, .. }
            | Self::Packet { session, .. } => session,
        }
    }
}
