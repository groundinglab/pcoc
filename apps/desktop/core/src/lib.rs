//! PCOC Hike desktop core. Everything here is free of GTK so it can be
//! tested without a display.
#![forbid(unsafe_code)]

pub mod about;
pub mod config;
pub mod dm;
pub mod endpoints;
pub mod format;
pub mod hex;
pub mod keys;
mod object_key;
pub mod library;
pub mod produce;
pub mod record;
#[cfg(any(test, feature = "sample"))]
pub mod sample;
pub mod signin;
pub mod verify_job;

#[cfg(test)]
#[path = "vectors-allowlist/nip19_vector.rs"]
mod nip19_vector;
