use palisade_assist_guardian::input::*;
use serde_json::json;
fn message(kind: &str) -> serde_json::Value {
    json!({"p":"palisade.assist.control","v":2,"session":"session_fixture_01","grant":"grant_fixture_01","gen":1,"seq":0,"kind":kind})
}
#[test]
fn bounded_wire_fields_duplicates_and_protected_keys() {
    let mut m = message("key");
    m["code"] = "KeyA".into();
    m["down"] = true.into();
    assert!(parse(&m.to_string()).is_ok());
    for code in ["MetaLeft", "OSLeft", "NumpadEnter", "Unknown", "F13"] {
        m["code"] = code.into();
        assert!(parse(&m.to_string()).is_err());
    }
    m["code"] = "KeyA".into();
    m["extra"] = true.into();
    assert!(parse(&m.to_string()).is_err());
    assert!(parse("{\"p\":\"x\",\"p\":\"y\"}").is_err());
    let mut m = message("pointer-move");
    m["x"] = (-0.1).into();
    m["y"] = 0.5.into();
    m["sourceGen"] = 1.into();
    assert!(parse(&m.to_string()).is_err());
}
#[test]
fn every_modifier_order_and_shortcut_cleanup() {
    for keys in [
        ["ControlLeft", "AltLeft", "Delete"],
        ["Delete", "ControlLeft", "AltLeft"],
        ["AltLeft", "Tab", "ControlLeft"],
        ["ControlLeft", "ShiftLeft", "Escape"],
    ] {
        let mut ledger = Ledger::default();
        let mut denied = false;
        for key in keys {
            if ledger
                .plan(&Action::Key {
                    code: key.into(),
                    down: true,
                })
                .is_err()
            {
                denied = true;
                break;
            }
        }
        assert!(denied);
        assert!(!ledger.releases().is_empty());
        assert!(!ledger.held());
    }
    let mut ledger = Ledger::default();
    let steps = ledger
        .plan(&Action::Shortcut("previousTab".into()))
        .unwrap();
    for step in &steps[..3] {
        ledger.plan(step).unwrap();
    }
    assert!(ledger.held());
    assert_eq!(ledger.releases().len(), 3);
}
#[test]
fn right_modifiers_and_physical_scan_codes() {
    assert_eq!(scan_code("KeyA"), Some((0x1e, false)));
    assert_eq!(scan_code("ControlRight"), Some((0x1d, true)));
    assert_eq!(scan_code("AltRight"), Some((0x38, true)));
    assert!(scan_code("MetaRight").is_none());
}
#[cfg(windows)]
#[test]
fn source_translation_handles_negative_origins_portrait_and_rejects_outside() {
    use palisade_assist_guardian::windows::input::Source;
    let s = Source {
        x: -1080,
        y: -500,
        width: 1080,
        height: 1920,
        generation: 1,
    };
    assert_eq!(s.point(0., 0.).unwrap(), (-1080, -500));
    assert_eq!(s.point(1., 1.).unwrap(), (-1, 1419));
    assert!(s.point(1.1, 0.).is_err());
    assert!(s.point(f64::NAN, 0.).is_err());
}
