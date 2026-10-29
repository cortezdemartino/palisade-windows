//! Verification/state core and dormant Windows adapters. Production activation
//! requires the separate native integration and device-qualification gates.
pub mod cleanup;
pub mod executor;
pub mod framing;
pub mod grant;
pub mod guard;
pub mod input;
pub mod issuer;
pub mod live;
pub mod statements;
pub mod stream;
#[cfg(windows)]
pub mod windows;
