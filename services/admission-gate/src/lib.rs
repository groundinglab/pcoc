//! Admission Gate for a Satoshi Hike v2.0 object.
//!
//! One process reads one config file. `POST /v1/sign-in` is the admission
//! door. The body is the locked §15.1 six-field JSON object. The website
//! polls `GET /v1/sign-in/result/<nonce>`. The phone is done after the POST.
//! A signature miss is `bad_signature` with no message. Anything else the
//! phone sees is delivery of that POST.
//!
//! [`verifier`] is the only module that will call the v2.0 reference crate.
//! This crate does not run v1.35 verify and does not load a v1.35 profile.
#![forbid(unsafe_code)]

pub mod canon;
pub mod config;
pub mod fast;
pub mod door;
pub mod http;
pub mod ids;
pub mod jobs;
pub mod log;
pub mod policy;
pub mod ratelimit;
pub mod reasons;
pub mod signin_row;
pub mod thorough;
pub mod upload;
pub mod verifier;
pub mod wire;

pub use config::Config;
pub use door::Door;
pub use http::Service;
pub use verifier::{reference_linked, SPEC_KIND, SPEC_NAME};
