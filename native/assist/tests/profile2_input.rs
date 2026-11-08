use palisade_assist_guardian::input::*;
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/assist-control-v2-profile2.json"
    ))
    .unwrap()
}
#[test]
fn stale_drop_is_handled_without_hiding_authority_or_replay_failure() {
    assert_eq!(packet_result(Err("stale-input")), Ok(()));
    assert_eq!(packet_result(Ok(())), Ok(()));
    for error in [
        "replay",
        "revoked",
        "invalid-message",
        "expired",
        "peer-unverified",
        "not-granted",
        "input-not-ready",
        "endpoint-failed",
    ] {
        assert_eq!(packet_result(Err(error)), Err(error));
    }
}
#[test]
fn pinned_profile2_input_messages_and_legacy_separation() {
    let f = fixture();
    for case in f["messages"]["valid"].as_array().unwrap() {
        if case["feature"] == "input2" {
            let raw = case["message"].to_string();
            assert!(parse_profile2(&raw).is_ok(), "{}", case["name"]);
            assert!(parse(&raw).is_err());
        }
    }
    for case in f["messages"]["invalid"].as_array().unwrap() {
        if case["message"]["kind"]
            .as_str()
            .is_some_and(|k| k.starts_with("input-"))
        {
            // Negotiation failures are checked by the bridge, not this v2-only parser.
            if case["error"] == "unsupported" {
                continue;
            }
            let mut m = case["message"].clone();
            if let Some(v) = case["version"].as_u64() {
                m["v"] = v.into();
            }
            if let Some(units) = case["textCodeUnits"].as_array() {
                let escaped = units
                    .iter()
                    .map(|n| format!("\\u{:04x}", n.as_u64().unwrap()))
                    .collect::<String>();
                m["text"] = Value::Null;
                let raw = m
                    .to_string()
                    .replace("\"text\":null", &format!("\"text\":\"{escaped}\""));
                assert!(parse_profile2(&raw).is_err(), "{}", case["name"]);
            } else {
                assert!(parse_profile2(&m.to_string()).is_err(), "{}", case["name"]);
            }
        }
    }
    for code in f["keyCodesAdded"].as_array().unwrap() {
        assert!(scan_code2(code.as_str().unwrap()).is_some());
        assert!(scan_code(code.as_str().unwrap()).is_none());
    }
    for code in [
        "MetaLeft",
        "MetaRight",
        "OSLeft",
        "PrintScreen",
        "Pause",
        "Power",
        "F013",
        "F25",
    ] {
        assert!(scan_code2(code).is_none());
    }
    assert_eq!(scan_code2("NumpadEnter"), Some((0x1c, true)));
    assert_eq!(scan_code2("NumpadDivide"), Some((0x35, true)));
    assert_eq!(scan_code2("F24"), Some((0x76, false)));
}
#[test]
fn profile2_text_is_bounded_well_formed_nfc_and_strict() {
    for text in ["é", "日本語", "🙂", &"🙂".repeat(128)] {
        assert!(valid_text(text));
    }
    for text in [
        "",
        "e\u{301}",
        "\n",
        "\t",
        "\u{85}",
        "\u{fffe}",
        "\u{ffff}",
        &"🙂".repeat(129),
    ] {
        assert!(!valid_text(text));
    }
    let raw=json!({"p":"palisade.assist.control","v":2,"session":"session_fixture_01","grant":"grant_fixture_01","gen":1,"seq":1,"kind":"input-text","text":"é","t":1}).to_string();
    assert!(parse_profile2(&raw).is_ok());
    assert!(parse_profile2(&raw.replace("\"t\":1", "\"t\":1,\"t\":2")).is_err());
    assert!(parse_profile2(&raw.replace("\"t\":1", "\"t\":1,\"extra\":0")).is_err());
}
#[test]
fn every_protected_combination_order_and_modifier_side_is_refused() {
    fn permutations(keys: &[String]) -> Vec<Vec<String>> {
        if keys.is_empty() {
            return vec![vec![]];
        }
        let mut out = vec![];
        for i in 0..keys.len() {
            let mut rest = keys.to_vec();
            let k = rest.remove(i);
            for mut p in permutations(&rest) {
                p.insert(0, k.clone());
                out.push(p);
            }
        }
        out
    }
    for case in fixture()["protectedCombinations"].as_array().unwrap() {
        for right in [false, true] {
            let keys = case
                .as_array()
                .unwrap()
                .iter()
                .map(|k| {
                    if right {
                        k.as_str().unwrap().replace("Left", "Right")
                    } else {
                        k.as_str().unwrap().into()
                    }
                })
                .collect::<Vec<_>>();
            for order in permutations(&keys) {
                let mut ledger = Ledger::default();
                assert!(
                    order
                        .into_iter()
                        .any(|code| ledger.plan(&Action::Key { code, down: true }).is_err())
                );
            }
        }
    }
}
#[test]
fn text_repeat_and_extra_buttons_keep_owned_cleanup() {
    for modifier in [
        "ControlLeft",
        "ControlRight",
        "ShiftLeft",
        "ShiftRight",
        "AltLeft",
        "AltRight",
    ] {
        let mut ledger = Ledger::default();
        ledger
            .plan(&Action::Key {
                code: modifier.into(),
                down: true,
            })
            .unwrap();
        assert_eq!(ledger.plan(&Action::Text("x".into())), Err("held-input"));
        assert_eq!(
            ledger.plan(&Action::Unicode {
                unit: 120,
                down: true
            }),
            Err("held-input")
        );
    }
    let mut ledger = Ledger::default();
    assert!(ledger.plan(&Action::RepeatKey("KeyA".into())).is_err());
    ledger
        .plan(&Action::Key {
            code: "KeyA".into(),
            down: true,
        })
        .unwrap();
    assert_eq!(
        ledger
            .plan(&Action::RepeatKey("KeyA".into()))
            .unwrap()
            .len(),
        1
    );
    ledger.releases();
    let steps = ledger.plan(&Action::Text("🙂".into())).unwrap();
    assert_eq!(steps.len(), 4);
    ledger.plan(&steps[0]).unwrap();
    assert_eq!(ledger.snapshot().len(), 1);
    assert_eq!(
        ledger.releases(),
        vec![Action::Unicode {
            unit: 0xd83d,
            down: false
        }]
    );
    for button in ["back", "forward"] {
        ledger
            .plan(&Action::Button {
                button: button.into(),
                down: true,
                source_gen: 1,
            })
            .unwrap();
    }
    assert_eq!(ledger.releases().len(), 2);
    assert!(!ledger.held());
}
