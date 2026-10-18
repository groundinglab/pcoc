//! Network code for the Satoshi Hike reference programs.
//! `hike` measures. This crate holds what is not measurement: the direct
//! Bitcoin node read (section 14.6). Policy values and judgments are not SDK
//! code (rc.2): the test-plane judgment moved to the admission gate's test
//! fixtures (services/admission-gate/tests/test_plane_policy.rs).
#![forbid(unsafe_code)]

mod chainrpc;

pub use chainrpc::ChainRpc;
