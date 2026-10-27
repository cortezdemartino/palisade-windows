//! No input injection, capture, clipboard, network or remote filesystem operations.
pub mod executor;
pub mod framing;
pub mod grant;
pub mod guard;
pub mod input;
pub mod issuer;
pub mod statements;
pub mod stream;
#[cfg(windows)]
pub mod windows;
