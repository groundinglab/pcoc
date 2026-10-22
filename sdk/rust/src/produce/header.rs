//! SPEC §9 header source, as the producer reads it.
//!
//! Hand-off for the CoS: P3 implements this trait (gossip, `HEADER_PEERS_MIN`,
//! silence). P2 did not find it on the core, so this module is the minimal
//! contract. `tip` returns at once. Nothing here waits on the walk thread.

use std::sync::Arc;

/// A header the source already holds. `natural` is `SHA-256(SHA-256(raw))` when `raw` is present.
#[derive(Clone, Debug)]
pub struct Observed {
    pub natural: [u8; 32],
    pub height: u64,
    pub raw: Option<[u8; 80]>,
}

/// SPEC §9 / Rule A1.1. The post-checkpoint read. It never waits.
#[derive(Clone, Debug)]
pub enum TipRead {
    /// The latest header already held.
    Ready(Observed),
    /// The source holds nothing yet. Not a failure.
    Empty,
    /// The read did not succeed.
    Failed,
}

/// SPEC §9. Implemented by P3. Test doubles live in the producer tests.
pub trait HeaderSource: Send + Sync {
    /// Latest header already held. Must return at once and must not take a lock an I/O call holds.
    fn tip(&self) -> TipRead;

    /// SPEC §13.1 fee header: the 80-byte best tip already held, if any.
    /// Not a walk tip read. Must return at once.
    fn fee_header(&self) -> Option<[u8; 80]>;

    /// Headers learned since the previous take, in learn order. Returns at once.
    fn take_learned(&self) -> Vec<Observed>;

    /// Park until [`take_learned`] would be non-empty, or [`wake`] runs.
    /// The ask thread calls this. The walk does not.
    fn wait_learned(&self);

    /// Unblock [`wait_learned`]. Used when a plant becomes 204 and at the close.
    ///
    /// The wake is sticky: a call that happens before `wait_learned` still
    /// unblocks the next wait. P3's implementation has to keep that, or a 204
    /// that lands while the ask thread is between waits is lost.
    fn wake(&self);
}

/// Shared source handle.
pub type Source = Arc<dyn HeaderSource>;
