//! No input injection, capture, clipboard, network or remote filesystem operations.
pub mod framing;
pub mod grant;
pub mod guard;
#[cfg(windows)]
pub mod windows;
