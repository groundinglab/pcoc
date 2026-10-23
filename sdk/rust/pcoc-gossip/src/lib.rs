//! Bitcoin header gossip.
#![allow(
    clippy::manual_is_multiple_of,
    clippy::manual_find,
    clippy::too_many_arguments,
    clippy::collapsible_if,
    clippy::large_enum_variant
)]
//!
//! Bari and the live producer share this crate. The session keeps at least
//! two peers, swaps a silent or invalid peer, and fails a read closed when
//! the quorum bound elapses. The next read tries again. This crate does not
//! dial from a test unless that test is ignored.
#![deny(unsafe_code)]

mod chain;
mod recorded;
mod session;
mod wire;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use chain::{mine_header, synth_bits, Chain};
use recorded::{
    ANCHOR_HEADER, ANCHOR_HEIGHT, ANCHOR_TIME, GENESIS_TIME, HEADER_2015, HEADER_2016,
    HEADER_30240_TIME, HEADER_32255, HEADER_32256, HEADER_971713,
};
use session::{connect_mainnet, connect_mainnet_cancellable, ScriptedPeer, Session};

pub use recorded::MAINNET_DNS_SEEDS;

/// What `bari --self-check` prints when the release binary took this arm.
pub const LIVE_ARM_NAME: &str = "bitcoin-gossip";

/// The sentence the pre-2.2 mainnet arm returned. A release probe fails
/// when a clock still produces it.
pub const ABSENT_LIVE_SOURCE: &str = "live header source is not in this build";

/// One gossip tip. `height` is the block height. `n_time` is header seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TipTick {
    pub hash: [u8; 32],
    pub height: u64,
    pub previous: Option<[u8; 32]>,
    pub n_time: u32,
    /// The 80-byte header whose SHA-256d is `hash`.
    pub header: [u8; 80],
    /// `nTime` of the block at `height - 2015` when `height + 1` is a multiple
    /// of 2016. Zero otherwise. The live retarget reads only the boundary case.
    pub period_first_time: u32,
}

/// Why a header read stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderError {
    pub message: String,
}

enum LiveState {
    Pending {
        peer: String,
        cancel: Option<Arc<AtomicBool>>,
    },
    Running(Session),
}

/// Blocking header clock. The producer does not call this on the walk.
/// A background thread owns it and publishes the latest tip.
pub struct LiveClock {
    state: LiveState,
}

/// Store the peer and dial on the first wait. Bari uses this. Nothing cancels it.
pub fn open_pending(peer: &str) -> LiveClock {
    open_cancellable(peer, None)
}

/// Store the peer. `wait` returns an error whose message is `cancelled` once
/// `cancel` is set, between reads.
pub fn open_cancellable(peer: &str, cancel: Option<Arc<AtomicBool>>) -> LiveClock {
    LiveClock {
        state: LiveState::Pending {
            peer: peer.to_owned(),
            cancel,
        },
    }
}

impl LiveClock {
    pub fn wait(&mut self, after: Option<[u8; 32]>) -> Result<Vec<TipTick>, HeaderError> {
        self.ensure_running()?;
        let session = match &mut self.state {
            LiveState::Running(session) => session,
            LiveState::Pending { .. } => {
                return Err(fail("inconsistent chain"));
            }
        };
        if !session.is_open() {
            let tip = session.opening_tip().map_err(fail)?;
            if after.is_none() || after != Some(tip.hash) {
                return Ok(vec![tip]);
            }
        }
        let after = after.ok_or_else(|| fail("inconsistent chain"))?;
        session.wait_after(after).map_err(fail)
    }

    fn ensure_running(&mut self) -> Result<(), HeaderError> {
        let (peer, cancel) = match &self.state {
            LiveState::Pending { peer, cancel } => (peer.clone(), cancel.clone()),
            LiveState::Running(_) => return Ok(()),
        };
        let session = match cancel {
            Some(flag) => connect_mainnet_cancellable(&peer, flag).map_err(fail)?,
            None => connect_mainnet(&peer).map_err(fail)?,
        };
        self.state = LiveState::Running(session);
        Ok(())
    }
}

fn fail(message: impl Into<String>) -> HeaderError {
    HeaderError {
        message: message.into(),
    }
}

/// Recorded-header and in-process peer checks. No dial.
pub fn gossip_self_check() -> Result<(), String> {
    recorded_mainnet_checks()?;
    in_process_peer_checks()
}

fn recorded_mainnet_checks() -> Result<(), String> {
    let mut chain = Chain::open_mainnet(ANCHOR_HEADER, ANCHOR_HEIGHT, ANCHOR_TIME)
        .map_err(|fault| fault.message())?;
    chain
        .append(HEADER_971713)
        .map_err(|fault| fault.message())?;
    let mut bad = HEADER_971713;
    bad[79] ^= 0x01;
    match chain.append(bad) {
        Err(chain::Fault::BadPow) => {}
        Err(fault) => return Err(fault.message()),
        Ok(()) => return Err("bad proof of work".to_owned()),
    }
    let mut clamped =
        Chain::open_mainnet(HEADER_2015, 2015, GENESIS_TIME).map_err(|fault| fault.message())?;
    clamped
        .append(HEADER_2016)
        .map_err(|fault| fault.message())?;
    let mut retarget = Chain::open_mainnet(HEADER_32255, 32_255, HEADER_30240_TIME)
        .map_err(|fault| fault.message())?;
    retarget
        .append(HEADER_32256)
        .map_err(|fault| fault.message())?;
    if chain::header_bits(&HEADER_32256) == chain::header_bits(&HEADER_32255) {
        return Err("unexpected difficulty".to_owned());
    }
    Ok(())
}

fn in_process_peer_checks() -> Result<(), String> {
    let bits = synth_bits();
    let anchor = mine_header([0x21; 32], 1_700_000_000, bits).map_err(|fault| fault.message())?;
    let anchor_hash = chain::header_hash(&anchor);
    let chain = Chain::open_fixed(anchor, 10, bits).map_err(|fault| fault.message())?;
    let header = mine_header(anchor_hash, 1_700_000_050, bits).map_err(|fault| fault.message())?;
    let peers = vec![
        ScriptedPeer::new(anchor_hash, vec![header], true),
        ScriptedPeer::new(anchor_hash, vec![header], true),
    ];
    let mut live = Session::from_scripted(chain, peers, Duration::from_secs(2))?;
    let tip = live.opening_tip()?;
    if tip.hash != chain::header_hash(&header) {
        return Err("peer disagreement".to_owned());
    }
    let lone = Chain::open_fixed(anchor, 10, bits).map_err(|fault| fault.message())?;
    let mut lone_session = Session::from_scripted(
        lone,
        vec![ScriptedPeer::new(anchor_hash, vec![header], false)],
        Duration::from_millis(500),
    )?;
    match lone_session.opening_tip() {
        Err(message) if message == "too few peers" => Ok(()),
        Err(message) => Err(message),
        Ok(_) => Err("too few peers".to_owned()),
    }
}
