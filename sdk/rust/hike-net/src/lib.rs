//! Network and policy code for the Satoshi Hike reference programs.
//! `hike` measures. This crate holds what is not measurement: the test-plane
//! policy judgment and the direct Bitcoin node read (section 14.6).
#![forbid(unsafe_code)]

mod chainrpc;
mod policy;

pub use chainrpc::ChainRpc;
pub use policy::{judge_test_policy, GateJudgment, GateSkip, TestGateMeasurement};
