#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    let result =
        palisade_update_helper::windows::application::run(std::env::args().skip(1).collect());
    #[cfg(not(windows))]
    let result: Result<Option<serde_json::Value>, &'static str> = Err("windows-x64-required");
    use std::io::Write;
    match result {
        Ok(Some(value)) => {
            if writeln!(std::io::stdout(), "{value}").is_err() {
                std::process::exit(1);
            }
        }
        Ok(None) => {}
        Err(code) => {
            let _ = writeln!(std::io::stderr(), "{code}");
            std::process::exit(1);
        }
    }
}
