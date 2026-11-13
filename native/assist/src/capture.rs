//! Closed app-owned capture metadata. These private fields are never accepted
//! from the hosted bridge and do not create session or input authority.
use crate::executor::Source;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalCandidate {
    pub port: u16,
    pub address: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub operation: String,
    pub source: String,
    pub display: Option<String>,
    pub bounds: Option<Source>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Screen,
    Window(u64),
}
impl Selection {
    pub fn validate(&self) -> Result<Kind, &'static str> {
        if !crate::grant::token(&self.operation, 8, 128) || self.source.len() > 128 {
            return Err("invalid-source");
        }
        let parts: Vec<_> = self.source.split(':').collect();
        if parts.len() != 3 || parts[1].is_empty() || !parts[1].bytes().all(|v| v.is_ascii_digit())
        {
            return Err("invalid-source");
        }
        if let Some(display) = &self.display {
            if display.is_empty()
                || display.len() > 20
                || display.parse::<i64>().is_err()
                || !display
                    .strip_prefix('-')
                    .unwrap_or(display)
                    .bytes()
                    .all(|v| v.is_ascii_digit())
            {
                return Err("invalid-source");
            }
        }
        match parts[0] {
            "screen" if parts[2] == "0" && self.display.is_some() => {
                parts[1].parse::<u32>().map_err(|_| "invalid-source")?;
                let bounds = self.bounds.as_ref().ok_or("invalid-source")?;
                bounds.point(0., 0.)?;
                if bounds.generation != 1 {
                    return Err("invalid-source");
                }
                Ok(Kind::Screen)
            }
            "window" if ["0", "1"].contains(&parts[2]) && self.bounds.is_none() => {
                let window = parts[1]
                    .parse::<u64>()
                    .ok()
                    .filter(|v| *v != 0)
                    .ok_or("invalid-source")?;
                Ok(Kind::Window(window))
            }
            _ => Err("invalid-source"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn screen() -> Selection {
        Selection {
            operation: "capture_fixture".into(),
            source: "screen:2:0".into(),
            display: Some("123".into()),
            bounds: Some(Source {
                x: -1920,
                y: 0,
                width: 1920,
                height: 1080,
                generation: 1,
            }),
        }
    }
    #[test]
    fn selection_requires_exact_source_kind_geometry_and_generation_without_first_display_fallback()
    {
        assert_eq!(screen().validate(), Ok(Kind::Screen));
        for display in ["-", "--1", "+1", "9223372036854775808"] {
            let mut value = screen();
            value.display = Some(display.into());
            assert!(value.validate().is_err());
        }
        for source in [
            "screen:0:1",
            "screen::0",
            "screen:-1:0",
            "window:0:0",
            "window:1:3",
            "screen:1:0:extra",
            "any:1:0",
        ] {
            let mut value = screen();
            value.source = source.into();
            assert!(value.validate().is_err(), "{source}");
        }
        let mut value = screen();
        value.display = None;
        assert!(value.validate().is_err());
        let mut value = screen();
        value.bounds.as_mut().unwrap().generation = 2;
        assert!(value.validate().is_err());
        let mut value = screen();
        value.source = "window:123:0".into();
        value.bounds = None;
        assert_eq!(value.validate(), Ok(Kind::Window(123)));
    }
}
