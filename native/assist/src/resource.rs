//! Frozen stream-v2 clipboard text envelopes, not OS-resource authority.
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(tag = "action", deny_unknown_fields, rename_all = "kebab-case")]
pub enum TextOperation {
    #[serde(rename_all = "camelCase")]
    Send {
        version: u8,
        grant_id: String,
    },
    Receive {
        text: String,
    },
    VerifySend {
        text: String,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TextMessage {
    p: String,
    v: u8,
    pub session: String,
    pub grant: String,
    #[serde(rename = "gen")]
    pub generation: u64,
    pub seq: u64,
    kind: String,
    pub text: String,
}
impl TextMessage {
    pub(crate) fn parse(raw: &str) -> Result<Self, &'static str> {
        if raw.encode_utf16().count() > 16_384 || raw.len() > 65_536 {
            return Err("too-large");
        }
        let message: Self = serde_json::from_str(raw).map_err(|_| "invalid-message")?;
        message.validate()?;
        Ok(message)
    }
    pub(crate) fn outgoing(
        session: &str,
        grant: &str,
        generation: u64,
        sequence: u64,
        text: String,
    ) -> Result<String, &'static str> {
        let message = Self {
            p: "palisade.assist.control".into(),
            v: 2,
            session: session.into(),
            grant: grant.into(),
            generation,
            seq: sequence,
            kind: "clipboard-text".into(),
            text,
        };
        message.validate()?;
        let raw = serde_json::to_string(&message).map_err(|_| "invalid-message")?;
        if raw.encode_utf16().count() > 16_384 {
            return Err("too-large");
        }
        Ok(raw)
    }
    fn validate(&self) -> Result<(), &'static str> {
        if self.p != "palisade.assist.control"
            || self.v != 2
            || self.kind != "clipboard-text"
            || !crate::grant::token(&self.session, 8, 128)
            || !crate::grant::token(&self.grant, 8, 128)
            || !(1..=1_000_000).contains(&self.generation)
            || self.seq > 9_007_199_254_740_991
        {
            return Err("invalid-message");
        }
        if self.text.is_empty() || self.text.contains('\0') || self.text.len() > 65_536 {
            return Err("too-large");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> serde_json::Value {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/assist-control-v2.json"
        ))
        .unwrap();
        fixture["messages"]["valid"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["kind"] == "clipboard-text")
            .unwrap()
            .clone()
    }
    #[test]
    fn frozen_text_fixture_and_full_safe_sequence_range_round_trip() {
        let value = fixture();
        let message = TextMessage::parse(&value.to_string()).unwrap();
        for seq in [0, u64::from(u32::MAX) + 1, 9_007_199_254_740_991] {
            let raw = TextMessage::outgoing(
                &message.session,
                &message.grant,
                message.generation,
                seq,
                "café 日本 🛡️".into(),
            )
            .unwrap();
            assert_eq!(TextMessage::parse(&raw).unwrap().seq, seq);
        }
    }
    #[test]
    fn text_refuses_other_kinds_unknown_fields_unsafe_bindings_and_invalid_unicode() {
        for (key, value) in [
            ("kind", json!("input-text")),
            ("v", json!(1)),
            ("p", json!("other")),
            ("session", json!("../other")),
            ("gen", json!(0)),
            ("seq", json!(9_007_199_254_740_992u64)),
            ("approved", json!(true)),
            ("text", json!("a\0b")),
            ("text", json!("")),
        ] {
            let mut message = fixture();
            message[key] = value;
            assert!(TextMessage::parse(&message.to_string()).is_err(), "{key}");
        }
        let raw = fixture()
            .to_string()
            .replace("synthetic clipboard text", "\\ud800");
        assert!(TextMessage::parse(&raw).is_err());
        let raw = fixture().to_string().replacen("{", "{\"seq\":1,", 1);
        assert!(TextMessage::parse(&raw).is_err());
    }
    #[test]
    fn serialization_obeys_the_frozen_wire_character_limit_even_for_valid_clipboard_size() {
        assert!(
            TextMessage::outgoing(
                "session_fixture_01",
                "grant_fixture_01",
                1,
                1,
                "x".repeat(16_384)
            )
            .is_err()
        );
        assert!(TextMessage::parse(&" ".repeat(16_385)).is_err());
        assert!(
            TextMessage::outgoing(
                "session_fixture_01",
                "grant_fixture_01",
                1,
                1,
                "日本".repeat(100)
            )
            .is_ok()
        );
    }
}
