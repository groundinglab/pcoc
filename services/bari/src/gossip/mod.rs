//! Live Bitcoin header source for `BARI_TIP_SOURCE=bitcoin-gossip`.
//!
//! The default release build links this module. `HeaderClock::start` on
//! mainnet enters it. The absent-source sentence is not a return of that arm.
//!
//! `rpc` is unchanged: it never constructs this session. The lab poll stays in
//! `main`.

mod chain;
mod recorded;
mod session;
mod wire;

use std::time::Duration;

use chain::{mine_header, synth_bits, Chain};
use recorded::{
    ANCHOR_HEADER, ANCHOR_HEIGHT, ANCHOR_TIME, GENESIS_TIME, HEADER_2015, HEADER_2016,
    HEADER_30240_TIME, HEADER_32255, HEADER_32256, HEADER_971713,
};
use session::{connect_mainnet, ScriptedPeer, Session};

use crate::header_source::{GossipNetwork, HeaderClock, HeaderError, TipTick};

/// What `bari --self-check` prints when the release binary took this arm.
pub const LIVE_ARM_NAME: &str = "bitcoin-gossip";

/// The sentence the pre-2.2 mainnet arm returned. The release probe fails
/// when `HeaderClock::start` still produces it.
pub const ABSENT_LIVE_SOURCE: &str = "live header source is not in this build";

pub fn release_self_check() -> Result<&'static str, String> {
    let clock = HeaderClock::start("self-check.invalid:8333", GossipNetwork::Mainnet);
    if clock.absent_live_source() {
        return Err(ABSENT_LIVE_SOURCE.to_owned());
    }
    if !clock.live_source_in_build() {
        return Err(ABSENT_LIVE_SOURCE.to_owned());
    }
    recorded_mainnet_checks()?;
    in_process_peer_checks()?;
    Ok(LIVE_ARM_NAME)
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
    let mut session = Session::from_scripted(chain, peers, Duration::from_secs(2))?;
    let tip = session.opening_tip()?;
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

pub fn open_pending(peer: &str) -> LiveClock {
    LiveClock {
        state: LiveState::Pending {
            peer: peer.to_owned(),
        },
    }
}

enum LiveState {
    Pending { peer: String },
    Running(Session),
}

pub struct LiveClock {
    state: LiveState,
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
}

fn fail(message: impl Into<String>) -> HeaderError {
    HeaderError {
        message: message.into(),
    }
}

impl LiveClock {
    fn ensure_running(&mut self) -> Result<(), HeaderError> {
        let peer = match &self.state {
            LiveState::Pending { peer } => peer.clone(),
            LiveState::Running(_) => return Ok(()),
        };
        let session = connect_mainnet(&peer).map_err(fail)?;
        self.state = LiveState::Running(session);
        Ok(())
    }
}
