//! Endpoint-authenticated ICE configuration. This is transport configuration,
//! never participant authority or a certificate-verification bypass.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

const MAX_RESPONSE: usize = 8192;
const MAX_TTL_MS: u64 = 3_600_000;
const CLOCK_SKEW_MS: u64 = 30_000;
const MAX_TIME: u64 = 8_640_000_000_000_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Response {
    ice_servers: Vec<Server>,
    expires_at: u64,
    ice_transport_policy: Policy,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Server {
    urls: Vec<String>,
    #[serde(
        default,
        deserialize_with = "provided_string",
        skip_serializing_if = "Option::is_none"
    )]
    username: Option<String>,
    #[serde(
        default,
        deserialize_with = "provided_string",
        skip_serializing_if = "Option::is_none"
    )]
    credential: Option<String>,
}
fn provided_string<'de, D: serde::Deserializer<'de>>(value: D) -> Result<Option<String>, D::Error> {
    String::deserialize(value).map(Some)
}
#[derive(Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Policy {
    All,
    Relay,
}

/// Constructed only from the fixed-origin HTTPS response. No Debug impl: TURN
/// credentials must not be included accidentally in diagnostic output.
pub(crate) struct Configuration {
    response: Response,
    validated_at: u64,
    until: Instant,
}
impl Configuration {
    pub(crate) fn parse(
        bytes: &[u8],
        started: Instant,
        now: u64,
        instant: Instant,
    ) -> Result<Self, &'static str> {
        if bytes.len() > MAX_RESPONSE
            || instant < started
            || instant.duration_since(started) >= Duration::from_secs(10)
            || now > MAX_TIME
        {
            return Err("invalid-ice");
        }
        let response: Response = serde_json::from_slice(bytes).map_err(|_| "invalid-ice")?;
        if response.expires_at <= now
            || response.expires_at > MAX_TIME
            || response.expires_at - now > MAX_TTL_MS + CLOCK_SKEW_MS
            || response.ice_servers.len() > 8
        {
            return Err("invalid-ice");
        }
        let mut urls = BTreeSet::new();
        let mut has_turn = false;
        for server in &response.ice_servers {
            if server.urls.is_empty() || server.urls.len() > 8 {
                return Err("invalid-ice");
            }
            let mut turn = None;
            for url in &server.urls {
                let relay = relay_url(url)?;
                if turn.is_some_and(|previous| previous != relay)
                    || !urls.insert(url.as_str())
                    || urls.len() > 8
                {
                    return Err("invalid-ice");
                }
                turn = Some(relay);
            }
            if turn == Some(true) {
                validate_credentials(server, response.expires_at)?;
                has_turn = true;
            } else if server.username.is_some() || server.credential.is_some() {
                return Err("invalid-ice");
            }
        }
        if response.ice_transport_policy == Policy::Relay && !has_turn {
            return Err("invalid-ice");
        }
        let until = (started + Duration::from_millis(MAX_TTL_MS))
            .min(instant + Duration::from_millis(response.expires_at - now));
        if instant >= until {
            return Err("invalid-ice");
        }
        Ok(Self {
            response,
            validated_at: now,
            until,
        })
    }
    /// Trusted transport construction only. Recheck after any queued work;
    /// never silently fall back to direct ICE after expiry or a refused relay.
    pub(crate) fn rtc(&self) -> Result<Value, &'static str> {
        self.rtc_at(super::lease::now()?, Instant::now())
    }
    fn rtc_at(&self, now: u64, instant: Instant) -> Result<Value, &'static str> {
        if now < self.validated_at || now >= self.response.expires_at || instant >= self.until {
            return Err("ice-expired");
        }
        Ok(
            json!({"iceServers":self.response.ice_servers,"iceTransportPolicy":self.response.ice_transport_policy}),
        )
    }
}
fn relay_url(value: &str) -> Result<bool, &'static str> {
    if value.len() > 320 || !value.is_ascii() {
        return Err("invalid-ice");
    }
    let (scheme, rest) = value.split_once(':').ok_or("invalid-ice")?;
    let relay = match scheme {
        "stun" | "stuns" => false,
        "turn" | "turns" => true,
        _ => return Err("invalid-ice"),
    };
    let host_port = match rest.split_once('?') {
        Some((host, "transport=udp" | "transport=tcp")) => host,
        Some(_) => return Err("invalid-ice"),
        None => rest,
    };
    let host = match host_port.split_once(':') {
        Some((host, port)) => {
            if port.is_empty()
                || port.len() > 5
                || !port.bytes().all(|c| c.is_ascii_digit())
                || port.parse::<u16>().ok().filter(|&v| v != 0).is_none()
            {
                return Err("invalid-ice");
            }
            host
        }
        None => host_port,
    };
    if host.is_empty()
        || host.len() > 253
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
    {
        return Err("invalid-ice");
    }
    if host.bytes().all(|c| c.is_ascii_digit() || c == b'.')
        && host.parse::<std::net::Ipv4Addr>().is_err()
    {
        return Err("invalid-ice");
    }
    Ok(relay)
}
fn validate_credentials(server: &Server, expires_at: u64) -> Result<(), &'static str> {
    let username = server.username.as_deref().ok_or("invalid-ice")?;
    let credential = server.credential.as_deref().ok_or("invalid-ice")?;
    let (expiry, pseudonym) = username.split_once(':').ok_or("invalid-ice")?;
    if expiry != (expires_at / 1000).to_string()
        || pseudonym.len() != 22
        || !pseudonym
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        || credential.len() != 28
    {
        return Err("invalid-ice");
    }
    let decoded = STANDARD.decode(credential).map_err(|_| "invalid-ice")?;
    if decoded.len() != 20 || STANDARD.encode(decoded) != credential {
        return Err("invalid-ice");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn direct(now: u64) -> Value {
        json!({"iceServers":[],"expiresAt":now+600000,"iceTransportPolicy":"all"})
    }
    fn relay(now: u64) -> Value {
        let expiry = now + 600000;
        json!({"iceServers":[{"urls":["stun:fixture.example:3478"]},{"urls":["turns:fixture.example:5349?transport=tcp"],"username":format!("{}:{}",expiry/1000,"p".repeat(22)),"credential":STANDARD.encode([1u8;20])}],"expiresAt":expiry,"iceTransportPolicy":"relay"})
    }
    fn parse(value: &Value, now: u64) -> Result<Configuration, &'static str> {
        let t = Instant::now();
        Configuration::parse(&serde_json::to_vec(value).unwrap(), t, now, t)
    }
    #[test]
    fn canonical_direct_stun_and_authenticated_turn_configs_are_transport_only() {
        let now = 1_800_000_000_000;
        for response in [direct(now), relay(now)] {
            let config = parse(&response, now).unwrap();
            let rtc = config.rtc_at(now, Instant::now()).unwrap();
            assert_eq!(rtc["iceServers"], response["iceServers"]);
            assert_eq!(rtc["iceTransportPolicy"], response["iceTransportPolicy"]);
            assert!(rtc.get("expiresAt").is_none());
            assert!(rtc.get("authorized").is_none());
        }
    }
    #[test]
    fn relay_configuration_rejects_untrusted_urls_and_invalid_ports() {
        for value in [
            "http://fixture.example",
            "https://fixture.example",
            "turn:user@fixture.example",
            "turn:fixture.example/path",
            "turn:fixture.example#fragment",
            "turn:fixture.example?transport=tcp&extra=1",
            "turn:fixture.example:0",
            "turn:fixture.example:65536",
            "turn:fixture.example:123456",
            "turn:fixture.example:abc",
            "turn:fixture.example\n",
            "turn:fixture..example",
            "turn:-fixture.example",
            "turn:fixture.example.",
            "turn:999.0.0.1",
            "turn:[::1]",
            "turn:é.example",
        ] {
            assert!(relay_url(value).is_err(), "{value}");
        }
        for value in [
            "stun:fixture.example",
            "stuns:fixture.example:443",
            "turn:127.0.0.1:3478?transport=udp",
            "turns:FIXTURE.example:65535?transport=tcp",
        ] {
            assert!(relay_url(value).is_ok(), "{value}");
        }
    }
    #[test]
    fn canonical_credentials_and_expiry_cannot_be_omitted_or_substituted() {
        let now = 1_800_000_000_000;
        for patch in [
            json!({"username":null}),
            json!({"credential":null}),
            json!({"credential":"not-a-credential"}),
            json!({"username":"1800000600:@account:example"}),
            json!({"username":format!("{}:{}",1800000601u64,"p".repeat(22))}),
            json!({"credential":STANDARD.encode([1u8;21])}),
        ] {
            let mut response = relay(now);
            for (key, value) in patch.as_object().unwrap() {
                response["iceServers"][1][key] = value.clone();
            }
            assert!(parse(&response, now).is_err());
        }
        let mut response = relay(now);
        response["iceServers"][0]["username"] = "unexpected".into();
        assert!(parse(&response, now).is_err());
        let mut response = relay(now);
        response["iceServers"][0]["username"] = Value::Null;
        assert!(parse(&response, now).is_err());
        let mut response = relay(now);
        response["iceServers"][1]["urls"] = json!(["stun:fixture.example"]);
        assert!(parse(&response, now).is_err());
        let mut response = direct(now);
        response["iceTransportPolicy"] = "relay".into();
        assert!(parse(&response, now).is_err());
    }
    #[test]
    fn strict_configuration_refuses_unknown_duplicate_oversized_and_stale_responses() {
        let now = 1_800_000_000_000;
        let mut response = direct(now);
        response["authorized"] = true.into();
        assert!(parse(&response, now).is_err());
        let mut response = relay(now);
        response["iceServers"][1]["credentialType"] = "password".into();
        assert!(parse(&response, now).is_err());
        let mut response = relay(now);
        response["iceServers"][1]["urls"] = json!(["turn:fixture.example", "turn:fixture.example"]);
        assert!(parse(&response, now).is_err());
        let mut response = direct(now);
        response["iceServers"] = json!([{"urls":[]}]);
        assert!(parse(&response, now).is_err());
        let mut response = direct(now);
        response["iceServers"] =
            json!([{"urls":(0..9).map(|n|format!("stun:fixture{n}.example")).collect::<Vec<_>>()}]);
        assert!(parse(&response, now).is_err());
        let mut response = direct(now);
        response["expiresAt"] = now.into();
        assert!(parse(&response, now).is_err());
        let mut response = direct(now);
        response["expiresAt"] = (now + MAX_TTL_MS + CLOCK_SKEW_MS + 1).into();
        assert!(parse(&response, now).is_err());
        let t = Instant::now();
        assert!(Configuration::parse(&vec![b' '; 8193], t, now, t).is_err());
        assert!(Configuration::parse(br#"{"iceServers":[],"expiresAt":1800000600000,"iceTransportPolicy":"all","iceTransportPolicy":"relay"}"#,t,now,t).is_err());
        assert!(
            Configuration::parse(
                &serde_json::to_vec(&direct(now)).unwrap(),
                t,
                now,
                t + Duration::from_secs(10)
            )
            .is_err()
        );
    }
    #[test]
    fn wall_clock_rollback_and_monotonic_expiry_never_reopen_relay_configuration() {
        let now = 1_800_000_000_000;
        let t = Instant::now();
        let config =
            Configuration::parse(&serde_json::to_vec(&relay(now)).unwrap(), t, now, t).unwrap();
        assert!(config.rtc_at(now - 1, t).is_err());
        assert!(config.rtc_at(now + 600000, t).is_err());
        assert!(config.rtc_at(now, t + Duration::from_secs(600)).is_err());
        let mut response = direct(now);
        response["expiresAt"] = (now + MAX_TTL_MS + CLOCK_SKEW_MS).into();
        let config =
            Configuration::parse(&serde_json::to_vec(&response).unwrap(), t, now, t).unwrap();
        assert!(config.rtc_at(now, t + Duration::from_secs(3600)).is_err());
    }
}
