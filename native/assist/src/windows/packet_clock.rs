//! The pinned main runtime's hrtime.bigint is raw libuv QPC nanoseconds.
//! Use it only on the authenticated main/native pipe, never from a web packet.
use std::time::{Duration, Instant};
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

fn elapsed(stamp: &str, ticks: i64, frequency: i64) -> Result<Duration, &'static str> {
    if stamp.is_empty()
        || stamp.len() > 20
        || !stamp.bytes().all(|v| v.is_ascii_digit())
        || (stamp.len() > 1 && stamp.starts_with('0'))
        || ticks < 0
        || frequency <= 0
    {
        return Err("invalid-input-clock");
    }
    let sent = stamp.parse::<u64>().map_err(|_| "invalid-input-clock")? as u128;
    let now = (ticks as u128)
        .checked_mul(1_000_000_000)
        .ok_or("invalid-input-clock")?
        / frequency as u128;
    // libuv divides two doubles and truncates. Bound that rounding, floor and
    // one QPC tick conservatively; never move an arrival later to make it pass.
    let slop = now.max(sent).div_ceil(1 << 48) + 1_000_000_000u128.div_ceil(frequency as u128) + 4;
    if sent > now + slop {
        return Err("invalid-input-clock");
    }
    let age = now
        .saturating_sub(sent)
        .checked_add(slop)
        .ok_or("invalid-input-clock")?;
    if age > 60_000_000_000 {
        return Err("stale-input");
    }
    Ok(Duration::from_nanos(
        u64::try_from(age).map_err(|_| "invalid-input-clock")?,
    ))
}
pub(crate) fn ingress(stamp: &str) -> Result<Instant, &'static str> {
    // Sampling in this order makes scheduling between reads conservative.
    let instant = Instant::now();
    let mut ticks = 0;
    let mut frequency = 0;
    if unsafe { QueryPerformanceCounter(&mut ticks) } == 0
        || unsafe { QueryPerformanceFrequency(&mut frequency) } == 0
    {
        return Err("invalid-input-clock");
    }
    instant
        .checked_sub(elapsed(stamp, ticks, frequency)?)
        .ok_or("invalid-input-clock")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_locked_electron_main_clock_and_native_qpc_share_the_same_origin() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let before = Instant::now();
        let output =
            std::process::Command::new(root.join("node_modules/electron/dist/electron.exe"))
                .arg(root.join("scripts/assist-clock-smoke.cjs"))
                .env_remove("ELECTRON_RUN_AS_NODE")
                .env_remove("NODE_OPTIONS")
                .output()
                .expect("locked Electron is required for Windows clock qualification");
        assert!(
            output.status.success(),
            "no-authority Electron clock fixture failed"
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let line = stdout
            .lines()
            .find_map(|line| line.strip_prefix("PALISADE_CLOCK="))
            .expect("clock evidence required");
        let evidence: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(evidence["electron"], "44.7.2");
        assert_eq!(evidence["node"], "24.21.0");
        assert_eq!(evidence["uv"], "1.52.1");
        let arrival = ingress(evidence["stamp"].as_str().unwrap()).unwrap();
        assert!(arrival <= Instant::now());
        assert!(arrival >= before - Duration::from_millis(1));
    }
    #[test]
    fn checked_main_stamp_is_backdated_with_conservative_tick_and_rounding_slop() {
        let age = elapsed("900000000", 10_000_000, 10_000_000).unwrap();
        assert!(age > Duration::from_millis(100));
        assert!(age < Duration::from_millis(101));
        for stamp in ["", "-1", "01", "1.0", "18446744073709551616", "1000001000"] {
            assert!(elapsed(stamp, 10_000_000, 10_000_000).is_err());
        }
        assert!(elapsed("1", -1, 10).is_err());
        assert!(elapsed("1", 1, 0).is_err());
        assert!(elapsed("1", 1_000_000_000, 1).is_err());
        for frequency in [1_000_000i64, 10_000_000, 1_000_000_000] {
            let slop = 1 + 1_000_000_000u128.div_ceil(frequency as u128) + 4;
            assert!(elapsed(&(1_000_000_000 + slop).to_string(), frequency, frequency).is_ok());
            assert!(elapsed(&(1_000_000_001 + slop).to_string(), frequency, frequency).is_err());
        }
    }
}
