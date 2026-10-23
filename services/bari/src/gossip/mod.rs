//! Live Bitcoin header source for `BARI_TIP_SOURCE=bitcoin-gossip`.
//!
//! The session, the peer swap, and the fail-closed quorum bound live in
//! `pcoc-gossip`. This module is the Bari clock's door to that crate.
//! The absent-source sentence is not a return of the live arm.

pub use pcoc_gossip::{
    gossip_self_check, open_pending, LiveClock, ABSENT_LIVE_SOURCE, LIVE_ARM_NAME,
};

/// What `bari --self-check` prints when the release binary took this arm.
pub fn release_self_check() -> Result<&'static str, String> {
    let clock = crate::header_source::HeaderClock::start(
        "self-check.invalid:8333",
        crate::header_source::GossipNetwork::Mainnet,
    );
    if clock.absent_live_source() || !clock.live_source_in_build() {
        return Err(ABSENT_LIVE_SOURCE.to_owned());
    }
    gossip_self_check()?;
    Ok(LIVE_ARM_NAME)
}
