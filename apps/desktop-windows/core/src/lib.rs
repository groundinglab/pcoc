//! Windows producer core. The egui window is the `app` crate.
//!
//! The Linux desktop under `apps/desktop` is a separate tree and is not
//! replaced by this one. This crate has no GTK dependency.
#![forbid(unsafe_code)]

pub mod claims;
pub mod dm;
pub mod endpoints;
pub mod hex;
pub mod history;
pub mod keys;
mod object_key;
pub mod pin;
pub mod produce;
pub mod signin;
pub mod verify;

#[cfg(test)]
mod harness;

#[cfg(test)]
#[path = "vectors-allowlist/nip19_vector.rs"]
mod nip19_vector;
