//! Strict canonical input decoding and a write-ahead held-input ledger. No OS effects here.
use serde::{
    Deserialize,
    de::{self, MapAccess, Visitor},
};
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};
#[derive(Clone, Debug, PartialEq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Action {
    Move {
        x: f64,
        y: f64,
        source_gen: u32,
    },
    Button {
        button: String,
        down: bool,
        source_gen: u32,
    },
    Wheel {
        dx: i32,
        dy: i32,
        source_gen: u32,
    },
    Key {
        code: String,
        down: bool,
    },
    Shortcut(String),
    ReleaseAll,
}
impl Action {
    pub fn validate(&self) -> Result<(), &'static str> {
        let valid = match self {
            Action::Move { x, y, source_gen } => {
                x.is_finite()
                    && y.is_finite()
                    && (0.0..=1.0).contains(x)
                    && (0.0..=1.0).contains(y)
                    && (1..=1_000_000).contains(source_gen)
            }
            Action::Button {
                button, source_gen, ..
            } => {
                ["left", "middle", "right"].contains(&button.as_str())
                    && (1..=1_000_000).contains(source_gen)
            }
            Action::Wheel { dx, dy, source_gen } => {
                dx.abs_diff(0) <= 1200
                    && dy.abs_diff(0) <= 1200
                    && (1..=1_000_000).contains(source_gen)
            }
            Action::Key { code, .. } => scan_code(code).is_some(),
            Action::Shortcut(name) => shortcut(name).is_some(),
            Action::ReleaseAll => true,
        };
        if valid {
            Ok(())
        } else {
            Err("invalid-message")
        }
    }
}
#[derive(Clone, Debug)]
pub struct Message {
    pub session: String,
    pub grant: String,
    pub generation: u64,
    pub sequence: u32,
    pub action: Action,
}
struct StrictObject(Map<String, Value>);
impl<'de> Deserialize<'de> for StrictObject {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Object;
        impl<'de> Visitor<'de> for Object {
            type Value = StrictObject;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a bounded object without duplicate keys")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut result = Map::new();
                while let Some((key, value)) = map.next_entry::<String, Value>()? {
                    if result.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate-field"));
                    }
                }
                Ok(StrictObject(result))
            }
        }
        d.deserialize_map(Object)
    }
}
fn number(v: &Value, min: f64, max: f64) -> Result<f64, &'static str> {
    v.as_f64()
        .filter(|n| n.is_finite() && *n >= min && *n <= max)
        .ok_or("invalid-message")
}
fn integer(v: &Value, min: u64, max: u64) -> Result<u64, &'static str> {
    v.as_u64()
        .filter(|n| *n >= min && *n <= max)
        .ok_or("invalid-message")
}
fn text(v: &Value) -> Result<&str, &'static str> {
    v.as_str().ok_or("invalid-message")
}
pub fn parse(raw: &str) -> Result<Message, &'static str> {
    if raw.len() > 2048 {
        return Err("too-large");
    }
    let object: StrictObject = serde_json::from_str(raw).map_err(|_| "invalid-message")?;
    let m = &object.0;
    let get = |k: &str| m.get(k).ok_or("invalid-message");
    if text(get("p")?)? != "palisade.assist.control" || integer(get("v")?, 2, 2)? != 2 {
        return Err("incompatible-protocol");
    }
    let kind = text(get("kind")?)?;
    let fields: &[&str] = match kind {
        "pointer-move" => &["x", "y", "sourceGen"],
        "button" => &["button", "down", "sourceGen"],
        "wheel" => &["dx", "dy", "sourceGen"],
        "key" => &["code", "down"],
        "shortcut" => &["name"],
        "release-all" => &[],
        _ => return Err("invalid-message"),
    };
    if m.len() != 7 + fields.len()
        || m.keys().any(|k| {
            !fields.contains(&k.as_str())
                && !["p", "v", "session", "grant", "gen", "seq", "kind"].contains(&k.as_str())
        })
    {
        return Err("invalid-message");
    }
    let session = text(get("session")?)?.to_owned();
    let grant = text(get("grant")?)?.to_owned();
    if !crate::grant::token(&session, 8, 128) || !crate::grant::token(&grant, 8, 128) {
        return Err("invalid-message");
    }
    let source = || integer(get("sourceGen")?, 1, 1_000_000).map(|v| v as u32);
    let down = || get("down")?.as_bool().ok_or("invalid-message");
    let action = match kind {
        "pointer-move" => Action::Move {
            x: number(get("x")?, 0., 1.)?,
            y: number(get("y")?, 0., 1.)?,
            source_gen: source()?,
        },
        "button" => {
            let button = text(get("button")?)?;
            if !["left", "middle", "right"].contains(&button) {
                return Err("invalid-message");
            }
            Action::Button {
                button: button.into(),
                down: down()?,
                source_gen: source()?,
            }
        }
        "wheel" => {
            let dx = number(get("dx")?, -1200., 1200.)?;
            let dy = number(get("dy")?, -1200., 1200.)?;
            if dx.fract() != 0. || dy.fract() != 0. {
                return Err("invalid-message");
            };
            Action::Wheel {
                dx: dx as i32,
                dy: dy as i32,
                source_gen: source()?,
            }
        }
        "key" => {
            let code = text(get("code")?)?;
            if scan_code(code).is_none() {
                return Err("invalid-message");
            };
            Action::Key {
                code: code.into(),
                down: down()?,
            }
        }
        "shortcut" => {
            let name = text(get("name")?)?;
            if shortcut(name).is_none() {
                return Err("invalid-message");
            };
            Action::Shortcut(name.into())
        }
        _ => Action::ReleaseAll,
    };
    Ok(Message {
        session,
        grant,
        generation: integer(get("gen")?, 1, 9_007_199_254_740_991)?,
        sequence: integer(get("seq")?, 0, u32::MAX as u64)? as u32,
        action,
    })
}
/// Physical DOM code -> Windows set-1 scan code + extended bit. Host layout processes it normally.
pub fn scan_code(code: &str) -> Option<(u16, bool)> {
    if let Some(letter) = code.strip_prefix("Key").filter(|v| v.len() == 1) {
        let codes = [
            0x1e, 0x30, 0x2e, 0x20, 0x12, 0x21, 0x22, 0x23, 0x17, 0x24, 0x25, 0x26, 0x32, 0x31,
            0x18, 0x19, 0x10, 0x13, 0x1f, 0x14, 0x16, 0x2f, 0x11, 0x2d, 0x15, 0x2c,
        ];
        let c = letter.as_bytes()[0];
        return c
            .is_ascii_uppercase()
            .then(|| (codes[(c - b'A') as usize], false));
    }
    if let Some(d) = code.strip_prefix("Digit").filter(|v| v.len() == 1) {
        let c = d.as_bytes()[0];
        return c.is_ascii_digit().then(|| {
            (
                if c == b'0' {
                    0x0b
                } else {
                    u16::from(c - b'1') + 2
                },
                false,
            )
        });
    }
    if let Some(n) = code
        .strip_prefix('F')
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|n| (1..=12).contains(n))
    {
        if code != format!("F{n}") {
            return None;
        }
        return Some((if n < 11 { 0x3a + n } else { 0x57 + (n - 11) }, false));
    }
    Some(match code {
        "ControlLeft" => (0x1d, false),
        "ControlRight" => (0x1d, true),
        "ShiftLeft" => (0x2a, false),
        "ShiftRight" => (0x36, false),
        "AltLeft" => (0x38, false),
        "AltRight" => (0x38, true),
        "Enter" => (0x1c, false),
        "Escape" => (1, false),
        "Tab" => (0xf, false),
        "Space" => (0x39, false),
        "Backspace" => (0xe, false),
        "Delete" => (0x53, true),
        "Insert" => (0x52, true),
        "Home" => (0x47, true),
        "End" => (0x4f, true),
        "PageUp" => (0x49, true),
        "PageDown" => (0x51, true),
        "ArrowUp" => (0x48, true),
        "ArrowDown" => (0x50, true),
        "ArrowLeft" => (0x4b, true),
        "ArrowRight" => (0x4d, true),
        "Minus" => (0xc, false),
        "Equal" => (0xd, false),
        "BracketLeft" => (0x1a, false),
        "BracketRight" => (0x1b, false),
        "Backslash" => (0x2b, false),
        "Semicolon" => (0x27, false),
        "Quote" => (0x28, false),
        "Backquote" => (0x29, false),
        "Comma" => (0x33, false),
        "Period" => (0x34, false),
        "Slash" => (0x35, false),
        _ => return None,
    })
}
pub fn shortcut(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "copy" => &["ControlLeft", "KeyC"],
        "paste" => &["ControlLeft", "KeyV"],
        "cut" => &["ControlLeft", "KeyX"],
        "selectAll" => &["ControlLeft", "KeyA"],
        "undo" => &["ControlLeft", "KeyZ"],
        "redo" => &["ControlLeft", "KeyY"],
        "save" => &["ControlLeft", "KeyS"],
        "find" => &["ControlLeft", "KeyF"],
        "nextTab" => &["ControlLeft", "Tab"],
        "previousTab" => &["ControlLeft", "ShiftLeft", "Tab"],
        _ => return None,
    })
}
#[derive(Clone, Default, Debug)]
pub struct Ledger {
    keys: BTreeSet<String>,
    buttons: BTreeSet<String>,
}
impl Ledger {
    pub fn is_held(&self, action: &Action) -> bool {
        match action {
            Action::Key { code, .. } => self.keys.contains(code),
            Action::Button { button, .. } => self.buttons.contains(button),
            _ => false,
        }
    }
    pub fn owns(&self, action: &Action) -> bool {
        match action {
            Action::Key { code, down: false } => self.keys.contains(code),
            Action::Button {
                button,
                down: false,
                ..
            } => self.buttons.contains(button),
            _ => true,
        }
    }
    pub fn plan(&mut self, action: &Action) -> Result<Vec<Action>, &'static str> {
        match action {
            Action::Key { code, down } => {
                if scan_code(code).is_none() {
                    return Err("invalid-message");
                };
                let mut next = self.keys.clone();
                if *down {
                    next.insert(code.clone());
                } else {
                    next.remove(code);
                }
                let alt = next.contains("AltLeft") || next.contains("AltRight");
                let ctrl = next.contains("ControlLeft") || next.contains("ControlRight");
                let shift = next.contains("ShiftLeft") || next.contains("ShiftRight");
                if (alt && next.contains("Tab"))
                    || (ctrl && alt && next.contains("Delete"))
                    || (ctrl && shift && next.contains("Escape"))
                    || next.len() > 16
                {
                    return Err("protected-shortcut");
                }
                self.keys = next;
                Ok(vec![action.clone()])
            }
            Action::Button { button, down, .. } => {
                if !["left", "middle", "right"].contains(&button.as_str()) {
                    return Err("invalid-message");
                };
                if *down {
                    self.buttons.insert(button.clone());
                } else {
                    self.buttons.remove(button);
                }
                Ok(vec![action.clone()])
            }
            Action::Shortcut(name) => {
                let keys = shortcut(name).ok_or("invalid-message")?;
                if !self.keys.is_empty() || !self.buttons.is_empty() {
                    return Err("held-input");
                }
                // Expansion is not a ledger commit. Dispatcher records every individual
                // intended down before sending it and clears only after its up completes.
                let mut plan = Vec::new();
                for &code in keys {
                    plan.push(Action::Key {
                        code: code.into(),
                        down: true,
                    });
                }
                for &code in keys.iter().rev() {
                    plan.push(Action::Key {
                        code: code.into(),
                        down: false,
                    });
                }
                Ok(plan)
            }
            Action::ReleaseAll => Ok(self.releases()),
            _ => Ok(vec![action.clone()]),
        }
    }
    pub fn releases(&mut self) -> Vec<Action> {
        let mut result = Vec::new();
        for code in std::mem::take(&mut self.keys) {
            result.push(Action::Key { code, down: false })
        }
        for button in std::mem::take(&mut self.buttons) {
            result.push(Action::Button {
                button,
                down: false,
                source_gen: 1,
            })
        }
        result
    }
    pub fn held(&self) -> bool {
        !self.keys.is_empty() || !self.buttons.is_empty()
    }
    pub fn snapshot(&self) -> BTreeMap<String, bool> {
        self.keys
            .iter()
            .chain(self.buttons.iter())
            .map(|v| (v.clone(), true))
            .collect()
    }
}
