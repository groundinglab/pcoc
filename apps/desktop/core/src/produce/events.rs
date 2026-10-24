//! Progress events, DESIGN-v2 section 6.
//!
//! Offline produce maps rig progress onto these events. Live produce maps
//! the gossip header source onto the same events. None of these run inside
//! a step.

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AskOutcome {
    Served,
    /// A miss, with the section 13.5 phrase when the body was one of those phrases.
    Miss(Option<String>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProduceEvent {
    Started {
        output: PathBuf,
        public_key_hex: String,
        at_ms: u64,
    },
    /// The first Observe. Stage 0 starts from this header.
    /// `header_time_unix_s` is Bitcoin `nTime`. Zero means it was not read.
    SeedBeacon {
        height: u64,
        natural_hash: [u8; 32],
        header_time_unix_s: u32,
        at_ms: u64,
    },
    /// Terrain is filled once the first leg is stored. No byte counter, no percentage.
    TerrainFilled {
        at_ms: u64,
    },
    /// A leg's checkpoint is stored. `j` is 0-based; `stored_in_stage` is `j + 1`.
    LegRecorded {
        stage: usize,
        j: u64,
        stored_in_stage: u64,
        at_ms: u64,
    },
    /// A header newer than the last one seen. `natural_hash` is natural byte order.
    ///
    /// `peers` is the handshake-complete count when the session has published
    /// one. `None` means the count is not known yet. It is not zero.
    Tip {
        height: u64,
        natural_hash: [u8; 32],
        peers: Option<u32>,
        header_seen_unix_ms: u64,
    },
    /// Handshake-complete peers (`peers`) and sockets still in the pool
    /// (`sockets`), including those still in `version`/`verack`. `addrs` names
    /// the handshake-complete peers when the pipe has an address.
    ///
    /// A `peers` of zero is a real empty handshake set. The panel still says
    /// "connecting…" while `sockets` is non-zero and no tip has been seen.
    Gossip {
        peers: u32,
        sockets: u32,
        addrs: Vec<String>,
    },
    /// The gossip thread ended without Stop. Its sockets are gone.
    TipFeedStopped,
    StageClosed {
        stage: usize,
        legs: u64,
        interrupt_height: u64,
    },
    /// `W_g` POST. `appended` is a success status. `status` is absent when the POST did not connect.
    WaypointPosted {
        g: u64,
        appended: bool,
        status: Option<u16>,
    },
    /// One path GET for `W_g`, at the tip height when it returned.
    PathAsk {
        g: u64,
        at_height: u64,
        outcome: AskOutcome,
    },
    /// Settle runs: every leg is stored, the remaining asks are being made.
    Settling,
    /// The final stderr step (Rule A3.4): `path_skip g` and `not fetched by design 17`.
    PathLines(Vec<String>),
    ObjectWritten {
        path: PathBuf,
        bytes: u64,
        sha256: [u8; 32],
        at_ms: u64,
    },
    /// One line for the log drawer. Already redacted.
    Log(String),
    /// Stop or quit. No object is written.
    Stopped,
    Failed {
        message: String,
    },
}
