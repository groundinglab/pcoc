//! Windows producer core. No window crate, no GTK, no direct-message send.
//!
//! W1 is compile plus harness. The Linux desktop under `apps/desktop` is a
//! separate tree and is not replaced by this one.
#![forbid(unsafe_code)]

pub mod claims;
pub mod endpoints;
pub mod hex;
pub mod history;
pub mod keys;
pub mod pin;
pub mod produce;
pub mod signin;
pub mod verify;

#[cfg(test)]
mod harness;

#[cfg(test)]
#[path = "vectors-allowlist/nip19_vector.rs"]
mod nip19_vector;
