//! groundinglab admission claims.
//!
//! One process, one SQLite file, one issuer key. Stage 1 issues the badge.
//! The name table is created and unused. Lookups of published artifacts never
//! reach this process.
#![forbid(unsafe_code)]

pub mod award;
pub mod backup;
pub mod clock;
pub mod config;
pub mod error;
pub mod http;
pub mod key;
pub mod logproof;
pub mod names;
pub mod nostr;
pub mod nostr_json;
pub mod publish;
pub mod rate;
pub mod s3;
pub mod store;
pub mod timeutil;

pub use clock::{Clock, ManualClock, SystemClock};
pub use config::Config;
pub use http::Service;
pub use publish::RelaySender;
