//! Verification/state core and dormant Windows adapters. Production activation
//! requires the separate native integration and device-qualification gates.
pub mod cancellation;
pub mod cleanup;
pub mod cooperative;
pub mod custody;
pub mod executor;
pub mod framing;
pub mod grant;
pub mod guard;
pub mod input;
pub mod input_age;
pub mod issuer;
pub mod live;
pub mod statements;
pub mod stream;
#[cfg(windows)]
pub mod windows;
