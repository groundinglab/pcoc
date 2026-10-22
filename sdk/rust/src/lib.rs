//! Satoshi Hike 2.2 core. Behaviour is `spec/satoshi-hike-v2.2/SPEC.md`.
#![forbid(unsafe_code)]

pub mod bari_rule;
pub mod cbor;
pub mod checkpoint;
pub mod compare;
pub mod constants;
pub mod draw;
pub mod hash;
pub mod hashcash;
pub mod hexutil;
pub mod hike;
pub mod indexdoc;
pub mod jsonutil;
pub mod mix;
pub mod object;
pub mod path;
pub mod pin;
pub mod profile;
pub mod step;
pub mod terrain;
pub mod verify;
pub mod waypoint;

/// SPEC §6 / Rule I5. The SDK version constant is `2.2.0`.
pub const VERSION: &str = "2.2.0";

/// SPEC §6 / Rule I5. Reports the SDK version.
pub fn version() -> &'static str {
    VERSION
}
