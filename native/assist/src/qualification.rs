//! Optional compile-time disposable-service profile. No runtime trust override.
pub const ORIGIN: &str = match option_env!("PALISADE_ASSIST_BUILD_ORIGIN") {
    Some(origin) => origin,
    None => "https://palisade.onyxops.org",
};
const PIN: Option<&str> = option_env!("PALISADE_ASSIST_BUILD_ISSUER");
const NAMESPACE: Option<&str> = option_env!("PALISADE_ASSIST_BUILD_NAMESPACE");
pub fn target() -> Result<(&'static str, u16), &'static str> {
    validate(ORIGIN, PIN, NAMESPACE)
}
fn validate<'a>(
    origin: &'a str,
    pin: Option<&str>,
    namespace: Option<&str>,
) -> Result<(&'a str, u16), &'static str> {
    if origin == "https://palisade.onyxops.org" {
        return if pin.is_none() && namespace.is_none() {
            Ok(("palisade.onyxops.org", 443))
        } else {
            Err("invalid-build-profile")
        };
    }
    let pin = pin.ok_or("invalid-build-profile")?;
    let namespace = namespace.ok_or("invalid-build-profile")?;
    if pin.len() != 64
        || !pin
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        || !(3..=32).contains(&namespace.len())
        || !namespace.as_bytes()[0].is_ascii_lowercase()
        || !namespace
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
    {
        return Err("invalid-build-profile");
    }
    let target = origin
        .strip_prefix("https://")
        .ok_or("invalid-build-profile")?;
    let (host, port) = match target.split_once(':') {
        Some((host, port)) => (
            host,
            port.parse::<u16>().map_err(|_| "invalid-build-profile")?,
        ),
        None => (target, 443),
    };
    if host.is_empty()
        || host.len() > 253
        || host.trim_end_matches('.') == "palisade.onyxops.org"
        || port == 0
        || !host
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'.' || c == b'-')
    {
        return Err("invalid-build-profile");
    }
    Ok((host, port))
}
pub fn issuer_pin() -> Result<Option<&'static str>, &'static str> {
    target()?;
    Ok(PIN)
}
pub fn namespace() -> Result<Option<&'static str>, &'static str> {
    target()?;
    Ok(NAMESPACE)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qualification_requires_separate_origin_independent_pin_and_private_namespace() {
        let pin = "a".repeat(64);
        assert_eq!(
            validate(
                "https://fixture.example:8443",
                Some(&pin),
                Some("acceptance-v1")
            ),
            Ok(("fixture.example", 8443))
        );
        for origin in [
            "http://fixture.example",
            "https://palisade.onyxops.org",
            "https://palisade.onyxops.org.",
            "https://fixture.example/path",
            "https://user@fixture.example",
            "https://fixture.example:0",
        ] {
            assert!(validate(origin, Some(&pin), Some("acceptance-v1")).is_err());
        }
        assert!(validate("https://fixture.example", None, Some("acceptance-v1")).is_err());
        assert!(validate("https://fixture.example", Some(&pin), Some("../Assist")).is_err());
        assert!(
            validate(
                "https://fixture.example",
                Some("not-a-pin"),
                Some("acceptance-v1")
            )
            .is_err()
        );
    }
}
