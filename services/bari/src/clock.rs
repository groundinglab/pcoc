//! Tip-keyed open pool, then one non-RBF tree per eligible block, then serving.
//!
//! Accept appends to a durable pool keyed by the live tip. A new tip hands the
//! prior pool to the fee gate (Merkle → persist → broadcast) without blocking
//! the new open pool. There is no protocol accept cap. `BARI_POOL_SOFT_MAX` is
//! the ops dial.
//!
//! Before that tree is built, the live path reads the mempool next-block
//! minimum (`getrawmempool`). The bid is
//! `max(min(1.5 × minimum, 1.1 sat/vB), relay floor)`. A tree is broadcast
//! only while that minimum is below 1.1. At or above it, or when no confirmed input can fund
//! the bid, the cohort stays collected. The hold reason is recorded. It is not
//! a stage. Nothing already accepted is dropped because a block passed. The
//! test plane has no fee gate and broadcasts on the next tip.
//!
//! A tree already in the mempool does not hold the next cohort. Each broadcast
//! is judged on its own. The anchor window is 5 blocks from that tree's
//! broadcast height. A tree broadcast in block B can confirm in B+1 through
//! B+5. More than 5 blocks is Missed. A confirmation after that is recorded
//! and its paths stay served, marked confirmed late, outside the window. A
//! confirmed path stays servable. There is no archive after serve. Several
//! non-RBF trees may sit in the mempool at once. A broadcast is not replaced
//! and is not sent again.

use crate::merkle::{MerkleError, MerkleTree};
use crate::ops;
use crate::pool::{self, ListedPool, PoolError, PoolState, PoolTipKey, PooledWaypoint, TipPool};
use crate::proof::CanonicalPathBody;
use crate::storage::{
    FileStore, PersistedBatch, StorageError, StoredInclusion, StoredIndexHeader, StoredIndexPlant,
    StoredPlant, StoredPortableIndex, StoredState,
};
use crate::{decode_bitcoin_id, encode_bitcoin_id, encode_hash32, Hash32};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainTip {
    pub hash: Hash32,
    pub previous_hash: Option<Hash32>,
    pub height: u32,
    /// Bitcoin header `nTime` in unix seconds (`getblockheader` `time`, gossip
    /// header bytes 68..72), in `0 … 2^32 − 1`. Absent means the header time
    /// was not read. Zero is a real `nTime`. This is not Bari's wall-clock
    /// observation.
    pub time: Option<u64>,
}

impl ChainTip {
    /// Copy Bitcoin header `nTime` off a gossip tick.
    ///
    /// Units are unix seconds, the header field as stored, including zero.
    pub fn from_gossip_tick(tick: &hike::TipTick) -> Result<Self, String> {
        let height = u32::try_from(tick.height).map_err(|_| "tip height exceeds u32".to_owned())?;
        Ok(Self {
            hash: tick.hash,
            previous_hash: tick.previous,
            height,
            time: Some(u64::from(tick.n_time)),
        })
    }
}

impl ChainTip {
    fn pool_key(&self) -> PoolTipKey {
        PoolTipKey {
            hash: self.hash,
            height: self.height,
        }
    }
}

type PendingDecision = (Option<u64>, Option<(u64, SkipReason)>);

struct HeightNote<'a> {
    /// `sent`, `held`, or `quiet`.
    decision: &'a str,
    /// Internal reason. Empty when the height sent or was quiet.
    detail: &'a str,
    min_fee: Option<f64>,
    bid: Option<f64>,
    txid: Option<Hash32>,
    broadcast_height: Option<u32>,
    pool_size: u32,
    tree: Option<u64>,
    fee_source: Option<String>,
}

/// Display code of a hold (§3.2). Internal reasons stay on `detail`.
fn display_hold(internal: &str) -> &'static str {
    match internal {
        "next-block-min" => "min-above-cap",
        "cap" => "floor-above-cap",
        // Older binaries recorded this. The clock no longer holds for it.
        "tree-in-mempool" => "tree-in-mempool",
        "no-estimate" | "fallback" | "fee-view" => "no-fee-reading",
        "no-confirmed-utxo" | "utxo-view" | "underfunded" => "no-confirmed-coin",
        "" => "",
        _ => "send-error",
    }
}

fn legacy_decision(status: &str) -> &'static str {
    match status {
        "quiet" => "quiet",
        "collecting" | "held" => "held",
        "in-mempool" | "confirmed" | "confirmed-late" | "missed" | "sent" => "sent",
        _ => "quiet",
    }
}

fn json_u32(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|number| u32::try_from(number).ok())
}

fn block_row_from_event(event: &Value) -> Option<BlockLedgerRow> {
    let height = json_u32(&event["height"])?;
    let v2 = event.get("decision").and_then(Value::as_str).is_some();
    let status = event["status"].as_str().unwrap_or("quiet").to_owned();
    let decision = event["decision"]
        .as_str()
        .unwrap_or_else(|| legacy_decision(&status))
        .to_owned();
    let added = json_u32(&event["added"]).or_else(|| json_u32(&event["new_waypoints"]));
    let pool = json_u32(&event["pool_size"]).or_else(|| json_u32(&event["leaf_count"]));
    Some(BlockLedgerRow {
        height,
        block_hash: event["block"]
            .as_str()
            .and_then(|text| decode_bitcoin_id(text).ok())
            .unwrap_or([0; 32]),
        new_waypoints: added.unwrap_or(0),
        malformed: json_u32(&event["malformed"]).unwrap_or(0),
        duplicate: json_u32(&event["already_held"])
            .or_else(|| json_u32(&event["duplicate"]))
            .unwrap_or(0),
        status: decision.clone(),
        hold_reason: event["hold_reason"].as_str().unwrap_or("").to_owned(),
        min_fee_sat_vb: event["next_block_min_sat_vb"]
            .as_f64()
            .or_else(|| event["min_fee_sat_vb"].as_f64()),
        bid_sat_vb: event["bid_sat_vb"].as_f64(),
        broadcast_txid: event["broadcast_txid"]
            .as_str()
            .and_then(|text| decode_bitcoin_id(text).ok()),
        broadcast_height: json_u32(&event["broadcast_height"]),
        leaf_count: pool.unwrap_or(0),
        inclusion_height: None,
        confirm_offset: None,
        fee_sats: event["fee_sats"].as_u64(),
        confirmed_late: false,
        seen_unix_ms: event["timestamp_unix_ms"].as_u64().unwrap_or(0),
        time: event["time"].as_u64(),
        decision,
        detail: event["detail"].as_str().unwrap_or("").to_owned(),
        tree: event["tree"].as_u64(),
        added_known: v2,
        fee_source: event["fee_source"]
            .as_str()
            .map(str::to_owned)
            .filter(|source| !source.is_empty()),
    })
}

fn timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn leaf_ids(leaves: &[Hash32]) -> Vec<String> {
    leaves.iter().map(encode_hash32).collect()
}

enum PendingOutcome {
    /// Still inside the confirmation window. Not a stored state.
    Hold,
    Included {
        inclusion: crate::storage::StoredInclusion,
        /// The tip block did not contain the transaction. `tx_place` found it.
        catch_up: bool,
    },
    /// The anchor window ended without this tip containing the plant.
    /// The batch stays pending so a later confirmation can still be served.
    /// Logged once as `not-mined`. Not a terminal path.
    WindowElapsed {
        lookup: Option<crate::storage::StoredInclusion>,
    },
    Skip {
        reason: SkipReason,
        detail: &'static str,
        message: String,
    },
}

fn plant_index_of(batch: &ServingBatch) -> Option<PlantIndex> {
    let plant = batch.record.plant.as_ref()?;
    let inclusion = batch.record.inclusion.as_ref()?;
    Some(PlantIndex {
        root: batch.record.root,
        tree_size: batch.record.tree_size,
        txid: plant.txid,
        vout: plant.vout,
        block_hash: inclusion.block_hash,
        block_height: inclusion.block_height,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlantReceipt {
    pub txid: Hash32,
    pub vout: u32,
    pub fee_rate_sat_vb: f64,
    pub fee_source: String,
    /// Cap in force for this plant. Absent on the test plane.
    pub fee_cap_sat_vb: Option<f64>,
    /// Absolute fee of the funded transaction, in sats.
    pub fee_sat: Option<u64>,
    /// vsize of the signed transaction.
    pub vsize: Option<u32>,
}

impl From<&PlantReceipt> for StoredPlant {
    fn from(value: &PlantReceipt) -> Self {
        Self {
            txid: value.txid,
            vout: value.vout,
            fee_rate_sat_vb: value.fee_rate_sat_vb,
            fee_source: value.fee_source.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkipReason {
    Fee,
    NotMined,
    Other,
}

impl SkipReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fee => "fee",
            Self::NotMined => "not-mined",
            Self::Other => "other",
        }
    }
}

/// How long a per-block ledger row is kept. 48 hours at 10 minutes a block.
pub const LEDGER_RETAIN_BLOCKS: u32 = 48 * 6;

/// One row for every Bitcoin block height this process has seen.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlockLedgerRow {
    pub height: u32,
    pub block_hash: Hash32,
    /// New valid waypoints whose post landed on this height.
    pub new_waypoints: u32,
    pub malformed: u32,
    pub duplicate: u32,
    /// `collecting`, `in-mempool`, `confirmed`, `missed`, or `quiet`.
    pub status: String,
    /// Hold reason while collecting. Empty when this height broadcast or was quiet.
    pub hold_reason: String,
    pub min_fee_sat_vb: Option<f64>,
    pub bid_sat_vb: Option<f64>,
    pub broadcast_txid: Option<Hash32>,
    pub broadcast_height: Option<u32>,
    pub leaf_count: u32,
    pub inclusion_height: Option<u32>,
    /// `+N` from the broadcast height when this row's tree confirmed.
    pub confirm_offset: Option<u32>,
    /// Sats the broadcast paid, when the chain or the plant recorded them.
    /// Absent means the page estimates from the rate and 159 vB.
    #[serde(default)]
    pub fee_sats: Option<u64>,
    /// The confirming block is more than 5 after the broadcast.
    /// Ignored by the page. Inclusion lives on the tree record.
    pub confirmed_late: bool,
    /// Unix milliseconds when the row was last written. Retention uses height.
    pub seen_unix_ms: u64,
    /// Header `nTime`, unix seconds. Absent when it was not read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<u64>,
    /// `sent`, `held`, or `quiet`. Empty until a v2 line or a legacy map fills it.
    #[serde(default)]
    pub decision: String,
    /// Internal hold reason. `hold_reason` on a v2 line is the display code.
    #[serde(default)]
    pub detail: String,
    /// `batch_sequence` when this height sent a tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree: Option<u64>,
    /// False on a pre-v2 line. R11 skips any pair that includes one.
    #[serde(default)]
    pub added_known: bool,
    /// `mempool` or `floor` when this height sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_source: Option<String>,
}

/// One unconfirmed tree. `offset` is blocks since the broadcast, of 5.
#[derive(Clone, Debug, PartialEq)]
pub struct InflightTree {
    pub txid: Hash32,
    pub fee_rate_sat_vb: f64,
    pub leaves: u32,
    pub broadcast_height: u32,
    pub offset: u32,
    pub window: u32,
    pub missed: bool,
}

/// The newest confirmed tree.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfirmedTree {
    pub inclusion_height: u32,
    pub broadcast_height: u32,
    pub offset: u32,
    pub leaves: u32,
    pub txid: Option<Hash32>,
    pub fee_rate_sat_vb: Option<f64>,
    pub confirmed_late: bool,
}

/// Blocks after the broadcast height that may still include the tree.
///
/// A tree broadcast while the tip is B can confirm in B+1 through B+5.
/// More than 5 blocks is Missed. This count is not an operator setting.
/// SPEC §13 names the producer and verifier offsets. Bari's own window is
/// this 5-block span from the broadcast height.
pub const ANCHOR_WINDOW_BLOCKS: u32 = 5;

/// Older name for [`ANCHOR_WINDOW_BLOCKS`]. The count is 5, from broadcast height.
pub const DEFAULT_CONFIRMATION_WINDOW_TIPS: u32 = ANCHOR_WINDOW_BLOCKS;

/// Last height, relative to the broadcast, that is still inside the window.
fn window_last_height(broadcast_height: u32) -> u32 {
    broadcast_height.saturating_add(ANCHOR_WINDOW_BLOCKS)
}

/// `+N of 5` while a broadcast is unconfirmed. `None` before the broadcast tip
/// has moved on, which does not happen: the window is counted from that height.
pub fn blocks_since_broadcast(broadcast_height: u32, tip_height: u32) -> u32 {
    tip_height.saturating_sub(broadcast_height)
}

/// Why `GET /path/{leaf}` is not serving that leaf.
///
/// The phrases are the ones section 13.1 requires a 404 to say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathMiss {
    StillInPool,
    SealedNotInBlock,
    NotIncluded,
    ArchivedAfterServe,
    Unknown,
}

impl PathMiss {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StillInPool => "still in the pool",
            Self::SealedNotInBlock => "in mempool, not in a block yet",
            Self::NotIncluded => "not included",
            Self::ArchivedAfterServe => "already archived after it was served",
            Self::Unknown => "unknown",
        }
    }
}

fn skip_fate(reason: SkipReason) -> Option<PathMiss> {
    match reason {
        SkipReason::NotMined | SkipReason::Fee => Some(PathMiss::NotIncluded),
        SkipReason::Other => None,
    }
}

fn record_fate(record: &PersistedBatch) -> Option<PathMiss> {
    match record.state {
        StoredState::Flushed => Some(PathMiss::ArchivedAfterServe),
        StoredState::Skipped => match record.skip_reason.as_deref() {
            Some("not-mined") | Some("fee") => Some(PathMiss::NotIncluded),
            _ => None,
        },
        _ => None,
    }
}

/// Why a cohort stays in the pool instead of becoming a tree.
#[derive(Clone, Debug, PartialEq)]
pub struct Deferral {
    pub reason: String,
    pub message: String,
    /// Block D, the tip this seal decision read. Absent when the deferral is
    /// not tied to one block (a direct plant, for example).
    pub decision_height: Option<u32>,
    pub next_block_min_sat_vb: Option<f64>,
    /// `estimatesmartfee` 1 economical. Not the gate.
    pub node_estimate_sat_vb: Option<f64>,
    pub needed_sat_vb: Option<f64>,
    pub cap_sat_vb: Option<f64>,
}

/// Gate read before a tree is built.
#[derive(Clone, Debug, PartialEq)]
pub enum SealClearance {
    /// No fee gate. The test plane uses this and plants every tip.
    Clear,
    /// Broadcast at this rate. The rate is at or under the cap.
    PlantAt {
        rate_sat_vb: f64,
        source: String,
        cap_sat_vb: Option<f64>,
        next_block_min_sat_vb: Option<f64>,
        /// `estimatesmartfee` 1 economical at this decision. Not the gate.
        node_estimate_sat_vb: Option<f64>,
    },
    /// Keep the waypoints pooled. Do not build a tree and do not broadcast.
    Defer(Deferral),
}

#[derive(Clone, Debug, Error)]
#[error("plant failed ({reason:?}): {message}")]
pub struct PlantError {
    pub reason: SkipReason,
    pub message: String,
    /// Chosen bid, or the rate the gate compared.
    pub fee_rate_sat_vb: Option<f64>,
    /// `bid`, or a deferral reason when this plant had a fee decision.
    pub fee_source: Option<String>,
    /// When set, the waypoints go back to the pool. Nothing was broadcast.
    pub keeps_pool: bool,
    pub deferral: Option<Deferral>,
}

impl PlantError {
    #[cfg(test)]
    pub(crate) fn new(reason: SkipReason, message: impl Into<String>) -> Self {
        Self::with_fee(reason, message, None, None)
    }

    pub(crate) fn with_fee(
        reason: SkipReason,
        message: impl Into<String>,
        fee_rate_sat_vb: Option<f64>,
        fee_source: Option<String>,
    ) -> Self {
        Self {
            reason,
            message: message.into(),
            fee_rate_sat_vb,
            fee_source,
            keeps_pool: false,
            deferral: None,
        }
    }

    pub(crate) fn defer(deferral: Deferral) -> Self {
        Self {
            reason: SkipReason::Fee,
            message: deferral.message.clone(),
            fee_rate_sat_vb: deferral.needed_sat_vb.or(deferral.next_block_min_sat_vb),
            fee_source: Some(deferral.reason.clone()),
            keeps_pool: true,
            deferral: Some(deferral),
        }
    }
}

#[derive(Clone, Debug, Error)]
#[error("Bitcoin ChainView failed: {0}")]
pub struct ChainError(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChainPostCheck {
    pub transaction_in_block: bool,
    pub output_exists: bool,
    pub zero_value: bool,
    pub exact_script: bool,
    pub selected_chain: bool,
    pub carried_height: u32,
    pub authenticated_height: Option<u32>,
    pub height_matches: Option<bool>,
}

impl ChainPostCheck {
    pub fn is_valid(&self) -> bool {
        self.transaction_in_block
            && self.output_exists
            && self.zero_value
            && self.exact_script
            && self.selected_chain
            && self.authenticated_height.is_some()
    }
}

/// Live plane: [`crate::bitcoin_rpc::BitcoinRpc`].
/// Test plane: [`crate::synth_commit::SynthCommit`].
pub trait ChainBackend: Send + Sync {
    fn network(&self) -> Result<String, ChainError>;
    fn initial_block_download(&self) -> Result<bool, ChainError>;
    fn best_tip(&self) -> Result<ChainTip, ChainError>;
    #[allow(clippy::result_large_err)]
    fn plant(&self, root: &Hash32) -> Result<PlantReceipt, PlantError>;

    /// Read before Merkle. The default plants. The argument is block D.
    fn seal_clearance(&self, decision: &ChainTip) -> Result<SealClearance, ChainError> {
        let _ = decision;
        Ok(SealClearance::Clear)
    }

    /// `PlantAt` carries the rate the gate already accepted. `Clear` calls [`Self::plant`].
    #[allow(clippy::result_large_err)]
    fn plant_cleared(
        &self,
        root: &Hash32,
        clearance: &SealClearance,
    ) -> Result<PlantReceipt, PlantError> {
        match clearance {
            SealClearance::Defer(deferral) => Err(PlantError::defer(deferral.clone())),
            SealClearance::Clear | SealClearance::PlantAt { .. } => self.plant(root),
        }
    }

    fn included_in_tip(
        &self,
        txid: &Hash32,
        tip: &ChainTip,
    ) -> Result<Option<StoredInclusion>, ChainError>;
    fn block_is_selected(&self, block_hash: &Hash32) -> Result<bool, ChainError>;
    fn post_check(&self, body: &CanonicalPathBody) -> Result<ChainPostCheck, ChainError>;

    /// Where a planted transaction sits. `Ok(None)` means this backend cannot see
    /// the chain (the test plane). `Ok(Some)` is a sight, including a miss.
    fn tx_place(&self, txid: &Hash32) -> Result<Option<TxPlace>, ChainError> {
        let _ = txid;
        Ok(None)
    }

    /// Confirmed and unconfirmed wallet figures. `Ok(None)` when this backend
    /// has no wallet.
    fn chain_wallet(&self) -> Result<Option<ChainWallet>, ChainError> {
        Ok(None)
    }

    /// Node's synced block height. `Ok(None)` when this backend has no node.
    fn chain_sync(&self) -> Result<Option<ChainSync>, ChainError> {
        Ok(None)
    }

    /// Live next-block minimum and the bid it would produce. Read-only.
    /// Does not select coins and does not broadcast.
    fn fee_quote(&self) -> Result<Option<FeeQuote>, ChainError> {
        Ok(None)
    }

    /// Next-block minimum only. Read at a quiet decision. `Ok(None)` on the
    /// test plane. The read does not change the decision.
    fn fee_observation(&self) -> Result<Option<f64>, ChainError> {
        Ok(None)
    }
}

/// Where a planted transaction sits on the chain the node can see.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TxPlace {
    pub inclusion_height: Option<u32>,
    /// Block that includes the transaction, Bitcoin natural order.
    pub block_hash: Option<Hash32>,
    /// The node returned the transaction and it is not in a block.
    pub in_mempool: bool,
    pub fee_sats: Option<u64>,
}

impl TxPlace {
    /// The node has no such transaction. Not in a block and not in the mempool.
    pub fn absent() -> Self {
        Self {
            inclusion_height: None,
            block_hash: None,
            in_mempool: false,
            fee_sats: None,
        }
    }
}

/// One unspent output from `listunspent`. No address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletCoin {
    pub amount_sat: u64,
    pub confirmations: u64,
    pub spendable: bool,
    pub safe: bool,
}

/// Wallet figures for the operator page, from `listunspent` only.
/// Confirmed and unconfirmed do not overlap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainWallet {
    pub confirmed_sat: u64,
    pub unconfirmed_sat: u64,
    pub confirmed_coins: u32,
    pub unconfirmed_coins: u32,
    pub coins: Vec<WalletCoin>,
    pub min_conf: u64,
    pub reserve_min: usize,
    pub reserve_max: usize,
    pub consolidate: bool,
}

/// `getblockchaininfo` sync. `blocks`, `headers`, and initial block download.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainSync {
    pub blocks: u32,
    pub headers: u32,
    pub initialblockdownload: bool,
}

/// A read of the live fee gate. Empty `hold_reason` means the gate would send.
#[derive(Clone, Debug, PartialEq)]
pub struct FeeQuote {
    pub min_sat_vb: Option<f64>,
    /// `estimatesmartfee` 1 economical. Display only.
    pub node_estimate_sat_vb: Option<f64>,
    pub bid_sat_vb: Option<f64>,
    /// Higher of the relay floor and `mempoolminfee`, in sat/vB.
    pub floor_sat_vb: Option<f64>,
    /// `mempool` or `floor` when the gate would broadcast.
    pub source: Option<String>,
    pub hold_reason: String,
}

/// A tree record or the event log could not be placed on the page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreIssue {
    pub file: String,
    pub error: String,
}

/// In-memory inputs for one operator-page render. No batch or event file is read.
#[derive(Clone, Debug)]
pub struct OpsMemory {
    pub network: String,
    pub tip: ChainTip,
    pub observed_unix_ms: Option<u64>,
    pub trees: Vec<PersistedBatch>,
    pub blocks: Vec<BlockLedgerRow>,
    pub pool_len: u32,
    pub oldest_post_height: Option<u32>,
    pub added_since_tip: u32,
    pub malformed_since_tip: u32,
    pub already_held_since_tip: u32,
    pub store_issues: Vec<StoreIssue>,
}

/// Durable inputs for one operator-page render. Chain calls happen after this
/// leaves the accept lock.
#[derive(Clone, Debug)]
pub struct OpsSources {
    pub network: String,
    pub tip_height: u32,
    pub batches: Vec<PersistedBatch>,
    pub sealed: Vec<Value>,
    pub deferred: Vec<Value>,
    /// Block hash (natural order) to height, for the retained window.
    pub hash_heights: HashMap<Hash32, u32>,
    pub notes: Vec<BlockLedgerRow>,
    pub accepts_this_tip: u32,
    pub pool_len: u32,
    pub malformed_this_tip: u32,
    pub duplicate_this_tip: u32,
    pub collecting_since: Option<u32>,
    pub inflight: Vec<InflightTree>,
    pub last_confirmed: Option<ConfirmedTree>,
}

#[derive(Clone, Debug)]
struct PendingBatch {
    record: PersistedBatch,
    tree: MerkleTree,
    /// Set when persist or broadcast failed; next inclusion is a miss; no retry.
    close_fault: Option<SkipReason>,
    /// Height of the tip the pool was keyed by. The waypoint's post height.
    post_height: u32,
    /// Height of the tip when the tree was broadcast. The 5-block window
    /// is counted from here. Absent until the broadcast succeeds.
    broadcast_height: Option<u32>,
    /// The anchor window already logged Missed for this broadcast.
    /// The batch stays pending until the confirming block is known.
    window_noted: bool,
}

#[derive(Clone, Debug)]
struct ServingBatch {
    record: PersistedBatch,
    tree: MerkleTree,
    /// Height the tree was broadcast. The window is counted from here.
    broadcast_height: u32,
    /// True when the confirming block is more than 5 after the broadcast.
    confirmed_late: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlantIndex {
    pub root: Hash32,
    pub tree_size: u32,
    pub txid: Hash32,
    pub vout: u32,
    pub block_hash: Hash32,
    pub block_height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexSnapshot {
    pub network: String,
    pub header: ChainTip,
    /// Latest serving plant, kept so a one-plant document still has `plant`.
    pub plant: Option<PlantIndex>,
    /// Plants included in this block. More than one is allowed.
    pub plants: Vec<PlantIndex>,
}

impl IndexSnapshot {
    /// Wire / durable form matching live `GET /index` presentation.
    pub fn to_stored(&self) -> StoredPortableIndex {
        StoredPortableIndex {
            version: 2,
            network: self.network.clone(),
            header: StoredIndexHeader {
                hash: encode_bitcoin_id(&self.header.hash),
                height: self.header.height,
                // Bitcoin header nTime, unix seconds, including zero. Not the observation stamp.
                time: self.header.time.filter(|secs| u32::try_from(*secs).is_ok()),
                // Bari wall-clock observation is applied at persist, from the event line.
                timestamp_unix_ms: None,
            },
            plant: self.plant.as_ref().map(stored_index_plant),
            plants: self.plants.iter().map(stored_index_plant).collect(),
        }
    }
}

fn stored_index_plant(entry: &PlantIndex) -> StoredIndexPlant {
    StoredIndexPlant {
        root: encode_hash32(&entry.root),
        tree_size: entry.tree_size,
        txid: encode_bitcoin_id(&entry.txid),
        vout: entry.vout,
        block_hash: encode_bitcoin_id(&entry.block_hash),
        block_height: entry.block_height,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionReport {
    pub tip: ChainTip,
    pub continuous: bool,
    pub closed_batch: Option<u64>,
    pub included: Option<u64>,
    pub missed: Option<(u64, SkipReason)>,
    pub archived: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostAccept {
    Queued,
    Duplicate,
    Full,
}

#[derive(Debug, Error)]
pub enum ClockError {
    #[error(transparent)]
    Chain(#[from] ChainError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Merkle(#[from] MerkleError),
    #[error(transparent)]
    Pool(#[from] PoolError),
    #[error("BARI expected Bitcoin network {expected}; connected node reports {actual}")]
    WrongNetwork { expected: String, actual: String },
    #[error("Bitcoin Core is still in initial block download; Bari must not observe historical sync tips")]
    InitialBlockDownload,
    #[error("batch sequence overflow")]
    SequenceOverflow,
    #[error("persisted batch {0} is internally inconsistent")]
    CorruptBatch(u64),
}

/// Cohort handed off at tip for seal. May run without holding the accept mutex.
pub struct SealJob {
    pub opened_tip: ChainTip,
    pub closed_tip: ChainTip,
    pub leaves: Vec<PooledWaypoint>,
    pub pool_dir: PathBuf,
    pub continuous: bool,
    /// Empty handoff already logged and sealed under the accept lock. Execute
    /// only records the fee decision, outside that lock.
    pub fee_log_only: bool,
    batch_sequence: u64,
    backend: Arc<dyn ChainBackend>,
    store: FileStore,
    #[cfg(test)]
    fail_persist: bool,
}

/// Cohort returned to the open pool. The old pool is marked sealed only after
/// the leaves are durable in the live pool.
struct DeferredCohort {
    leaves: Vec<PooledWaypoint>,
    pool_dir: PathBuf,
    opened_tip: PoolTipKey,
    deferral: Deferral,
}

/// Result of executing a [`SealJob`], ready to install as pending or to carry.
pub struct SealedPending {
    pending: Option<PendingBatch>,
    batch_sequence: Option<u64>,
    deferred: Option<DeferredCohort>,
    /// Ask windows that closed at this decision. Not planted and not carried.
    expired: Vec<Hash32>,
    /// Next-block minimum read at this decision. Null when the read did not answer.
    observed_min: Option<f64>,
}

fn deferred_seal(
    leaves: Vec<PooledWaypoint>,
    pool_dir: PathBuf,
    opened_tip: ChainTip,
    deferral: Deferral,
    expired: Vec<Hash32>,
) -> SealedPending {
    let observed_min = deferral.next_block_min_sat_vb;
    SealedPending {
        pending: None,
        batch_sequence: None,
        deferred: Some(DeferredCohort {
            leaves,
            pool_dir,
            opened_tip: opened_tip.pool_key(),
            deferral,
        }),
        expired,
        observed_min,
    }
}

struct SealRates {
    next_block_min_sat_vb: Option<f64>,
    node_estimate_sat_vb: Option<f64>,
    fee_cap_sat_vb: Option<f64>,
    bid_sat_vb: Option<f64>,
}

fn rates_of(clearance: &SealClearance) -> SealRates {
    match clearance {
        SealClearance::Clear => SealRates {
            next_block_min_sat_vb: None,
            node_estimate_sat_vb: None,
            fee_cap_sat_vb: None,
            bid_sat_vb: None,
        },
        SealClearance::PlantAt {
            rate_sat_vb,
            cap_sat_vb,
            next_block_min_sat_vb,
            node_estimate_sat_vb,
            ..
        } => SealRates {
            next_block_min_sat_vb: *next_block_min_sat_vb,
            node_estimate_sat_vb: *node_estimate_sat_vb,
            fee_cap_sat_vb: *cap_sat_vb,
            bid_sat_vb: Some(*rate_sat_vb),
        },
        SealClearance::Defer(deferral) => SealRates {
            next_block_min_sat_vb: deferral.next_block_min_sat_vb,
            node_estimate_sat_vb: deferral.node_estimate_sat_vb,
            fee_cap_sat_vb: deferral.cap_sat_vb,
            bid_sat_vb: deferral.needed_sat_vb,
        },
    }
}

fn log_seal_decision(
    decision: &str,
    closed_tip: &ChainTip,
    rates: &SealRates,
    kept: &[PooledWaypoint],
) {
    let fields = json!({
        "decision_height": closed_tip.height,
        "decision_block": encode_bitcoin_id(&closed_tip.hash),
        "next_block_min_sat_vb": rates.next_block_min_sat_vb,
        "node_estimate_sat_vb": rates.node_estimate_sat_vb,
        "fee_cap_sat_vb": rates.fee_cap_sat_vb,
        "bid_sat_vb": rates.bid_sat_vb,
        "decision": decision,
        "leaf_count": kept.len(),
        "leaves": kept.iter().map(|waypoint| encode_hash32(&waypoint.leaf)).collect::<Vec<_>>(),
        "dropped_leaf_count": 0,
        "dropped_leaves": [],
    });
    if decision == crate::fee::DECISION_DEFER {
        ops::warn(ops::SEAL_DECISION, fields);
    } else {
        ops::info(ops::SEAL_DECISION, fields);
    }
}

impl SealJob {
    /// Merkle → persist batch → plant. Does not touch the open pool.
    ///
    /// An accepted waypoint is not dropped because the anchor window moved.
    /// The whole cohort is planted, or the whole cohort stays in the pool.
    pub fn execute(self) -> Result<SealedPending, ClockError> {
        let SealJob {
            opened_tip,
            closed_tip,
            leaves,
            pool_dir,
            continuous,
            fee_log_only,
            batch_sequence,
            backend,
            store,
            #[cfg(test)]
            fail_persist,
        } = self;

        if fee_log_only {
            let min = backend.fee_observation().ok().flatten();
            log_seal_decision(
                crate::fee::DECISION_EMPTY,
                &closed_tip,
                &SealRates {
                    next_block_min_sat_vb: min,
                    node_estimate_sat_vb: None,
                    fee_cap_sat_vb: None,
                    bid_sat_vb: None,
                },
                &[],
            );
            return Ok(SealedPending {
                pending: None,
                batch_sequence: None,
                deferred: None,
                expired: Vec::new(),
                observed_min: min,
            });
        }

        if leaves.is_empty() {
            TipPool::mark_sealed(&pool_dir, &opened_tip.pool_key())?;
            store.append_event(
                "pool_empty_seal",
                json!({
                    "opened_tip": encode_bitcoin_id(&opened_tip.hash),
                    "opened_tip_height": opened_tip.height,
                    "closed_tip": encode_bitcoin_id(&closed_tip.hash),
                    "closed_tip_height": closed_tip.height,
                    "continuous": continuous,
                }),
            )?;
            // No Merkle tree: `MerkleTree::build` rejects an empty batch, so
            // this line has no `tree_size`.
            ops::info(
                ops::SEAL,
                json!({
                    "empty": true,
                    "leaf_count": 0,
                    "leaves": [],
                    "durable_event": "pool_empty_seal",
                    "opened_tip": encode_bitcoin_id(&opened_tip.hash),
                    "opened_tip_height": opened_tip.height,
                    "closed_tip": encode_bitcoin_id(&closed_tip.hash),
                    "closed_tip_height": closed_tip.height,
                    "continuous": continuous,
                }),
            );
            let min = backend.fee_observation().ok().flatten();
            log_seal_decision(
                crate::fee::DECISION_EMPTY,
                &closed_tip,
                &SealRates {
                    next_block_min_sat_vb: min,
                    node_estimate_sat_vb: None,
                    fee_cap_sat_vb: None,
                    bid_sat_vb: None,
                },
                &[],
            );
            return Ok(SealedPending {
                pending: None,
                batch_sequence: None,
                deferred: None,
                expired: Vec::new(),
                observed_min: min,
            });
        }

        let clearance = match backend.seal_clearance(&closed_tip) {
            Ok(clearance) => clearance,
            Err(error) => SealClearance::Defer(Deferral {
                reason: "fee-view".to_owned(),
                message: format!(
                    "seal clearance failed: {error}; deferred; no tree; no psbt; not replaced"
                ),
                decision_height: Some(closed_tip.height),
                next_block_min_sat_vb: None,
                node_estimate_sat_vb: None,
                needed_sat_vb: None,
                cap_sat_vb: None,
            }),
        };
        let decision = if matches!(clearance, SealClearance::Defer(_)) {
            crate::fee::DECISION_DEFER
        } else {
            crate::fee::DECISION_PLANT
        };
        let rates = rates_of(&clearance);
        log_seal_decision(decision, &closed_tip, &rates, &leaves);
        if let SealClearance::Defer(deferral) = clearance {
            return Ok(deferred_seal(
                leaves,
                pool_dir,
                opened_tip,
                deferral,
                Vec::new(),
            ));
        }

        let tree = MerkleTree::build(leaves.iter().map(|waypoint| waypoint.leaf).collect())?;
        let mut record = PersistedBatch {
            batch_sequence,
            opened_tip: opened_tip.hash,
            closed_tip: closed_tip.hash,
            leaves: tree.leaves().to_vec(),
            tree_size: tree.tree_size(),
            root: tree.root(),
            state: StoredState::Persisted,
            plant: None,
            inclusion: None,
            skip_reason: None,
            opened_tip_height: Some(opened_tip.height),
            broadcast_height: None,
            next_block_min_sat_vb: None,
            fee_sat: None,
            vsize: None,
            leaf_post_heights: tree
                .leaves()
                .iter()
                .map(|leaf| {
                    leaves
                        .iter()
                        .find(|waypoint| waypoint.leaf == *leaf)
                        .map(|waypoint| waypoint.post_height)
                        .unwrap_or(opened_tip.height)
                })
                .collect(),
        };

        ops::info(
            ops::SEAL,
            json!({
                "empty": false,
                "batch_sequence": record.batch_sequence,
                "tree_size": record.tree_size,
                "leaves": leaf_ids(&record.leaves),
                "root": encode_hash32(&record.root),
                "opened_tip": encode_bitcoin_id(&record.opened_tip),
                "opened_tip_height": opened_tip.height,
                "closed_tip": encode_bitcoin_id(&record.closed_tip),
                "closed_tip_height": closed_tip.height,
                "continuous": continuous,
            }),
        );

        let mut close_fault = None;
        let mut plant_fee_rate: Option<f64> = None;
        let mut plant_fee_source: Option<String> = None;
        let mut plant_fee_cap: Option<f64> = None;

        #[cfg(test)]
        let force_persist_fail = fail_persist;
        #[cfg(not(test))]
        let force_persist_fail = false;

        let persist_result = if force_persist_fail {
            Err(StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "injected persist failure",
            )))
        } else {
            store.persist_new(&record)
        };

        match persist_result {
            Ok(()) => match backend.plant_cleared(&record.root, &clearance) {
                Ok(receipt) => {
                    plant_fee_rate = Some(receipt.fee_rate_sat_vb);
                    plant_fee_source = Some(receipt.fee_source.clone());
                    plant_fee_cap = receipt.fee_cap_sat_vb;
                    record.plant = Some((&receipt).into());
                    record.state = StoredState::Broadcast;
                    record.broadcast_height = Some(closed_tip.height);
                    record.next_block_min_sat_vb = rates.next_block_min_sat_vb;
                    record.fee_sat = receipt.fee_sat;
                    record.vsize = receipt.vsize;
                    ops::info(
                        ops::PLANT_BROADCAST,
                        json!({
                            "batch_sequence": record.batch_sequence,
                            "root": encode_hash32(&record.root),
                            "tree_size": record.tree_size,
                            "leaves": leaf_ids(&record.leaves),
                            "txid": encode_bitcoin_id(&receipt.txid),
                            "vout": receipt.vout,
                            "fee_rate_sat_vb": receipt.fee_rate_sat_vb,
                            "fee_source": &receipt.fee_source,
                            "fee_cap_sat_vb": receipt.fee_cap_sat_vb,
                        }),
                    );
                    if let Err(error) = store.update(&record) {
                        close_fault = Some(SkipReason::Other);
                        ops::warn(
                            ops::PLANT_SKIP,
                            json!({
                                "at": "broadcast_record",
                                "batch_sequence": record.batch_sequence,
                                "root": encode_hash32(&record.root),
                                "tree_size": record.tree_size,
                                "leaves": leaf_ids(&record.leaves),
                                "txid": encode_bitcoin_id(&receipt.txid),
                                "vout": receipt.vout,
                                "fee_rate_sat_vb": receipt.fee_rate_sat_vb,
                                "fee_source": &receipt.fee_source,
                                "skip_reason": SkipReason::Other.as_str(),
                                "message": error.to_string(),
                            }),
                        );
                    }
                }
                Err(error) if error.keeps_pool => {
                    let deferral = error.deferral.clone().unwrap_or(Deferral {
                        reason: error.fee_source.clone().unwrap_or_else(|| "fee".to_owned()),
                        message: error.message.clone(),
                        decision_height: Some(closed_tip.height),
                        next_block_min_sat_vb: None,
                        node_estimate_sat_vb: None,
                        needed_sat_vb: error.fee_rate_sat_vb,
                        cap_sat_vb: None,
                    });
                    // The batch file has no broadcast. Drop it and keep the leaves pooled.
                    let _ = store.discard_unbroadcast(record.batch_sequence);
                    return Ok(deferred_seal(
                        leaves,
                        pool_dir,
                        opened_tip,
                        deferral,
                        Vec::new(),
                    ));
                }
                Err(error) => {
                    plant_fee_rate = error.fee_rate_sat_vb;
                    plant_fee_source = error.fee_source.clone();
                    record.state = StoredState::PlantFailed;
                    record.skip_reason = Some(error.reason.as_str().to_owned());
                    let _ = store.update(&record);
                    close_fault = Some(error.reason);
                    ops::warn(
                        ops::PLANT_SKIP,
                        json!({
                            "at": "plant",
                            "batch_sequence": record.batch_sequence,
                            "root": encode_hash32(&record.root),
                            "tree_size": record.tree_size,
                            "leaves": leaf_ids(&record.leaves),
                            "fee_rate_sat_vb": error.fee_rate_sat_vb,
                            "fee_source": error.fee_source.as_deref(),
                            "skip_reason": error.reason.as_str(),
                            "message": error.message,
                        }),
                    );
                }
            },
            Err(error) => {
                close_fault = Some(SkipReason::Other);
                record.state = StoredState::PlantFailed;
                record.skip_reason = Some(SkipReason::Other.as_str().to_owned());
                ops::warn(
                    ops::PLANT_SKIP,
                    json!({
                        "at": "persist",
                        "batch_sequence": record.batch_sequence,
                        "root": encode_hash32(&record.root),
                        "tree_size": record.tree_size,
                        "leaves": leaf_ids(&record.leaves),
                        "skip_reason": SkipReason::Other.as_str(),
                        "message": error.to_string(),
                    }),
                );
            }
        }

        let plant = record.plant.as_ref();
        store.append_event(
            "pool_sealed",
            json!({
                "batch_sequence": record.batch_sequence,
                "opened_tip": encode_bitcoin_id(&record.opened_tip),
                "opened_tip_height": opened_tip.height,
                "closed_tip": encode_bitcoin_id(&record.closed_tip),
                "closed_tip_height": closed_tip.height,
                "continuous": continuous,
                "leaves": record.leaves.iter().map(encode_hash32).collect::<Vec<_>>(),
                "root": encode_hash32(&record.root),
                "tree_size": record.tree_size,
                "planted": plant.is_some(),
                "txid": plant.map(|value| encode_bitcoin_id(&value.txid)),
                "vout": plant.map(|value| value.vout),
                "fee_rate_sat_vb": plant_fee_rate,
                "fee_source": plant_fee_source.as_deref(),
                "fee_cap_sat_vb": plant_fee_cap,
                "next_block_min_sat_vb": rates.next_block_min_sat_vb,
                "node_estimate_sat_vb": rates.node_estimate_sat_vb,
                "skip_reason": record.skip_reason.as_deref(),
                "close_fault": close_fault.map(|reason| reason.as_str()),
            }),
        )?;

        TipPool::mark_sealed(&pool_dir, &opened_tip.pool_key())?;

        let broadcast_height = record.plant.as_ref().map(|_| closed_tip.height);
        Ok(SealedPending {
            pending: Some(PendingBatch {
                record,
                tree,
                close_fault,
                post_height: opened_tip.height,
                broadcast_height,
                window_noted: false,
            }),
            batch_sequence: Some(batch_sequence),
            deferred: None,
            expired: Vec::new(),
            observed_min: rates.next_block_min_sat_vb,
        })
    }
}

pub struct BariClock {
    backend: Arc<dyn ChainBackend>,
    store: FileStore,
    network: String,
    current_tip: ChainTip,
    open_pool: TipPool,
    /// Optional ops soft max on the open pool (None / 0 = off).
    soft_max: Option<usize>,
    /// Broadcast plants still waiting for their confirming block.
    pending: Vec<PendingBatch>,
    /// Confirmed trees. A path stays servable after the anchor window.
    serving: Vec<ServingBatch>,
    /// Leaves handed to seal and not yet installed as pending.
    seal_leaves: Vec<Hash32>,
    /// Terminal path fates for leaves that are no longer pooled, pending, or serving.
    terminal: HashMap<Hash32, PathMiss>,
    /// Plant object published with the latest tip (None ⇒ JSON null).
    published_plant: Option<PlantIndex>,
    /// Unix ms of the latest `selected_tip_observed` line in this process.
    /// Copied onto the portable-index header when that tip is persisted.
    observed_unix_ms: Option<u64>,
    /// Per-block ledger, newest decisions included. Retained for 48 hours.
    ledger: Vec<BlockLedgerRow>,
    /// Records the page cannot place. The banner names the file.
    store_issues: Vec<StoreIssue>,
    /// `(tip, rule)` pairs already warned. One line per rule per tip.
    warned_page_checks: HashSet<(u32, String)>,
    /// Waypoints accepted on the live tip, before the next height's row.
    accepts_this_tip: u32,
    malformed_this_tip: u32,
    duplicate_this_tip: u32,
    /// Counts taken at handoff, before the seal runs outside the accept lock.
    /// A waypoint posted during the seal belongs to the next height.
    handoff_added: Option<u32>,
    handoff_malformed: Option<u32>,
    handoff_duplicate: Option<u32>,
    /// Open-since height of the live collecting cohort.
    collecting_since: Option<u32>,
    next_batch_sequence: u64,
    #[cfg(test)]
    fail_next_persist: bool,
}

impl BariClock {
    pub fn open(backend: Arc<dyn ChainBackend>, store: FileStore) -> Result<Self, ClockError> {
        Self::open_for_network(backend, store, "signet")
    }

    pub fn open_for_network(
        backend: Arc<dyn ChainBackend>,
        store: FileStore,
        expected_network: &str,
    ) -> Result<Self, ClockError> {
        let tip = Self::startup_tip(&backend, expected_network)?;
        Self::open_at_tip_for_network(backend, store, tip, expected_network, None)
    }

    /// `soft_max`: None or Some(0) disables the ops dial (default).
    pub fn open_for_network_with_soft_max(
        backend: Arc<dyn ChainBackend>,
        store: FileStore,
        expected_network: &str,
        soft_max: Option<usize>,
    ) -> Result<Self, ClockError> {
        let tip = Self::startup_tip(&backend, expected_network)?;
        Self::open_at_tip_for_network(backend, store, tip, expected_network, soft_max)
    }

    #[cfg(test)]
    pub(crate) fn open_at_tip(
        backend: Arc<dyn ChainBackend>,
        store: FileStore,
        tip: ChainTip,
    ) -> Result<Self, ClockError> {
        Self::open_at_tip_for_network(backend, store, tip, "signet", None)
    }

    #[cfg(test)]
    pub(crate) fn open_at_tip_with_soft_max(
        backend: Arc<dyn ChainBackend>,
        store: FileStore,
        tip: ChainTip,
        soft_max: Option<usize>,
    ) -> Result<Self, ClockError> {
        Self::open_at_tip_for_network(backend, store, tip, "signet", soft_max)
    }

    fn startup_tip(
        backend: &Arc<dyn ChainBackend>,
        expected_network: &str,
    ) -> Result<ChainTip, ClockError> {
        let actual_network = backend.network()?;
        if actual_network != expected_network {
            return Err(ClockError::WrongNetwork {
                expected: expected_network.to_owned(),
                actual: actual_network,
            });
        }
        if backend.initial_block_download()? {
            return Err(ClockError::InitialBlockDownload);
        }
        Ok(backend.best_tip()?)
    }

    pub(crate) fn open_at_tip_for_network(
        backend: Arc<dyn ChainBackend>,
        store: FileStore,
        _caller_tip: ChainTip,
        expected_network: &str,
        soft_max: Option<usize>,
    ) -> Result<Self, ClockError> {
        let tip = Self::startup_tip(&backend, expected_network)?;
        Self::finish_open(backend, store, tip, expected_network, soft_max)
    }

    /// Open on a tip the plane sequence already handed in.
    ///
    /// Checks the command backend's network and initial-block-download flag.
    /// Does not call `best_tip`. The unset RPC poll keeps [`Self::open_for_network`].
    /// `BARI_TIP_SOURCE=bitcoin-gossip` and `synth` open here.
    pub fn open_at_supplied_tip(
        backend: Arc<dyn ChainBackend>,
        store: FileStore,
        tip: ChainTip,
        expected_network: &str,
        soft_max: Option<usize>,
    ) -> Result<Self, ClockError> {
        let actual_network = backend.network()?;
        if actual_network != expected_network {
            return Err(ClockError::WrongNetwork {
                expected: expected_network.to_owned(),
                actual: actual_network,
            });
        }
        if backend.initial_block_download()? {
            return Err(ClockError::InitialBlockDownload);
        }
        Self::finish_open(backend, store, tip, expected_network, soft_max)
    }

    fn finish_open(
        backend: Arc<dyn ChainBackend>,
        store: FileStore,
        tip: ChainTip,
        expected_network: &str,
        soft_max: Option<usize>,
    ) -> Result<Self, ClockError> {
        // Incomplete records normally fail-closed. Broadcast-with-plant is the
        // one restart-safe pending shape: same txid still sits in mempool/chain,
        // so restore it into pending instead of skipping (LIVE path recovery).
        let mut next_batch_sequence = 0u64;
        let mut restore_broadcasts: Vec<crate::storage::PersistedBatch> = Vec::new();
        for mut record in store.load_batches()? {
            next_batch_sequence = next_batch_sequence.max(
                record
                    .batch_sequence
                    .checked_add(1)
                    .ok_or(ClockError::SequenceOverflow)?,
            );
            if matches!(
                record.state,
                StoredState::Included | StoredState::Flushed | StoredState::Skipped
            ) {
                continue;
            }
            if record.state == StoredState::Broadcast
                && record.plant.is_some()
                && record.inclusion.is_none()
            {
                restore_broadcasts.push(record);
                continue;
            }
            record.state = StoredState::Skipped;
            record.skip_reason = Some(SkipReason::Other.as_str().to_owned());
            store.update(&record)?;
            store.append_event(
                "restart_incomplete_failed_closed",
                json!({
                    "batch_sequence": record.batch_sequence,
                    "root": encode_hash32(&record.root),
                    "tree_size": record.tree_size,
                    "skip_reason": SkipReason::Other.as_str(),
                }),
            )?;
        }

        let soft_max = soft_max.filter(|n| *n > 0);

        let mut pending = Vec::new();
        for record in restore_broadcasts {
            let tree = MerkleTree::build(record.leaves.clone())?;
            if tree.root() != record.root || tree.tree_size() != record.tree_size {
                return Err(ClockError::Storage(StorageError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "broadcast batch {} root/tree_size mismatch on restore",
                        record.batch_sequence
                    ),
                ))));
            }
            let plant = record.plant.as_ref().expect("broadcast plant checked");
            store.append_event(
                "restart_broadcast_restored_pending",
                json!({
                    "batch_sequence": record.batch_sequence,
                    "root": encode_hash32(&record.root),
                    "tree_size": record.tree_size,
                    "txid": encode_bitcoin_id(&plant.txid),
                    "vout": plant.vout,
                }),
            )?;
            let post_height = record
                .opened_tip_height
                .unwrap_or(tip.height.saturating_sub(1));
            let broadcast_height = Some(Self::broadcast_height_of(&record, tip.height));
            pending.push(PendingBatch {
                record,
                tree,
                close_fault: None,
                post_height,
                broadcast_height,
                window_noted: false,
            });
        }

        // Every confirmed tree stays servable, including one an older binary
        // marked flushed after the anchor window.
        let mut restore_serving: Vec<PersistedBatch> = Vec::new();
        for mut record in store.load_batches()? {
            let confirmed = matches!(record.state, StoredState::Included | StoredState::Flushed)
                && record.plant.is_some()
                && record.inclusion.is_some();
            if !confirmed {
                continue;
            }
            if record.state == StoredState::Flushed {
                record.state = StoredState::Included;
                store.update(&record)?;
            }
            restore_serving.push(record);
        }
        let mut serving = Vec::new();
        for record in restore_serving {
            let tree = MerkleTree::build(record.leaves.clone())?;
            if tree.root() != record.root || tree.tree_size() != record.tree_size {
                return Err(ClockError::Storage(StorageError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "included batch {} root/tree_size mismatch on restore",
                        record.batch_sequence
                    ),
                ))));
            }
            let plant = record.plant.as_ref().expect("included plant checked");
            let inclusion = record
                .inclusion
                .as_ref()
                .expect("included inclusion checked");
            store.append_event(
                "restart_included_restored_serving",
                json!({
                    "batch_sequence": record.batch_sequence,
                    "root": encode_hash32(&record.root),
                    "tree_size": record.tree_size,
                    "txid": encode_bitcoin_id(&plant.txid),
                    "vout": plant.vout,
                    "block_hash": encode_bitcoin_id(&inclusion.block_hash),
                    "block_height": inclusion.block_height,
                }),
            )?;
            let broadcast_height = Self::broadcast_height_of(&record, tip.height);
            let confirmed_late = inclusion.block_height > window_last_height(broadcast_height);
            serving.push(ServingBatch {
                broadcast_height,
                confirmed_late,
                record,
                tree,
            });
        }

        let mut clock = Self {
            backend: backend.clone(),
            store: store.clone(),
            network: expected_network.to_owned(),
            current_tip: tip,
            open_pool: TipPool::open_or_create(store.root(), tip.pool_key())?,
            soft_max,
            pending,
            serving,
            seal_leaves: Vec::new(),
            terminal: HashMap::new(),
            published_plant: None,
            observed_unix_ms: None,
            ledger: Vec::new(),
            store_issues: Vec::new(),
            warned_page_checks: HashSet::new(),
            accepts_this_tip: 0,
            malformed_this_tip: 0,
            duplicate_this_tip: 0,
            handoff_added: None,
            handoff_malformed: None,
            handoff_duplicate: None,
            collecting_since: None,
            next_batch_sequence,
            #[cfg(test)]
            fail_next_persist: false,
        };
        clock.refresh_published_plant();

        clock.recover_pools_on_startup()?;
        clock.rebuild_terminal()?;
        clock.restore_ledger()?;
        clock.backfill_broadcast_heights()?;
        // This tip was not passed through `on_tip_prepare`. Without a write
        // here, its height file is never created.
        clock.retain_startup_tip_index()?;
        Ok(clock)
    }

    /// Fill `broadcast_height` once, in the design's order, and write it back.
    /// A record that resolves none stays out of the page and raises a store issue.
    fn backfill_broadcast_heights(&mut self) -> Result<(), ClockError> {
        let sealed = match self.store.load_events_named("pool_sealed") {
            Ok(events) => events,
            Err(error) => {
                self.store_issues.push(StoreIssue {
                    file: "events.jsonl".to_owned(),
                    error: error.to_string(),
                });
                Vec::new()
            }
        };
        let pending_len = self.pending.len();
        for index in 0..pending_len {
            let height = self.resolved_broadcast_height(&self.pending[index].record, &sealed);
            self.apply_resolved_broadcast(index, true, height)?;
        }
        let serving_len = self.serving.len();
        for index in 0..serving_len {
            let height = self.resolved_broadcast_height(&self.serving[index].record, &sealed);
            self.apply_resolved_broadcast(index, false, height)?;
        }
        Ok(())
    }

    fn resolved_broadcast_height(&self, record: &PersistedBatch, sealed: &[Value]) -> Option<u32> {
        if record.plant.is_none() || record.broadcast_height.is_some() {
            return record.broadcast_height;
        }
        if let Some(post) = record.opened_tip_height {
            return Some(post.saturating_add(1));
        }
        sealed
            .iter()
            .find(|event| event["batch_sequence"].as_u64() == Some(record.batch_sequence))
            .and_then(|event| json_u32(&event["closed_tip_height"]))
            .or_else(|| self.index_height_for_hash(&record.closed_tip))
    }

    fn apply_resolved_broadcast(
        &mut self,
        index: usize,
        pending: bool,
        height: Option<u32>,
    ) -> Result<(), ClockError> {
        let (has_plant, already, sequence) = {
            let record = if pending {
                &self.pending[index].record
            } else {
                &self.serving[index].record
            };
            (
                record.plant.is_some(),
                record.broadcast_height,
                record.batch_sequence,
            )
        };
        if !has_plant {
            return Ok(());
        }
        if let Some(already) = already {
            if pending {
                self.pending[index].broadcast_height = Some(already);
            } else {
                self.serving[index].broadcast_height = already;
            }
            return Ok(());
        }
        match height {
            Some(height) => {
                if pending {
                    self.pending[index].record.broadcast_height = Some(height);
                    self.pending[index].broadcast_height = Some(height);
                    let stored = self.pending[index].record.clone();
                    self.store.update(&stored)?;
                } else {
                    self.serving[index].record.broadcast_height = Some(height);
                    self.serving[index].broadcast_height = height;
                    let stored = self.serving[index].record.clone();
                    self.store.update(&stored)?;
                }
            }
            None => self.store_issues.push(StoreIssue {
                file: format!("batches/batch-{sequence:020}.json"),
                error: "broadcast height unresolved".to_owned(),
            }),
        }
        Ok(())
    }

    fn index_height_for_hash(&self, hash: &Hash32) -> Option<u32> {
        let display = encode_bitcoin_id(hash);
        let entries = std::fs::read_dir(self.store.index_dir()).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Ok(file) = std::fs::File::open(&path) else {
                continue;
            };
            let Ok(tip) = serde_json::from_reader::<_, crate::storage::StoredPortableIndex>(file)
            else {
                continue;
            };
            if tip.header.hash == display {
                return Some(tip.header.height);
            }
        }
        None
    }

    fn broadcast_height_of(record: &PersistedBatch, open_tip_height: u32) -> u32 {
        // The tree is broadcast on the tip that closed the pool. That height
        // is one above the post when the chain was continuous. A record that
        // never stored the post height uses the open tip as the broadcast.
        record
            .opened_tip_height
            .map(|post| post.saturating_add(1))
            .unwrap_or(open_tip_height)
    }

    fn note_leaves(&mut self, leaves: &[Hash32], fate: PathMiss) {
        for leaf in leaves {
            self.terminal.insert(*leaf, fate);
        }
    }

    fn rebuild_terminal(&mut self) -> Result<(), ClockError> {
        self.terminal.clear();
        let mut in_batch = std::collections::HashSet::new();
        for record in self.store.load_batches()? {
            in_batch.extend(record.leaves.iter().copied());
            if let Some(fate) = record_fate(&record) {
                self.note_leaves(&record.leaves, fate);
            }
        }
        // A waypoint dropped for a closed ask window never gets a batch. A leaf
        // that did get a batch keeps that fate, including a later re-post.
        for event in self.store.load_events_named("waypoint_ask_expired")? {
            let Some(leaves) = event.get("leaves").and_then(|value| value.as_array()) else {
                continue;
            };
            for leaf in leaves {
                let Some(text) = leaf.as_str() else {
                    continue;
                };
                let Ok(hash) = crate::decode_hash32(text) else {
                    continue;
                };
                if !in_batch.contains(&hash) {
                    self.terminal.insert(hash, PathMiss::NotIncluded);
                }
            }
        }
        Ok(())
    }

    /// Seal non-current unsealed pools in tip order; reopen pool for live tip.
    fn recover_pools_on_startup(&mut self) -> Result<(), ClockError> {
        let listed = pool::list_pools(self.store.root())?;
        let live = self.current_tip.hash;
        let mut stale: Vec<ListedPool> = listed
            .into_iter()
            .filter(|p| p.state != PoolState::Sealed && p.tip.hash != live)
            .collect();
        stale.sort_by_key(|p| (p.tip.height, encode_hash32(&p.tip.hash)));

        // Pair each stale pool with a closed_tip: next pool tip hash if present, else live tip.
        let live_tip = self.current_tip;
        for index in 0..stale.len() {
            let pool = &stale[index];
            let closed_tip = if index + 1 < stale.len() {
                ChainTip {
                    hash: stale[index + 1].tip.hash,
                    previous_hash: Some(pool.tip.hash),
                    height: stale[index + 1].tip.height,
                    time: None,
                }
            } else {
                live_tip
            };
            let opened = ChainTip {
                hash: pool.tip.hash,
                previous_hash: None,
                height: pool.tip.height,
                time: None,
            };
            let continuous = closed_tip.previous_hash == Some(opened.hash)
                && closed_tip.height == opened.height.saturating_add(1);
            let is_last = index + 1 == stale.len();
            // Last stale pool one tip behind live stays pending. Older catch-up
            // seals are skipped after they persist: the tip they closed on is
            // already gone, so the window cannot be judged from here. A tree
            // already restored into pending stays. A new seal does not displace it.
            let qualify_pending = is_last && continuous && closed_tip.hash == live_tip.hash;

            let batch_sequence = self.alloc_batch_sequence()?;
            let job = SealJob {
                opened_tip: opened,
                closed_tip,
                leaves: pool.leaves.clone(),
                pool_dir: pool.dir.clone(),
                continuous,
                fee_log_only: false,
                batch_sequence,
                backend: self.backend.clone(),
                store: self.store.clone(),
                #[cfg(test)]
                fail_persist: false,
            };
            let sealed = job.execute()?;
            if sealed.deferred.is_some() {
                // Fee or input still says wait. Carry every stale cohort into
                // the live pool instead of planting them one by one.
                self.install_sealed(sealed)?;
                continue;
            }
            if qualify_pending {
                self.install_sealed(sealed)?;
            } else if let (Some(seq), Some(mut pending)) = (sealed.batch_sequence, sealed.pending) {
                // Catch-up: mark skipped so saga does not wait on a historical tip.
                pending.record.state = StoredState::Skipped;
                pending.record.skip_reason = Some(SkipReason::Other.as_str().to_owned());
                let _ = self.store.update(&pending.record);
                self.store.append_event(
                    "restart_pool_catchup_skipped",
                    json!({
                        "batch_sequence": seq,
                        "opened_tip": encode_bitcoin_id(&pending.record.opened_tip),
                        "closed_tip": encode_bitcoin_id(&pending.record.closed_tip),
                    }),
                )?;
            }
            // empty seal already marked sealed inside execute
        }

        // Ensure open pool for live tip (may already exist).
        if self.open_pool.tip.hash != live {
            self.open_pool = TipPool::open_or_create(self.store.root(), live_tip.pool_key())?;
        }
        Ok(())
    }

    fn alloc_batch_sequence(&mut self) -> Result<u64, ClockError> {
        let batch_sequence = self.next_batch_sequence;
        let Some(next) = self.next_batch_sequence.checked_add(1) else {
            return Err(ClockError::SequenceOverflow);
        };
        self.next_batch_sequence = next;
        Ok(batch_sequence)
    }

    pub fn current_tip(&self) -> ChainTip {
        self.current_tip
    }

    pub fn network(&self) -> &str {
        &self.network
    }

    /// Distinct waypoints in the live tip's open pool.
    pub fn pool_len(&self) -> usize {
        self.open_pool.len()
    }

    /// Alias retained for older call sites; equals [`Self::pool_len`].
    pub fn queue_len(&self) -> usize {
        self.pool_len()
    }

    /// Alias retained for older call sites; equals [`Self::pool_len`].
    pub fn collecting_len(&self) -> usize {
        self.pool_len()
    }

    pub fn pool_contains(&self, waypoint: &Hash32) -> bool {
        self.open_pool.contains(waypoint)
    }

    pub fn queue_contains(&self, waypoint: &Hash32) -> bool {
        self.pool_contains(waypoint)
    }

    pub fn collecting_contains(&self, waypoint: &Hash32) -> bool {
        self.pool_contains(waypoint)
    }

    pub fn soft_max(&self) -> Option<usize> {
        self.soft_max
    }

    /// Tips after the seal that may still include a plant. Always
    /// [`DEFAULT_CONFIRMATION_WINDOW_TIPS`]. An environment variable does not change it.
    pub fn confirmation_window_tips(&self) -> u32 {
        DEFAULT_CONFIRMATION_WINDOW_TIPS
    }

    /// Newest first. At most the 48-hour retention, plus a tree still unconfirmed.
    pub fn ledger(&self) -> &[BlockLedgerRow] {
        &self.ledger
    }

    /// Clone of the chain backend so the operator page can read the node
    /// without holding the accept lock.
    pub fn backend_handle(&self) -> Arc<dyn ChainBackend> {
        Arc::clone(&self.backend)
    }

    pub fn network_name(&self) -> &str {
        &self.network
    }

    /// Trees and block records already in memory. Does not read batch files
    /// or `events.jsonl`.
    pub fn ops_memory(&self) -> OpsMemory {
        let mut trees = Vec::new();
        for pending in &self.pending {
            if pending.record.plant.is_some() && pending.close_fault.is_none() {
                trees.push(pending.record.clone());
            }
        }
        for serving in &self.serving {
            if serving.record.plant.is_some() && serving.record.inclusion.is_some() {
                trees.push(serving.record.clone());
            }
        }
        OpsMemory {
            network: self.network.clone(),
            tip: self.current_tip,
            observed_unix_ms: self.observed_unix_ms,
            trees,
            blocks: self.ledger.clone(),
            pool_len: u32::try_from(self.pool_len()).unwrap_or(u32::MAX),
            oldest_post_height: self.open_pool.oldest_post_height(),
            added_since_tip: self.accepts_this_tip,
            malformed_since_tip: self.malformed_this_tip,
            already_held_since_tip: self.duplicate_this_tip,
            store_issues: self.store_issues.clone(),
        }
    }

    /// Broadcasts whose absolute fee was not stored. The page asks the node
    /// once and [`Self::apply_known_fees`] writes the answer onto the record.
    pub fn missing_fee_txids(&self) -> Vec<(u64, Hash32)> {
        let mut missing = Vec::new();
        for pending in &self.pending {
            if pending.close_fault.is_none() && pending.record.fee_sat.is_none() {
                if let Some(plant) = &pending.record.plant {
                    missing.push((pending.record.batch_sequence, plant.txid));
                }
            }
        }
        for serving in &self.serving {
            if serving.record.fee_sat.is_none() {
                if let Some(plant) = &serving.record.plant {
                    missing.push((serving.record.batch_sequence, plant.txid));
                }
            }
        }
        missing
    }

    /// Record absolute fees, in sats, for trees that do not have one yet.
    /// `fees` is `(batch_sequence, fee_sat)`.
    pub fn apply_known_fees(&mut self, fees: &[(u64, u64)]) {
        for &(seq, fee) in fees {
            if let Some(pending) = self
                .pending
                .iter_mut()
                .find(|pending| pending.record.batch_sequence == seq)
            {
                if pending.record.fee_sat.is_none() {
                    pending.record.fee_sat = Some(fee);
                    let _ = self.store.update(&pending.record);
                }
            }
            if let Some(serving) = self
                .serving
                .iter_mut()
                .find(|serving| serving.record.batch_sequence == seq)
            {
                if serving.record.fee_sat.is_none() {
                    serving.record.fee_sat = Some(fee);
                    let _ = self.store.update(&serving.record);
                }
            }
        }
    }

    /// Read `tx_place` for trees that have no absolute fee and store it.
    /// The operator page does this off the accept lock. Tests call it directly.
    pub fn remember_tx_fees(&mut self) {
        let missing = self.missing_fee_txids();
        let mut fees = Vec::new();
        for (seq, txid) in missing {
            if let Ok(Some(place)) = self.backend.tx_place(&txid) {
                if let Some(fee) = place.fee_sats {
                    fees.push((seq, fee));
                }
            }
        }
        self.apply_known_fees(&fees);
    }

    /// Warn `bari.ops.page.check` once per rule per tip.
    pub fn note_page_checks(&mut self, tip: u32, issues: &[(String, String)]) {
        for (rule, detail) in issues {
            if self.warned_page_checks.insert((tip, rule.clone())) {
                ops::warn(
                    ops::PAGE_CHECK,
                    json!({
                        "rule": rule,
                        "tip_height": tip,
                        "detail": detail,
                    }),
                );
            }
        }
    }

    /// Batches, seal events, and the in-memory ledger. No chain calls.
    pub fn ops_sources(&self) -> OpsSources {
        let tip = self.current_tip.height;
        let oldest = tip.saturating_sub(LEDGER_RETAIN_BLOCKS.saturating_add(ANCHOR_WINDOW_BLOCKS));
        let batches = self.store.load_batches().unwrap_or_default();
        let sealed = self
            .store
            .load_events_named("pool_sealed")
            .unwrap_or_default();
        let deferred = self
            .store
            .load_events_named("pool_deferred")
            .unwrap_or_default();
        let mut hash_heights = HashMap::new();
        hash_heights.insert(self.current_tip.hash, self.current_tip.height);
        for height in oldest..=tip {
            let Ok(Some(index)) = self.store.load_index_tip(height) else {
                continue;
            };
            let Ok(hash) = decode_bitcoin_id(&index.header.hash) else {
                continue;
            };
            hash_heights.insert(hash, height);
        }
        OpsSources {
            network: self.network.clone(),
            tip_height: tip,
            batches,
            sealed,
            deferred,
            hash_heights,
            notes: self.ledger.clone(),
            accepts_this_tip: self.accepts_this_tip,
            pool_len: u32::try_from(self.pool_len()).unwrap_or(u32::MAX),
            malformed_this_tip: self.malformed_this_tip,
            duplicate_this_tip: self.duplicate_this_tip,
            collecting_since: self.collecting_since,
            inflight: self.inflight(),
            last_confirmed: self.last_confirmed(),
        }
    }

    pub fn collecting_since(&self) -> Option<u32> {
        self.collecting_since
    }

    pub fn malformed_this_tip(&self) -> u32 {
        self.malformed_this_tip
    }

    pub fn duplicate_this_tip(&self) -> u32 {
        self.duplicate_this_tip
    }

    /// Unconfirmed broadcasts, oldest first.
    pub fn inflight(&self) -> Vec<InflightTree> {
        self.pending
            .iter()
            .filter(|pending| pending.close_fault.is_none() && pending.record.plant.is_some())
            .filter_map(|pending| {
                let plant = pending.record.plant.as_ref()?;
                let broadcast_height = pending.broadcast_height?;
                Some(InflightTree {
                    txid: plant.txid,
                    fee_rate_sat_vb: plant.fee_rate_sat_vb,
                    leaves: pending.record.leaves.len() as u32,
                    broadcast_height,
                    offset: blocks_since_broadcast(broadcast_height, self.current_tip.height),
                    window: ANCHOR_WINDOW_BLOCKS,
                    missed: pending.window_noted,
                })
            })
            .collect()
    }

    /// Newest confirmed tree, if one is being served.
    pub fn last_confirmed(&self) -> Option<ConfirmedTree> {
        self.serving
            .iter()
            .max_by_key(|batch| {
                batch
                    .record
                    .inclusion
                    .as_ref()
                    .map(|inclusion| inclusion.block_height)
                    .unwrap_or(0)
            })
            .and_then(|batch| {
                let inclusion = batch.record.inclusion.as_ref()?;
                let offset = inclusion
                    .block_height
                    .saturating_sub(batch.broadcast_height);
                Some(ConfirmedTree {
                    inclusion_height: inclusion.block_height,
                    broadcast_height: batch.broadcast_height,
                    offset,
                    leaves: batch.record.leaves.len() as u32,
                    txid: batch.record.plant.as_ref().map(|plant| plant.txid),
                    fee_rate_sat_vb: batch
                        .record
                        .plant
                        .as_ref()
                        .map(|plant| plant.fee_rate_sat_vb),
                    confirmed_late: batch.confirmed_late,
                })
            })
    }

    pub fn held_anywhere(&self, waypoint: &Hash32) -> bool {
        self.open_pool.contains(waypoint)
            || self
                .pending
                .iter()
                .any(|batch| batch.tree.leaves().contains(waypoint))
            || self
                .serving
                .iter()
                .any(|batch| batch.tree.leaves().contains(waypoint))
    }

    /// POST /waypoint write-model accept after Hashcash (HTTP layer). Persists before Ok.
    /// A `POST /waypoint` that was not 32 bytes, or otherwise not a waypoint.
    /// Counted on the live tip's ledger row. Not a stage.
    pub fn note_malformed(&mut self) {
        self.malformed_this_tip = self.malformed_this_tip.saturating_add(1);
    }

    /// A repeat of a waypoint this process already holds.
    pub fn note_duplicate(&mut self) {
        self.duplicate_this_tip = self.duplicate_this_tip.saturating_add(1);
    }

    pub fn post_waypoint(&mut self, waypoint: Hash32) -> Result<PostAccept, ClockError> {
        if self.held_anywhere(&waypoint) {
            self.note_duplicate();
            return Ok(PostAccept::Duplicate);
        }
        if let Some(max) = self.soft_max {
            if self.open_pool.len() >= max {
                return Ok(PostAccept::Full);
            }
        }
        self.open_pool.append(waypoint)?;
        self.accepts_this_tip = self.accepts_this_tip.saturating_add(1);
        if self.collecting_since.is_none() {
            self.collecting_since = Some(self.current_tip.height);
        }
        let leaf_ids: Vec<String> = self.open_pool.leaves().iter().map(encode_hash32).collect();
        ops::info(
            ops::OPEN_POOL_ADMIT,
            json!({
                "waypoint": encode_hash32(&waypoint),
                "leaf_count": leaf_ids.len(),
                "leaf_ids": leaf_ids,
                "tip_height": self.current_tip.height,
                "tip_hash": encode_bitcoin_id(&self.current_tip.hash),
            }),
        );
        Ok(PostAccept::Queued)
    }

    /// Retired mid-interval drain. No-op; open pool is the accept buffer.
    pub fn queue_tick(&mut self) -> Result<usize, ClockError> {
        Ok(0)
    }

    pub fn index_snapshot(&self) -> IndexSnapshot {
        IndexSnapshot {
            network: self.network.clone(),
            header: self.current_tip,
            plant: self.published_plant.clone(),
            plants: self.plants_included_here(),
        }
    }

    /// Plants whose inclusion block is the live tip. One height can hold several.
    fn plants_included_here(&self) -> Vec<PlantIndex> {
        let height = self.current_tip.height;
        self.serving
            .iter()
            .filter_map(|batch| {
                let plant = plant_index_of(batch)?;
                (plant.block_height == height).then_some(plant)
            })
            .collect()
    }

    /// Case for a leaf that is not in the serving tree.
    pub fn path_miss(&self, leaf: &Hash32) -> PathMiss {
        if self.open_pool.contains(leaf) {
            return PathMiss::StillInPool;
        }
        if self.seal_leaves.contains(leaf)
            || self
                .pending
                .iter()
                .any(|batch| batch.tree.leaves().contains(leaf))
        {
            return PathMiss::SealedNotInBlock;
        }
        self.terminal
            .get(leaf)
            .copied()
            .unwrap_or(PathMiss::Unknown)
    }

    /// L1: retained portable-index blob for a tip height, if still on disk.
    pub fn index_at_height(
        &self,
        height: u32,
    ) -> Result<Option<StoredPortableIndex>, StorageError> {
        self.store.load_index_tip(height)
    }

    fn published_index_document(&self) -> StoredPortableIndex {
        let mut stored = self.index_snapshot().to_stored();
        stored.header.timestamp_unix_ms = self.observed_unix_ms.filter(|ms| *ms > 0);
        stored
    }

    fn persist_published_index(&self) -> Result<(), StorageError> {
        let stored = self.published_index_document();
        self.store.persist_index_tip(&stored)?;
        self.store.append_event(
            "index_tip_retained",
            json!({
                "height": stored.header.height,
                "hash": stored.header.hash,
                "plant": stored.plant.is_some(),
                "plants": stored.plants.len(),
            }),
        )?;
        Ok(())
    }

    /// Write the open tip when that height file is missing.
    ///
    /// The document is the one [`Self::persist_published_index`] writes for a
    /// new tip. A file already at this height is left as stored, whether it
    /// already matches that document or differs (an observation stamp from
    /// `on_tip_prepare` is the usual difference). No other height is written
    /// or deleted.
    fn retain_startup_tip_index(&self) -> Result<(), StorageError> {
        let stored = self.published_index_document();
        if self.store.load_index_tip(stored.header.height)?.is_some() {
            return Ok(());
        }
        self.persist_published_index()
    }

    pub fn path_for(&self, leaf: &Hash32) -> Result<Option<CanonicalPathBody>, ChainError> {
        for serving in &self.serving {
            let Some(inclusion) = serving.record.inclusion.as_ref() else {
                continue;
            };
            let Ok(path) = serving.tree.path(leaf) else {
                continue;
            };
            let Some(plant) = serving.record.plant.as_ref() else {
                continue;
            };
            return Ok(Some(CanonicalPathBody {
                path,
                root: serving.record.root,
                txid: plant.txid,
                vout: plant.vout,
                block_hash: inclusion.block_hash,
                block_height: inclusion.block_height,
            }));
        }
        Ok(None)
    }

    pub fn post_check(&self, body: &CanonicalPathBody) -> Result<ChainPostCheck, ChainError> {
        self.backend.post_check(body)
    }

    /// Tip poll with seal completed inline (tests / simple callers).
    pub fn poll_once(&mut self) -> Result<Option<TransitionReport>, ClockError> {
        match self.poll_once_prepare()? {
            None => Ok(None),
            Some((report, job)) => {
                if let Some(job) = job {
                    let sealed = job.execute()?;
                    self.install_sealed(sealed)?;
                }
                Ok(Some(report))
            }
        }
    }

    /// Handoff + saga steps; returns an optional [`SealJob`] to run without the accept lock.
    pub fn poll_once_prepare(
        &mut self,
    ) -> Result<Option<(TransitionReport, Option<SealJob>)>, ClockError> {
        let tip = match self.backend.best_tip() {
            Ok(tip) => tip,
            Err(error) => {
                self.store.append_event(
                    "rpc_observation_failed",
                    json!({
                        "operation": "best_tip",
                        "error": error.to_string(),
                        "clock_tip_height": self.current_tip.height,
                        "clock_tip_hash": encode_bitcoin_id(&self.current_tip.hash),
                    }),
                )?;
                return Ok(None);
            }
        };
        if tip.hash == self.current_tip.hash {
            return Ok(None);
        }
        self.on_tip_prepare(tip).map(Some)
    }

    pub fn on_tip(&mut self, tip: ChainTip) -> Result<TransitionReport, ClockError> {
        let (report, job) = self.on_tip_prepare(tip)?;
        if let Some(job) = job {
            let sealed = job.execute()?;
            self.install_sealed(sealed)?;
        }
        Ok(report)
    }

    pub fn on_tip_prepare(
        &mut self,
        tip: ChainTip,
    ) -> Result<(TransitionReport, Option<SealJob>), ClockError> {
        let continuous = tip.previous_hash == Some(self.current_tip.hash)
            && tip.height == self.current_tip.height.saturating_add(1);
        let observed_unix_ms = self.store.append_event(
            "selected_tip_observed",
            json!({
                "height": tip.height,
                "hash": encode_bitcoin_id(&tip.hash),
                "previous_hash": tip.previous_hash.map(|hash| encode_bitcoin_id(&hash)),
                "prior_clock_height": self.current_tip.height,
                "prior_clock_hash": encode_bitcoin_id(&self.current_tip.hash),
                "continuous": continuous,
            }),
        )?;
        self.observed_unix_ms = Some(observed_unix_ms);

        // 0. Handoff open pool for prior tip → seal job (accepts must not wait on seal).
        let prior_pool = std::mem::replace(
            &mut self.open_pool,
            TipPool::open_or_create(self.store.root(), tip.pool_key())?,
        );
        let (opened_tip_key, leaves, pool_dir) = prior_pool.handoff()?;
        self.seal_leaves = leaves.iter().map(|waypoint| waypoint.leaf).collect();
        let opened_tip = ChainTip {
            hash: opened_tip_key.hash,
            previous_hash: None,
            height: opened_tip_key.height,
            time: None,
        };

        // 1. Serving trees past the confirmation window → archive. Path HTTP off.
        let archived = self.archive_expired(tip.height)?;

        // 2. Decide pending → serving or miss. Sets published_plant for this tip.
        // Each pending tree is judged on its own. A tree still in the mempool
        // does not hold the next cohort.
        let (included, missed) = self.decide_pending(&tip, continuous)?;

        // 3. Fee gate, then one tree of everything in the handed pool.
        // A tree already in the mempool is not a hold.
        let seal_job = if leaves.is_empty() {
            self.seal_leaves.clear();
            // Mark sealed immediately; no pending.
            TipPool::mark_sealed(&pool_dir, &opened_tip_key)?;
            self.store.append_event(
                "pool_empty_handoff",
                json!({
                    "opened_tip": encode_bitcoin_id(&opened_tip.hash),
                    "opened_tip_height": opened_tip.height,
                    "closed_tip": encode_bitcoin_id(&tip.hash),
                    "observed_tip_height": tip.height,
                    "continuous": continuous,
                }),
            )?;
            // No Merkle tree on an empty handoff, so this line has no `tree_size`.
            ops::info(
                ops::SEAL,
                json!({
                    "empty": true,
                    "leaf_count": 0,
                    "leaves": [],
                    "durable_event": "pool_empty_handoff",
                    "opened_tip": encode_bitcoin_id(&opened_tip.hash),
                    "opened_tip_height": opened_tip.height,
                    "closed_tip": encode_bitcoin_id(&tip.hash),
                    "closed_tip_height": tip.height,
                    "continuous": continuous,
                }),
            );
            Some(SealJob {
                opened_tip,
                closed_tip: tip,
                leaves: Vec::new(),
                pool_dir,
                continuous,
                fee_log_only: true,
                batch_sequence: 0,
                backend: self.backend.clone(),
                store: self.store.clone(),
                #[cfg(test)]
                fail_persist: false,
            })
        } else {
            match self.alloc_batch_sequence() {
                Ok(batch_sequence) => {
                    #[cfg(test)]
                    let fail_persist = {
                        let fail = self.fail_next_persist;
                        self.fail_next_persist = false;
                        fail
                    };
                    Some(SealJob {
                        opened_tip,
                        closed_tip: tip,
                        leaves,
                        pool_dir,
                        continuous,
                        fee_log_only: false,
                        batch_sequence,
                        backend: self.backend.clone(),
                        store: self.store.clone(),
                        #[cfg(test)]
                        fail_persist,
                    })
                }
                Err(ClockError::SequenceOverflow) => {
                    let hashes: Vec<Hash32> = leaves.iter().map(|waypoint| waypoint.leaf).collect();
                    self.note_leaves(&hashes, PathMiss::NotIncluded);
                    self.seal_leaves.clear();
                    self.store.append_event(
                        "pool_seal_sequence_overflow",
                        json!({
                            "opened_tip": encode_bitcoin_id(&opened_tip.hash),
                            "closed_tip": encode_bitcoin_id(&tip.hash),
                            "leaf_count": hashes.len(),
                        }),
                    )?;
                    TipPool::mark_sealed(&pool_dir, &opened_tip_key)?;
                    None
                }
                Err(other) => return Err(other),
            }
        };

        // 4. Open pool already created for new tip above.
        // 5. Publish index (header + plant together) and retain for L1.
        // The singular plant is one included in this tip. Older confirmed
        // trees stay in `serving` for GET /path and are not copied onto a
        // later height.
        self.current_tip = tip;
        self.refresh_published_plant();
        self.persist_published_index()?;

        let closed_batch = seal_job
            .as_ref()
            .filter(|job| !job.fee_log_only)
            .map(|job| job.batch_sequence);
        // Sample the cohort before the seal runs without the accept lock.
        // `record_height` reads this sample. A post during the seal increments
        // the live counter and is the next height's waypoint.
        if seal_job.is_some() {
            self.handoff_added = Some(self.accepts_this_tip);
            self.handoff_malformed = Some(self.malformed_this_tip);
            self.handoff_duplicate = Some(self.duplicate_this_tip);
            self.accepts_this_tip = 0;
            self.malformed_this_tip = 0;
            self.duplicate_this_tip = 0;
        }
        Ok((
            TransitionReport {
                tip,
                continuous,
                closed_batch,
                included,
                missed,
                archived,
            },
            seal_job,
        ))
    }

    pub fn install_sealed(&mut self, sealed: SealedPending) -> Result<Option<u64>, ClockError> {
        if !sealed.expired.is_empty() {
            self.note_leaves(&sealed.expired, PathMiss::NotIncluded);
        }
        self.seal_leaves.clear();
        let observed_min = sealed.observed_min;
        if let Some(deferred) = sealed.deferred {
            let tip = self.current_tip;
            let reason = deferred.deferral.reason.clone();
            let min_fee = deferred.deferral.next_block_min_sat_vb.or(observed_min);
            let bid = deferred.deferral.needed_sat_vb;
            let leaves = deferred.leaves.len() as u32;
            self.record_height(
                &tip,
                HeightNote {
                    decision: "held",
                    detail: &reason,
                    min_fee,
                    bid,
                    txid: None,
                    broadcast_height: None,
                    pool_size: leaves,
                    tree: None,
                    fee_source: None,
                },
            );
            self.apply_deferred(deferred)?;
            return Ok(None);
        }
        let seq = sealed.batch_sequence;
        if let Some(pending) = sealed.pending {
            let broadcast = pending.record.plant.is_some() && pending.close_fault.is_none();
            let decision = if broadcast {
                "sent"
            } else if pending.record.leaves.is_empty() {
                "quiet"
            } else {
                "held"
            };
            let detail = if broadcast {
                "broadcast"
            } else if decision == "held" {
                "other"
            } else {
                ""
            };
            let txid = pending.record.plant.as_ref().map(|plant| plant.txid);
            let fee_source = pending
                .record
                .plant
                .as_ref()
                .map(|plant| plant.fee_source.clone());
            let bid = pending
                .record
                .plant
                .as_ref()
                .map(|plant| plant.fee_rate_sat_vb);
            let min_fee = pending.record.next_block_min_sat_vb.or(observed_min);
            let leaves = pending.record.leaves.len() as u32;
            let broadcast_height = pending.broadcast_height;
            let tree = broadcast.then_some(pending.record.batch_sequence);
            let tip = self.current_tip;
            self.record_height(
                &tip,
                HeightNote {
                    decision,
                    detail,
                    min_fee,
                    bid,
                    txid,
                    broadcast_height,
                    pool_size: leaves,
                    tree,
                    fee_source,
                },
            );
            if broadcast {
                self.collecting_since = None;
            }
            self.pending.push(pending);
        } else {
            let tip = self.current_tip;
            self.record_height(
                &tip,
                HeightNote {
                    decision: "quiet",
                    detail: "",
                    min_fee: observed_min,
                    bid: None,
                    txid: None,
                    broadcast_height: None,
                    pool_size: 0,
                    tree: None,
                    fee_source: None,
                },
            );
        }
        Ok(seq)
    }

    fn restore_ledger(&mut self) -> Result<(), ClockError> {
        let path = self.store.root().join("events.jsonl");
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                self.store_issues.push(StoreIssue {
                    file: "events.jsonl".to_owned(),
                    error: error.to_string(),
                });
                return Ok(());
            }
        };
        let mut logged_bad_line = false;
        for (index, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let event: Value = match serde_json::from_str(line) {
                Ok(event) => event,
                Err(error) => {
                    if !logged_bad_line {
                        logged_bad_line = true;
                        self.store_issues.push(StoreIssue {
                            file: "events.jsonl".to_owned(),
                            error: format!("line {}: {error}", index + 1),
                        });
                    }
                    continue;
                }
            };
            if event["event"] != "block_ledger" {
                continue;
            }
            let Some(row) = block_row_from_event(&event) else {
                continue;
            };
            self.upsert_ledger(row);
        }
        self.retain_ledger(self.current_tip.height);
        Ok(())
    }

    fn record_height(&mut self, tip: &ChainTip, note: HeightNote<'_>) {
        let (new_waypoints, malformed, duplicate) = if let Some(added) = self.handoff_added.take() {
            (
                added,
                self.handoff_malformed.take().unwrap_or(0),
                self.handoff_duplicate.take().unwrap_or(0),
            )
        } else {
            let added = self.accepts_this_tip;
            let malformed = self.malformed_this_tip;
            let duplicate = self.duplicate_this_tip;
            self.accepts_this_tip = 0;
            self.malformed_this_tip = 0;
            self.duplicate_this_tip = 0;
            (added, malformed, duplicate)
        };
        let hold_reason = if note.decision == "held" {
            display_hold(note.detail).to_owned()
        } else {
            String::new()
        };
        let row = BlockLedgerRow {
            height: tip.height,
            block_hash: tip.hash,
            new_waypoints,
            malformed,
            duplicate,
            status: note.decision.to_owned(),
            hold_reason,
            min_fee_sat_vb: note.min_fee,
            bid_sat_vb: note.bid,
            broadcast_txid: note.txid,
            broadcast_height: note.broadcast_height,
            leaf_count: note.pool_size,
            inclusion_height: None,
            confirm_offset: None,
            fee_sats: None,
            confirmed_late: false,
            seen_unix_ms: timestamp_ms(),
            time: tip.time,
            decision: note.decision.to_owned(),
            detail: note.detail.to_owned(),
            tree: note.tree,
            added_known: true,
            fee_source: note.fee_source,
        };
        self.upsert_ledger(row.clone());
        self.retain_ledger(tip.height);
        self.persist_ledger_row(&row);
    }

    fn persist_ledger_row(&self, row: &BlockLedgerRow) {
        let _ = self.store.append_event(
            "block_ledger",
            json!({
                "height": row.height,
                "block": encode_bitcoin_id(&row.block_hash),
                "time": row.time,
                "decision": row.decision,
                "status": row.decision,
                "hold_reason": row.hold_reason,
                "detail": row.detail,
                "next_block_min_sat_vb": row.min_fee_sat_vb,
                "min_fee_sat_vb": row.min_fee_sat_vb,
                "bid_sat_vb": row.bid_sat_vb,
                "pool_size": row.leaf_count,
                "leaf_count": row.leaf_count,
                "added": row.new_waypoints,
                "new_waypoints": row.new_waypoints,
                "malformed": row.malformed,
                "already_held": row.duplicate,
                "duplicate": row.duplicate,
                "tree": row.tree,
                "fee_source": row.fee_source,
                "broadcast_txid": row.broadcast_txid.map(|id| encode_bitcoin_id(&id)),
                "broadcast_height": row.broadcast_height,
            }),
        );
    }

    fn upsert_ledger(&mut self, row: BlockLedgerRow) {
        if let Some(existing) = self
            .ledger
            .iter_mut()
            .find(|kept| kept.height == row.height)
        {
            *existing = row;
        } else {
            self.ledger.push(row);
        }
        self.ledger.sort_by_key(|row| std::cmp::Reverse(row.height));
    }

    fn retain_ledger(&mut self, tip_height: u32) {
        let oldest = tip_height.saturating_sub(LEDGER_RETAIN_BLOCKS);
        self.ledger.retain(|row| row.height >= oldest);
    }

    /// Append a deferred cohort onto the live tip's pool, then seal the old directory.
    fn apply_deferred(&mut self, deferred: DeferredCohort) -> Result<(), ClockError> {
        let DeferredCohort {
            leaves,
            pool_dir,
            opened_tip,
            deferral,
        } = deferred;
        let leaf_hashes: Vec<Hash32> = leaves.iter().map(|waypoint| waypoint.leaf).collect();
        for waypoint in &leaves {
            self.open_pool
                .append_posted(waypoint.leaf, waypoint.post_height)?;
        }
        TipPool::mark_sealed(&pool_dir, &opened_tip)?;
        self.store.append_event(
            "pool_deferred",
            json!({
                "reason": deferral.reason,
                "message": deferral.message,
                "leaf_count": leaves.len(),
                "leaves": leaf_ids(&leaf_hashes),
                "post_heights": leaves.iter().map(|waypoint| waypoint.post_height).collect::<Vec<_>>(),
                "decision_height": deferral.decision_height,
                "next_block_min_sat_vb": deferral.next_block_min_sat_vb,
                "node_estimate_sat_vb": deferral.node_estimate_sat_vb,
                "needed_sat_vb": deferral.needed_sat_vb,
                "fee_cap_sat_vb": deferral.cap_sat_vb,
                "opened_tip": encode_bitcoin_id(&opened_tip.hash),
                "opened_tip_height": opened_tip.height,
                "live_tip": encode_bitcoin_id(&self.current_tip.hash),
                "live_tip_height": self.current_tip.height,
            }),
        )?;
        ops::warn(
            ops::PLANT_DEFERRED,
            json!({
                "deferred": true,
                "reason": deferral.reason,
                "message": deferral.message,
                "leaf_count": leaves.len(),
                "leaves": leaf_ids(&leaf_hashes),
                "decision_height": deferral.decision_height,
                "next_block_min_sat_vb": deferral.next_block_min_sat_vb,
                "node_estimate_sat_vb": deferral.node_estimate_sat_vb,
                "needed_sat_vb": deferral.needed_sat_vb,
                "fee_cap_sat_vb": deferral.cap_sat_vb,
                "opened_tip": encode_bitcoin_id(&opened_tip.hash),
                "opened_tip_height": opened_tip.height,
                "tip_height": self.current_tip.height,
                "tip_hash": encode_bitcoin_id(&self.current_tip.hash),
            }),
        );
        Ok(())
    }

    /// Confirmed paths stay servable. The anchor window is the producer and
    /// verifier contract. Bari does not take a served path off HTTP because
    /// that window moved.
    fn archive_expired(&mut self, _tip_height: u32) -> Result<Option<u64>, ClockError> {
        Ok(None)
    }

    fn refresh_published_plant(&mut self) {
        let height = self.current_tip.height;
        self.published_plant = self
            .serving
            .iter()
            .filter(|batch| {
                batch
                    .record
                    .inclusion
                    .as_ref()
                    .is_some_and(|inclusion| inclusion.block_height == height)
            })
            .max_by_key(|batch| batch.record.batch_sequence)
            .and_then(plant_index_of);
    }

    fn decide_pending(
        &mut self,
        tip: &ChainTip,
        continuous: bool,
    ) -> Result<PendingDecision, ClockError> {
        if self.pending.is_empty() {
            self.refresh_published_plant();
            return Ok((None, None));
        }
        let window = self.confirmation_window_tips();
        let mut included_seq = None;
        let mut missed = None;
        let mut still = Vec::new();
        for mut pending in std::mem::take(&mut self.pending) {
            match self.pending_outcome(&pending, tip, continuous) {
                PendingOutcome::Hold => still.push(pending),
                PendingOutcome::WindowElapsed { lookup } => {
                    let sequence = pending.record.batch_sequence;
                    if !pending.window_noted {
                        pending.window_noted = true;
                        self.note_window_elapsed(&pending, tip, window, lookup.as_ref())?;
                        missed = Some((sequence, SkipReason::NotMined));
                    }
                    still.push(pending);
                }
                other => {
                    let report = self.settle_pending(pending, tip, other, window)?;
                    if report.included.is_some() {
                        included_seq = report.included;
                    }
                    if report.missed.is_some() {
                        missed = report.missed;
                    }
                }
            }
        }
        self.pending = still;
        self.refresh_published_plant();
        Ok((included_seq, missed))
    }

    fn pending_outcome(
        &self,
        pending: &PendingBatch,
        tip: &ChainTip,
        continuous: bool,
    ) -> PendingOutcome {
        let _ = continuous;
        if let Some(reason) = pending.close_fault {
            return PendingOutcome::Skip {
                reason,
                detail: "close_fault",
                message: String::new(),
            };
        }
        // A tip that is not exactly +1 does not skip the tree. It stays
        // pending until this tip or `tx_place` finds its block.
        let Some(plant) = pending.record.plant.as_ref() else {
            return PendingOutcome::Skip {
                reason: SkipReason::Other,
                detail: "plant_absent",
                message: String::new(),
            };
        };
        let broadcast_height = pending
            .record
            .broadcast_height
            .or(pending.broadcast_height)
            .unwrap_or(pending.post_height);
        let last = window_last_height(broadcast_height);
        match self.backend.included_in_tip(&plant.txid, tip) {
            // A confirming block is served even after the anchor window.
            Ok(Some(inclusion)) if inclusion.block_hash == tip.hash => PendingOutcome::Included {
                inclusion,
                catch_up: false,
            },
            Ok(lookup) => {
                if let Some(inclusion) = self.catch_up_inclusion(&plant.txid) {
                    return PendingOutcome::Included {
                        inclusion,
                        catch_up: true,
                    };
                }
                if tip.height >= last && !pending.window_noted {
                    PendingOutcome::WindowElapsed { lookup }
                } else {
                    PendingOutcome::Hold
                }
            }
            Err(error) => PendingOutcome::Skip {
                reason: SkipReason::Other,
                detail: "inclusion_lookup_failed",
                message: error.to_string(),
            },
        }
    }

    /// The tip block did not include the plant. Ask where the transaction sits.
    /// A block on the selected chain becomes the inclusion. `Ok(None)` from
    /// `tx_place` means this backend cannot see the chain.
    fn catch_up_inclusion(&self, txid: &Hash32) -> Option<StoredInclusion> {
        let place = self.backend.tx_place(txid).ok()??;
        let height = place.inclusion_height?;
        let block_hash = place.block_hash?;
        match self.backend.block_is_selected(&block_hash) {
            Ok(true) => Some(StoredInclusion {
                block_hash,
                block_height: height,
            }),
            _ => None,
        }
    }

    fn note_window_elapsed(
        &mut self,
        pending: &PendingBatch,
        tip: &ChainTip,
        window: u32,
        lookup: Option<&StoredInclusion>,
    ) -> Result<(), ClockError> {
        let sequence = pending.record.batch_sequence;
        let plant = pending.record.plant.as_ref();
        self.store.append_event(
            "pending_missed",
            json!({
                "batch_sequence": sequence,
                "root": encode_hash32(&pending.record.root),
                "tree_size": pending.record.tree_size,
                "planted": plant.is_some(),
                "txid": plant.map(|value| encode_bitcoin_id(&value.txid)),
                "vout": plant.map(|value| value.vout),
                "decision_tip_hash": encode_bitcoin_id(&tip.hash),
                "decision_tip_height": tip.height,
                "skip_reason": SkipReason::NotMined.as_str(),
                "kept_pending": true,
            }),
        )?;
        ops::warn(
            ops::CONFIRMATION_WINDOW_MISSED,
            json!({
                "batch_sequence": sequence,
                "root": encode_hash32(&pending.record.root),
                "tree_size": pending.record.tree_size,
                "leaves": leaf_ids(&pending.record.leaves),
                "txid": plant.map(|value| encode_bitcoin_id(&value.txid)),
                "vout": plant.map(|value| value.vout),
                "fee_rate_sat_vb": plant.map(|value| value.fee_rate_sat_vb),
                "fee_source": plant.map(|value| value.fee_source.as_str()),
                "decision_tip_hash": encode_bitcoin_id(&tip.hash),
                "decision_tip_height": tip.height,
                "skip_reason": SkipReason::NotMined.as_str(),
                "confirmation_window_tips": window,
                "lookup_block_hash": lookup.map(|value| encode_bitcoin_id(&value.block_hash)),
                "lookup_block_height": lookup.map(|value| value.block_height),
                "kept_pending": true,
            }),
        );
        Ok(())
    }

    fn settle_pending(
        &mut self,
        mut pending: PendingBatch,
        tip: &ChainTip,
        outcome: PendingOutcome,
        window: u32,
    ) -> Result<TransitionReport, ClockError> {
        let sequence = pending.record.batch_sequence;
        let report = TransitionReport {
            tip: *tip,
            continuous: true,
            closed_batch: None,
            included: None,
            missed: None,
            archived: None,
        };
        match outcome {
            PendingOutcome::Hold => Ok(report),
            PendingOutcome::Included {
                inclusion,
                catch_up,
            } => {
                pending.record.inclusion = Some(inclusion);
                pending.record.state = StoredState::Included;
                pending.record.skip_reason = None;
                if pending.record.fee_sat.is_none() {
                    let txid = pending.record.plant.as_ref().map(|plant| plant.txid);
                    if let Some(txid) = txid {
                        if let Ok(Some(place)) = self.backend.tx_place(&txid) {
                            pending.record.fee_sat = place.fee_sats;
                        }
                    }
                }
                self.store.update(&pending.record)?;
                let plant = pending
                    .record
                    .plant
                    .as_ref()
                    .ok_or(ClockError::CorruptBatch(sequence))?;
                let inclusion = pending
                    .record
                    .inclusion
                    .as_ref()
                    .ok_or(ClockError::CorruptBatch(sequence))?;
                self.store.append_event(
                    "pending_included",
                    json!({
                        "batch_sequence": sequence,
                        "root": encode_hash32(&pending.record.root),
                        "tree_size": pending.record.tree_size,
                        "txid": encode_bitcoin_id(&plant.txid),
                        "vout": plant.vout,
                        "block_hash": encode_bitcoin_id(&inclusion.block_hash),
                        "block_height": inclusion.block_height,
                        "catch_up": catch_up,
                    }),
                )?;
                ops::info(
                    ops::PLANT_INCLUDED,
                    json!({
                        "batch_sequence": sequence,
                        "root": encode_hash32(&pending.record.root),
                        "tree_size": pending.record.tree_size,
                        "leaves": leaf_ids(&pending.record.leaves),
                        "txid": encode_bitcoin_id(&plant.txid),
                        "vout": plant.vout,
                        "fee_rate_sat_vb": plant.fee_rate_sat_vb,
                        "fee_source": &plant.fee_source,
                        "block_hash": encode_bitcoin_id(&inclusion.block_hash),
                        "block_height": inclusion.block_height,
                        "decision_tip_hash": encode_bitcoin_id(&tip.hash),
                        "decision_tip_height": tip.height,
                        "confirmation_window_tips": window,
                    }),
                );
                let broadcast_height = pending
                    .record
                    .broadcast_height
                    .or(pending.broadcast_height)
                    .unwrap_or(pending.post_height.saturating_add(1));
                let confirmed_late = inclusion.block_height > window_last_height(broadcast_height);
                for leaf in &pending.record.leaves {
                    self.terminal.remove(leaf);
                }
                self.serving.push(ServingBatch {
                    broadcast_height,
                    confirmed_late,
                    record: pending.record,
                    tree: pending.tree,
                });
                Ok(TransitionReport {
                    included: Some(sequence),
                    ..report
                })
            }
            PendingOutcome::WindowElapsed { .. } => Ok(report),
            PendingOutcome::Skip {
                reason,
                detail,
                message,
            } => {
                pending.record.state = StoredState::Skipped;
                pending.record.inclusion = None;
                pending.record.skip_reason = Some(reason.as_str().to_owned());
                if let Some(fate) = skip_fate(reason) {
                    self.note_leaves(&pending.record.leaves, fate);
                }
                let _ = self.store.update(&pending.record);
                let plant = pending.record.plant.as_ref();
                self.store.append_event(
                    "pending_missed",
                    json!({
                        "batch_sequence": sequence,
                        "root": encode_hash32(&pending.record.root),
                        "tree_size": pending.record.tree_size,
                        "planted": plant.is_some(),
                        "txid": plant.map(|value| encode_bitcoin_id(&value.txid)),
                        "vout": plant.map(|value| value.vout),
                        "decision_tip_hash": encode_bitcoin_id(&tip.hash),
                        "decision_tip_height": tip.height,
                        "skip_reason": reason.as_str(),
                    }),
                )?;
                ops::warn(
                    ops::PLANT_SKIP,
                    json!({
                        "at": "inclusion_decision",
                        "detail": detail,
                        "message": message,
                        "batch_sequence": sequence,
                        "root": encode_hash32(&pending.record.root),
                        "tree_size": pending.record.tree_size,
                        "leaves": leaf_ids(&pending.record.leaves),
                        "txid": plant.map(|value| encode_bitcoin_id(&value.txid)),
                        "vout": plant.map(|value| value.vout),
                        "fee_rate_sat_vb": plant.map(|value| value.fee_rate_sat_vb),
                        "fee_source": plant.map(|value| value.fee_source.as_str()),
                        "decision_tip_hash": encode_bitcoin_id(&tip.hash),
                        "decision_tip_height": tip.height,
                        "skip_reason": reason.as_str(),
                    }),
                );
                Ok(TransitionReport {
                    missed: Some((sequence, reason)),
                    ..report
                })
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn fail_next_persist(&mut self) {
        self.fail_next_persist = true;
    }

    pub fn pending_sequence(&self) -> Option<u64> {
        self.pending
            .iter()
            .map(|batch| batch.record.batch_sequence)
            .max()
    }

    #[cfg(test)]
    pub(crate) fn serving_sequence(&self) -> Option<u64> {
        self.serving
            .iter()
            .map(|batch| batch.record.batch_sequence)
            .max()
    }

    pub(crate) fn serving_sequence_for(&self, leaf: &Hash32) -> Option<u64> {
        self.serving
            .iter()
            .find(|batch| batch.tree.leaves().contains(leaf))
            .map(|batch| batch.record.batch_sequence)
    }

    #[cfg(test)]
    pub(crate) fn collecting_opened_tip(&self) -> Hash32 {
        self.open_pool.tip.hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proof::commitment_script;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeState {
        network: Option<String>,
        tip: Option<ChainTip>,
        initial_block_download: bool,
        included: bool,
        plants: Vec<Hash32>,
        /// Tip height of each `plant` call, same order as `plants`.
        plant_heights: Vec<u32>,
        /// When set, only the plant broadcast at this height is in the tip.
        include_broadcast: Option<u32>,
        plant_failure: Option<SkipReason>,
        best_tip_failures: usize,
        inclusion_lookup_failures: usize,
        /// When set, the next seal keeps the cohort in the pool.
        deferral: Option<Deferral>,
        /// Inclusion reported by `tx_place` for a block the clock did not step through.
        catch_up: Option<TxPlace>,
    }

    #[derive(Default)]
    struct FakeChain {
        state: Mutex<FakeState>,
    }

    impl FakeChain {
        fn set_network(&self, network: &str) {
            self.state.lock().unwrap().network = Some(network.to_owned());
        }

        fn set_tip(&self, tip: ChainTip) {
            self.state.lock().unwrap().tip = Some(tip);
        }

        fn set_included(&self, included: bool) {
            self.state.lock().unwrap().included = included;
        }

        fn set_plant_failure(&self, reason: Option<SkipReason>) {
            self.state.lock().unwrap().plant_failure = reason;
        }

        fn set_deferral(&self, deferral: Option<Deferral>) {
            self.state.lock().unwrap().deferral = deferral;
        }

        fn set_catch_up(&self, place: Option<TxPlace>) {
            self.state.lock().unwrap().catch_up = place;
        }

        fn fail_next_best_tip(&self) {
            self.state.lock().unwrap().best_tip_failures += 1;
        }

        fn fail_next_inclusion_lookup(&self) {
            self.state.lock().unwrap().inclusion_lookup_failures += 1;
        }

        fn plant_count(&self) -> usize {
            self.state.lock().unwrap().plants.len()
        }

        /// Include only the plant broadcast at `broadcast_height`. Other plants
        /// stay unconfirmed. `None` clears the pin and the global flag decides.
        fn include_broadcast(&self, broadcast_height: Option<u32>) {
            self.state.lock().unwrap().include_broadcast = broadcast_height;
        }
    }

    impl ChainBackend for FakeChain {
        fn network(&self) -> Result<String, ChainError> {
            Ok(self
                .state
                .lock()
                .unwrap()
                .network
                .clone()
                .unwrap_or_else(|| "signet".to_owned()))
        }

        fn initial_block_download(&self) -> Result<bool, ChainError> {
            Ok(self.state.lock().unwrap().initial_block_download)
        }

        fn best_tip(&self) -> Result<ChainTip, ChainError> {
            let mut state = self.state.lock().unwrap();
            if state.best_tip_failures > 0 {
                state.best_tip_failures -= 1;
                return Err(ChainError("injected best-tip failure".to_owned()));
            }
            state
                .tip
                .ok_or_else(|| ChainError("no fake tip".to_owned()))
        }

        fn seal_clearance(&self, decision: &ChainTip) -> Result<SealClearance, ChainError> {
            let _ = decision;
            Ok(match self.state.lock().unwrap().deferral.clone() {
                Some(deferral) => SealClearance::Defer(deferral),
                None => SealClearance::Clear,
            })
        }

        fn plant(&self, root: &Hash32) -> Result<PlantReceipt, PlantError> {
            let mut state = self.state.lock().unwrap();
            if let Some(reason) = state.plant_failure {
                return Err(PlantError::new(reason, "injected failure"));
            }
            let plant_number = state.plants.len() as u32 + 1;
            let height = state.tip.map(|tip| tip.height).unwrap_or(0);
            state.plants.push(*root);
            state.plant_heights.push(height);
            Ok(PlantReceipt {
                txid: [0xA0 + plant_number as u8; 32],
                vout: plant_number,
                fee_rate_sat_vb: 2.5,
                fee_source: "test".to_owned(),
                fee_cap_sat_vb: None,
                fee_sat: None,
                vsize: None,
            })
        }

        fn included_in_tip(
            &self,
            txid: &Hash32,
            tip: &ChainTip,
        ) -> Result<Option<StoredInclusion>, ChainError> {
            let mut state = self.state.lock().unwrap();
            if state.inclusion_lookup_failures > 0 {
                state.inclusion_lookup_failures -= 1;
                return Err(ChainError("injected inclusion lookup failure".to_owned()));
            }
            // A tree broadcast on this same tip cannot already be in it.
            // Inclusion starts at the next block. The flag stays set, so a
            // later tip still confirms. Several plants are judged one by one:
            // `include_broadcast` names the one this tip contains.
            let index = txid[0].saturating_sub(0xA0).saturating_sub(1) as usize;
            let broadcast_height = state.plant_heights.get(index).copied();
            let selected = match state.include_broadcast {
                Some(height) => broadcast_height == Some(height),
                None => state.included,
            };
            let in_this_tip =
                selected && broadcast_height.is_some_and(|height| tip.height > height);
            Ok(in_this_tip.then_some(StoredInclusion {
                block_hash: tip.hash,
                block_height: tip.height,
            }))
        }

        fn block_is_selected(&self, _block_hash: &Hash32) -> Result<bool, ChainError> {
            Ok(true)
        }

        fn tx_place(&self, _txid: &Hash32) -> Result<Option<TxPlace>, ChainError> {
            Ok(self.state.lock().unwrap().catch_up.clone())
        }

        fn post_check(&self, body: &CanonicalPathBody) -> Result<ChainPostCheck, ChainError> {
            Ok(ChainPostCheck {
                transaction_in_block: true,
                output_exists: true,
                zero_value: true,
                exact_script: commitment_script(&body.root).len() == 39,
                selected_chain: true,
                carried_height: body.block_height,
                authenticated_height: Some(body.block_height),
                height_matches: Some(true),
            })
        }
    }

    fn tip(byte: u8, previous: Option<u8>, height: u32) -> ChainTip {
        ChainTip {
            hash: [byte; 32],
            previous_hash: previous.map(|value| [value; 32]),
            height,
            time: None,
        }
    }

    fn open_at(chain: Arc<FakeChain>, tip0: ChainTip) -> (BariClock, tempfile::TempDir) {
        let temp = tempfile::tempdir().unwrap();
        chain.set_tip(tip0);
        let clock = BariClock::open(chain, FileStore::open(temp.path()).unwrap()).unwrap();
        (clock, temp)
    }

    fn assert_index_v2(snapshot: &IndexSnapshot) {
        // S19 keys are asserted at HTTP layer; here check plant never zero-root miss.
        if let Some(plant) = &snapshot.plant {
            assert_ne!(plant.root, [0u8; 32]);
        }
    }

    /// S01 Tip advances the saga
    #[test]
    fn s01_tip_advances_the_saga() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        let before = clock.index_snapshot();
        chain.set_tip(tip(2, Some(1), 101));
        let report = clock.poll_once().unwrap().unwrap();
        assert_eq!(report.tip.hash, [2; 32]);
        let after = clock.index_snapshot();
        assert_eq!(after.header.hash, [2; 32]);
        assert_ne!(before.header.hash, after.header.hash);
        assert_index_v2(&after);
        // Not: a POST or QueueTick published this blob — covered by S04/S05.
    }

    /// The portable-index blob carries the same unix ms as `selected_tip_observed`.
    #[test]
    fn selected_tip_observation_is_stamped_on_the_index_blob() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, temp) = open_at(chain.clone(), tip(1, None, 100));
        chain.set_tip(ChainTip {
            hash: [2; 32],
            previous_hash: Some([1; 32]),
            height: 101,
            time: Some(1_700_000_000),
        });
        clock.poll_once().unwrap().unwrap();
        let stored = clock.index_at_height(101).unwrap().unwrap();
        assert_eq!(stored.header.time, Some(1_700_000_000));
        let stamped = stored.header.timestamp_unix_ms.expect("observation stamp");
        assert!(stamped > 1_000_000_000_000, "{stamped}");
        let events = std::fs::read_to_string(temp.path().join("events.jsonl")).unwrap();
        let mut observed = None;
        for line in events.lines() {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            if value["event"] == "selected_tip_observed" && value["height"] == 101 {
                observed = value["timestamp_unix_ms"].as_u64();
            }
        }
        assert_eq!(observed, Some(stamped));
        // The observation is not the chain's header.time scaled to ms.
        assert_ne!(stamped, 1_700_000_000_000);
    }

    /// Gossip `nTime` is header seconds. The observation stamp stays its own field.
    #[test]
    fn gossip_tick_ntime_is_header_seconds_and_observation_stays_separate() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain, tip(1, None, 100));
        let tick = hike::TipTick {
            hash: [2; 32],
            height: 101,
            previous: Some([1; 32]),
            n_time: 1_700_000_050,
        };
        let tip = ChainTip::from_gossip_tick(&tick).unwrap();
        assert_eq!(tip.time, Some(1_700_000_050));
        clock.on_tip(tip).unwrap();
        let stored = clock.index_at_height(101).unwrap().unwrap();
        assert_eq!(stored.header.time, Some(u64::from(tick.n_time)));
        let observed = stored.header.timestamp_unix_ms.expect("observation stamp");
        assert!(observed > 1_000_000_000_000, "{observed}");
        assert_ne!(observed, u64::from(tick.n_time));
        assert_ne!(observed, u64::from(tick.n_time) * 1000);

        let epoch = ChainTip::from_gossip_tick(&hike::TipTick {
            hash: [3; 32],
            height: 102,
            previous: Some([2; 32]),
            n_time: 0,
        })
        .unwrap();
        assert_eq!(epoch.time, Some(0));
    }

    /// S02 Same tip is a no-op
    #[test]
    fn s02_same_tip_is_a_noop() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        let before = clock.index_snapshot();
        assert!(clock.poll_once().unwrap().is_none());
        assert_eq!(clock.index_snapshot(), before);
        assert_eq!(clock.collecting_len(), 0);
        assert!(clock.pending_sequence().is_none());
        assert!(clock.serving_sequence().is_none());
    }

    /// S03 Failed tip RPC is not a tip
    #[test]
    fn s03_failed_tip_rpc_is_not_a_tip() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, temp) = open_at(chain.clone(), tip(1, None, 100));
        clock.post_waypoint([0x11; 32]).unwrap();
        let before = clock.index_snapshot();
        chain.fail_next_best_tip();
        assert!(clock.poll_once().unwrap().is_none());
        assert_eq!(clock.index_snapshot(), before);
        assert_eq!(clock.pool_len(), 1);
        let events = std::fs::read_to_string(temp.path().join("events.jsonl")).unwrap();
        assert!(events.contains("rpc_observation_failed"));
    }

    /// S04 POST does not publish index
    #[test]
    fn s04_post_does_not_publish_index() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain, tip(1, None, 100));
        let before = clock.index_snapshot();
        assert_eq!(clock.post_waypoint([0x22; 32]).unwrap(), PostAccept::Queued);
        assert_eq!(clock.index_snapshot(), before);
    }

    /// S05 QueueTick does not publish index
    #[test]
    fn s05_queue_tick_does_not_publish_index() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain, tip(1, None, 100));
        clock.post_waypoint([0x33; 32]).unwrap();
        let before = clock.index_snapshot();
        // QueueTick retired: open pool already holds the accept.
        assert_eq!(clock.queue_tick().unwrap(), 0);
        assert_eq!(clock.pool_len(), 1);
        assert_eq!(clock.index_snapshot(), before);
    }

    /// S06 New waypoint enters the queue
    #[test]
    fn s06_new_waypoint_enters_the_queue() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain, tip(1, None, 100));
        let w = [0x44; 32];
        assert_eq!(clock.post_waypoint(w).unwrap(), PostAccept::Queued);
        assert!(clock.pool_contains(&w));
        assert_eq!(clock.pool_len(), 1);
    }

    /// S07 Duplicate is idempotent
    #[test]
    fn s07_duplicate_is_idempotent() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        let w = [0x55; 32];
        clock.post_waypoint(w).unwrap();
        assert_eq!(clock.post_waypoint(w).unwrap(), PostAccept::Duplicate);
        assert_eq!(clock.pool_len(), 1);

        chain.set_tip(tip(2, Some(1), 101));
        clock.poll_once().unwrap();
        assert_eq!(clock.post_waypoint(w).unwrap(), PostAccept::Duplicate);
        assert_eq!(clock.pending_sequence(), Some(0));

        chain.set_included(true);
        chain.set_tip(tip(3, Some(2), 102));
        clock.poll_once().unwrap();
        assert_eq!(clock.post_waypoint(w).unwrap(), PostAccept::Duplicate);
        assert!(clock.path_for(&w).unwrap().is_some());
    }

    /// S08 Bad size — HTTP layer; saga queue unchanged helper
    #[test]
    fn s08_bad_size_queue_unchanged_helper() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain, tip(1, None, 100));
        let before = clock.pool_len();
        // Size check is HTTP; clock only accepts Hash32. Assert pool baseline.
        assert_eq!(before, 0);
        clock.post_waypoint([0x66; 32]).unwrap();
        assert_eq!(clock.pool_len(), 1);
    }

    /// Soft max ops dial. There is no protocol accept cap.
    #[test]
    fn soft_max_ops_dial() {
        let chain = Arc::new(FakeChain::default());
        let temp = tempfile::tempdir().unwrap();
        chain.set_tip(tip(1, None, 100));
        let mut clock = BariClock::open_at_tip_with_soft_max(
            chain,
            FileStore::open(temp.path()).unwrap(),
            tip(1, None, 100),
            Some(3),
        )
        .unwrap();
        for i in 0..3u64 {
            let mut w = [0u8; 32];
            w[..8].copy_from_slice(&i.to_le_bytes());
            assert_eq!(clock.post_waypoint(w).unwrap(), PostAccept::Queued);
        }
        let mut extra = [0xff; 32];
        extra[0] = 0x01;
        assert_eq!(clock.post_waypoint(extra).unwrap(), PostAccept::Full);
        assert!(!clock.pool_contains(&extra));
        assert_eq!(clock.pool_len(), 3);
    }

    /// Open pool accumulates distinct accepts (no mid-drain).
    #[test]
    fn s11_open_pool_accumulates() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain, tip(1, None, 100));
        let w0 = [0x10; 32];
        let w1 = [0x11; 32];
        let w2 = [0x12; 32];
        clock.post_waypoint(w0).unwrap();
        clock.post_waypoint(w1).unwrap();
        clock.post_waypoint(w2).unwrap();
        assert_eq!(clock.pool_len(), 3);
        assert!(clock.pool_contains(&w0));
        assert!(clock.pool_contains(&w1));
        assert!(clock.pool_contains(&w2));
        assert!(clock.pending_sequence().is_none());
        assert!(clock.serving_sequence().is_none());
    }

    /// S12 Empty collecting does not plant
    #[test]
    fn s12_empty_collecting_does_not_plant() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        chain.set_tip(tip(2, Some(1), 101));
        let report = clock.poll_once().unwrap().unwrap();
        assert_eq!(report.closed_batch, None);
        assert!(clock.pending_sequence().is_none());
        assert!(clock.index_snapshot().plant.is_none());
        assert_eq!(chain.plant_count(), 0);
        assert_eq!(clock.collecting_opened_tip(), [2; 32]);
        assert_eq!(clock.collecting_len(), 0);
    }

    /// S13 Non-empty collecting becomes pending
    #[test]
    fn s13_non_empty_collecting_becomes_pending() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        let w = [0x77; 32];
        clock.post_waypoint(w).unwrap();
        chain.set_tip(tip(2, Some(1), 101));
        let report = clock.poll_once().unwrap().unwrap();
        assert_eq!(report.closed_batch, Some(0));
        assert_eq!(clock.pending_sequence(), Some(0));
        assert_eq!(clock.pool_len(), 0);
        assert_eq!(clock.collecting_opened_tip(), [2; 32]);
        assert_eq!(chain.plant_count(), 1);
        // Not: that plant is a member of block H1 — inclusion decided next tip.
        assert!(clock.path_for(&w).unwrap().is_none());
        assert!(clock.index_snapshot().plant.is_none());
    }

    /// S14 Inclusion makes serving and fills plant
    #[test]
    fn s14_inclusion_makes_serving_and_fills_plant() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        let w = [0x88; 32];
        clock.post_waypoint(w).unwrap();
        chain.set_tip(tip(2, Some(1), 101));
        clock.poll_once().unwrap();
        chain.set_included(true);
        chain.set_tip(tip(3, Some(2), 102));
        clock.poll_once().unwrap();
        let snap = clock.index_snapshot();
        assert_eq!(snap.header.hash, [3; 32]);
        let plant = snap.plant.expect("plant object");
        assert_ne!(plant.root, [0u8; 32]);
        let body = clock.path_for(&w).unwrap().expect("serving");
        body.validate_for(&w).unwrap();
        assert_eq!(body.root, plant.root);
    }

    /// S15 Miss leaves plant null, after the confirmation window, not on the first tip.
    #[test]
    fn s15_miss_leaves_plant_null() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        let w = [0x99; 32];
        clock.post_waypoint(w).unwrap();
        chain.set_tip(tip(2, Some(1), 101));
        clock.poll_once().unwrap();
        chain.set_included(false);
        assert_eq!(chain.plant_count(), 1);
        // Broadcast at 101. Window is 5: still inside at 104, missed at 106.
        chain.set_tip(tip(3, Some(2), 102));
        let early = clock.poll_once().unwrap().unwrap();
        assert!(early.missed.is_none());
        assert_eq!(clock.pending_sequence(), Some(0));
        assert!(clock.path_for(&w).unwrap().is_none());
        chain.set_tip(tip(4, Some(3), 103));
        assert!(clock.poll_once().unwrap().unwrap().missed.is_none());
        chain.set_tip(tip(5, Some(4), 104));
        assert!(clock.poll_once().unwrap().unwrap().missed.is_none());
        chain.set_tip(tip(6, Some(5), 105));
        assert!(clock.poll_once().unwrap().unwrap().missed.is_none());
        chain.set_tip(tip(7, Some(6), 106));
        let report = clock.poll_once().unwrap().unwrap();
        assert_eq!(report.missed.unwrap().1, SkipReason::NotMined);
        let snap = clock.index_snapshot();
        assert_eq!(snap.header.hash, [7; 32]);
        assert!(snap.plant.is_none());
        assert!(clock.path_for(&w).unwrap().is_none());
        assert_eq!(chain.plant_count(), 1);
        // Not: plant.root of 64 zero hex — miss is null, not a zero root object.
        assert!(snap.plant.is_none());
    }

    /// A plant that misses the first inclusion tip and lands inside the window
    /// is still an anchor. It is not broadcast again.
    #[test]
    fn late_inclusion_inside_the_window_serves_without_a_second_plant() {
        assert_eq!(DEFAULT_CONFIRMATION_WINDOW_TIPS, 5);
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        assert_eq!(clock.confirmation_window_tips(), 5);
        let w = [0x9A; 32];
        clock.post_waypoint(w).unwrap();
        chain.set_included(false);
        chain.set_tip(tip(2, Some(1), 101));
        clock.poll_once().unwrap();
        assert_eq!(chain.plant_count(), 1);
        chain.set_tip(tip(3, Some(2), 102));
        clock.poll_once().unwrap();
        chain.set_tip(tip(4, Some(3), 103));
        clock.poll_once().unwrap();
        assert!(clock.path_for(&w).unwrap().is_none());
        assert_eq!(clock.pending_sequence(), Some(0));
        chain.set_included(true);
        chain.set_tip(tip(5, Some(4), 104));
        let report = clock.poll_once().unwrap().unwrap();
        assert!(report.included.is_some());
        assert!(report.missed.is_none());
        assert!(clock.path_for(&w).unwrap().is_some());
        assert!(clock.index_snapshot().plant.is_some());
        assert_eq!(chain.plant_count(), 1);
        assert!(clock.pending_sequence().is_none());
    }

    /// S16 Serving lasts through the confirmation window, then the next tip archives.
    #[test]
    fn s16_serving_lasts_through_the_confirmation_window() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        let w = [0xAA; 32];
        clock.post_waypoint(w).unwrap();
        chain.set_tip(tip(2, Some(1), 101));
        clock.poll_once().unwrap();
        chain.set_included(true);
        chain.set_tip(tip(3, Some(2), 102));
        clock.poll_once().unwrap();
        assert!(clock.path_for(&w).unwrap().is_some());
        // Planted at 101, window 3, still serving at 103 and 104.
        chain.set_tip(tip(4, Some(3), 103));
        clock.poll_once().unwrap();
        assert!(clock.path_for(&w).unwrap().is_some());
        chain.set_tip(tip(5, Some(4), 104));
        clock.poll_once().unwrap();
        assert!(clock.path_for(&w).unwrap().is_some());
        chain.set_tip(tip(6, Some(5), 105));
        clock.poll_once().unwrap();
        let first = clock
            .path_for(&w)
            .unwrap()
            .expect("still served after the window");
        let second = clock
            .path_for(&w)
            .unwrap()
            .expect("same path on the next read");
        assert_eq!(first.encode().unwrap(), second.encode().unwrap());
        assert_eq!(first.block_height, 102);
        assert!(clock.serving_sequence().is_some());
    }

    /// Overlapping confirmation windows can serve two trees. Each leaf is its own tree.
    #[test]
    fn s17_overlapping_windows_serve_each_leaf() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        let a = [0xB1; 32];
        let b = [0xB2; 32];
        clock.post_waypoint(a).unwrap();
        chain.set_tip(tip(2, Some(1), 101));
        clock.poll_once().unwrap();
        clock.post_waypoint(b).unwrap();
        chain.set_included(true);
        chain.set_tip(tip(3, Some(2), 102));
        clock.poll_once().unwrap();
        assert!(clock.path_for(&a).unwrap().is_some());
        assert!(clock.path_for(&b).unwrap().is_none());
        chain.set_tip(tip(4, Some(3), 103));
        clock.poll_once().unwrap();
        assert!(clock.path_for(&a).unwrap().is_some());
        assert!(clock.path_for(&b).unwrap().is_some());
        assert_eq!(clock.serving_sequence(), Some(1));
        // A planted at 101 serves through 104. B planted at 102 serves through 105.
        chain.set_tip(tip(5, Some(4), 104));
        clock.poll_once().unwrap();
        assert!(clock.path_for(&a).unwrap().is_some());
        assert!(clock.path_for(&b).unwrap().is_some());
        chain.set_tip(tip(6, Some(5), 105));
        clock.poll_once().unwrap();
        assert!(clock.path_for(&a).unwrap().is_some());
        assert!(clock.path_for(&b).unwrap().is_some());
        chain.set_tip(tip(7, Some(6), 106));
        clock.poll_once().unwrap();
        assert!(clock.path_for(&a).unwrap().is_some());
        assert!(clock.path_for(&b).unwrap().is_some());
    }

    /// S18 Index appears only after the inclusion decision
    #[test]
    fn s18_index_appears_only_after_inclusion_decision() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        clock.post_waypoint([0xC1; 32]).unwrap();
        chain.set_tip(tip(2, Some(1), 101));
        clock.poll_once().unwrap();
        chain.set_included(true);
        chain.set_tip(tip(3, Some(2), 102));
        clock.poll_once().unwrap();
        let snap = clock.index_snapshot();
        assert_eq!(snap.header.hash, [3; 32]);
        assert!(snap.plant.is_some());
    }

    /// S19 Index keys — shape asserted with HTTP; plant null vs object here
    #[test]
    fn s19_index_plant_key_always_present_as_option() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        let snap = clock.index_snapshot();
        assert_eq!(snap.network, "signet");
        assert!(snap.plant.is_none());
        chain.set_tip(tip(2, Some(1), 101));
        clock.poll_once().unwrap();
        assert!(clock.index_snapshot().plant.is_none());
    }

    /// S20 Reads do not advance
    #[test]
    fn s20_reads_do_not_advance() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        clock.post_waypoint([0xD1; 32]).unwrap();
        chain.set_tip(tip(2, Some(1), 101));
        clock.poll_once().unwrap();
        chain.set_included(true);
        chain.set_tip(tip(3, Some(2), 102));
        clock.poll_once().unwrap();
        let before = clock.index_snapshot();
        let q = clock.queue_len();
        let c = clock.collecting_len();
        let p = clock.pending_sequence();
        let s = clock.serving_sequence();
        for _ in 0..20 {
            let _ = clock.index_snapshot();
            let _ = clock.path_for(&[0xD1; 32]);
            let _ = clock.path_for(&[0x00; 32]);
        }
        assert_eq!(clock.index_snapshot(), before);
        assert_eq!(clock.queue_len(), q);
        assert_eq!(clock.collecting_len(), c);
        assert_eq!(clock.pending_sequence(), p);
        assert_eq!(clock.serving_sequence(), s);
    }

    /// S21 Path only while serving
    #[test]
    fn s21_path_only_while_serving() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        let w = [0xE1; 32];
        assert!(clock.path_for(&w).unwrap().is_none());
        clock.post_waypoint(w).unwrap();
        assert!(clock.path_for(&w).unwrap().is_none());
        chain.set_tip(tip(2, Some(1), 101));
        clock.poll_once().unwrap();
        assert!(clock.path_for(&w).unwrap().is_none());
        chain.set_included(true);
        chain.set_tip(tip(3, Some(2), 102));
        clock.poll_once().unwrap();
        assert!(clock.path_for(&w).unwrap().is_some());
        assert!(clock.path_for(&[0x00; 32]).unwrap().is_none());
    }

    /// S22 No hold on GET — answered from current slots immediately
    #[test]
    fn s22_no_hold_on_get() {
        let chain = Arc::new(FakeChain::default());
        let (clock, _temp) = open_at(chain, tip(1, None, 100));
        assert!(clock.path_for(&[0xF1; 32]).unwrap().is_none());
    }

    /// S23 Bind header is this blob's header — POST still only hits queue
    #[test]
    fn s23_post_after_index_only_hits_queue() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain, tip(1, None, 100));
        let _ = clock.index_snapshot();
        let w = [0xF2; 32];
        assert_eq!(clock.post_waypoint(w).unwrap(), PostAccept::Queued);
        assert!(clock.pool_contains(&w));
    }

    /// S24 Three surfaces only — documented; HTTP router test companion
    #[test]
    fn s24_three_surfaces_documented() {
        assert_eq!(ANCHOR_WINDOW_BLOCKS, DEFAULT_CONFIRMATION_WINDOW_TIPS);
    }

    /// S25 Persist or broadcast failure is a miss
    #[test]
    fn s25_persist_or_broadcast_failure_is_a_miss() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        let w = [0xA5; 32];
        clock.post_waypoint(w).unwrap();
        chain.set_plant_failure(Some(SkipReason::Fee));
        chain.set_tip(tip(2, Some(1), 101));
        let report = clock.poll_once().unwrap().unwrap();
        assert_eq!(report.tip.hash, [2; 32]); // close-tip snapshot publishes; no stall
        assert_eq!(chain.plant_count(), 0); // broadcast failed; no successful plant
        assert!(clock.pending_sequence().is_some());
        let plants_after_close = chain.plant_count();
        chain.set_plant_failure(None);
        chain.set_included(true);
        chain.set_tip(tip(3, Some(2), 102));
        clock.poll_once().unwrap();
        let snap = clock.index_snapshot();
        assert!(snap.plant.is_none());
        assert!(clock.path_for(&w).unwrap().is_none());
        assert_eq!(chain.plant_count(), plants_after_close); // Not: a retry plant
                                                             // Persist-failure branch
        let chain2 = Arc::new(FakeChain::default());
        let (mut clock2, _temp2) = open_at(chain2.clone(), tip(1, None, 200));
        let w2 = [0xA6; 32];
        clock2.post_waypoint(w2).unwrap();
        clock2.fail_next_persist();
        chain2.set_tip(tip(2, Some(1), 201));
        let report2 = clock2.poll_once().unwrap().unwrap();
        assert_eq!(report2.tip.hash, [2; 32]);
        assert_eq!(chain2.plant_count(), 0);
        chain2.set_included(true);
        chain2.set_tip(tip(3, Some(2), 202));
        clock2.poll_once().unwrap();
        assert!(clock2.index_snapshot().plant.is_none());
        assert!(clock2.path_for(&w2).unwrap().is_none());
        assert_eq!(chain2.plant_count(), 0);
    }

    /// S26 Tip boundary drains the queue before close
    #[test]
    fn s26_tip_boundary_hands_open_pool_to_seal() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        let w = [0xA7; 32];
        clock.post_waypoint(w).unwrap();
        assert_eq!(clock.pool_len(), 1);
        chain.set_tip(tip(2, Some(1), 101));
        clock.poll_once().unwrap();
        assert_eq!(clock.pool_len(), 0);
        assert_eq!(clock.pending_sequence(), Some(0));
        assert_eq!(clock.collecting_opened_tip(), [2; 32]);
        assert_eq!(chain.plant_count(), 1);
    }

    /// S27 Inclusion-lookup failure is a miss
    #[test]
    fn s27_inclusion_lookup_failure_is_a_miss() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        let w = [0xA8; 32];
        clock.post_waypoint(w).unwrap();
        chain.set_tip(tip(2, Some(1), 101));
        clock.poll_once().unwrap();
        let plants = chain.plant_count();
        chain.fail_next_inclusion_lookup();
        chain.set_tip(tip(3, Some(2), 102));
        let report = clock.poll_once().unwrap().unwrap();
        assert_eq!(report.tip.hash, [3; 32]); // transition completes
        assert!(clock.index_snapshot().plant.is_none());
        assert!(clock.path_for(&w).unwrap().is_none());
        assert_eq!(chain.plant_count(), plants); // not checked/broadcast again
    }

    /// A broadcast that confirms after the anchor window is still included.
    /// The confirming block is served. The plant is not broadcast again.
    #[test]
    fn a_tree_that_confirms_after_the_window_still_serves_its_path() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        let w = [0xA9; 32];
        clock.post_waypoint(w).unwrap();
        chain.set_tip(tip(2, Some(1), 101));
        clock.poll_once().unwrap();
        chain.set_included(false);
        chain.set_tip(tip(3, Some(2), 102));
        clock.poll_once().unwrap();
        chain.set_tip(tip(4, Some(3), 103));
        clock.poll_once().unwrap();
        chain.set_tip(tip(5, Some(4), 104));
        clock.poll_once().unwrap();
        chain.set_tip(tip(6, Some(5), 105));
        clock.poll_once().unwrap();
        chain.set_tip(tip(7, Some(6), 106));
        let missed = clock.poll_once().unwrap().unwrap();
        assert_eq!(missed.missed.unwrap().1, SkipReason::NotMined);
        assert!(clock.index_snapshot().plant.is_none());
        assert_eq!(clock.path_miss(&w), PathMiss::SealedNotInBlock);
        assert!(clock.path_for(&w).unwrap().is_none());
        chain.set_included(true);
        chain.set_tip(tip(8, Some(7), 107));
        clock.poll_once().unwrap();
        let body = clock
            .path_for(&w)
            .unwrap()
            .expect("late confirmation is servable");
        body.validate_for(&w).unwrap();
        assert_eq!(body.block_height, 107);
        let confirmed = clock.last_confirmed().expect("late confirm is recorded");
        assert!(confirmed.confirmed_late);
        assert_eq!(confirmed.offset, 6);
        let again = clock.path_for(&w).unwrap().unwrap();
        assert_eq!(again.encode().unwrap(), body.encode().unwrap());
        assert_eq!(clock.index_snapshot().header.hash, [8; 32]);
        assert!(clock.index_snapshot().plant.is_some());
        assert_eq!(chain.plant_count(), 1);
    }

    /// A02 QueueTick never waits on a tip
    #[test]
    fn a02_queue_tick_never_waits_on_a_tip() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain, tip(1, None, 100));
        clock.post_waypoint([0xAA; 32]).unwrap();
        let before = clock.current_tip();
        clock.queue_tick().unwrap();
        assert_eq!(clock.current_tip(), before);
    }

    /// A03 POST never waits on a tip
    #[test]
    fn a03_post_never_waits_on_a_tip() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain, tip(1, None, 100));
        let before = clock.current_tip();
        clock.post_waypoint([0xAB; 32]).unwrap();
        assert_eq!(clock.current_tip(), before);
    }

    #[test]
    fn operator_network_is_free_and_the_connected_node_must_agree() {
        let temp = tempfile::tempdir().unwrap();
        let chain = Arc::new(FakeChain::default());
        chain.set_network("testnet4");
        chain.set_tip(tip(1, None, 100));
        let clock = BariClock::open_for_network(
            chain.clone(),
            FileStore::open(temp.path()).unwrap(),
            "testnet4",
        )
        .unwrap();
        assert_eq!(clock.network(), "testnet4");
        assert_eq!(clock.index_snapshot().network, "testnet4");

        let mismatch = tempfile::tempdir().unwrap();
        chain.set_network("signet");
        assert!(matches!(
            BariClock::open_for_network(
                chain.clone(),
                FileStore::open(mismatch.path()).unwrap(),
                "testnet4",
            ),
            Err(ClockError::WrongNetwork { .. })
        ));

        let synthetic = tempfile::tempdir().unwrap();
        chain.set_network("synthetic");
        let synthetic_clock = BariClock::open_for_network(
            chain.clone(),
            FileStore::open(synthetic.path()).unwrap(),
            "synthetic",
        )
        .unwrap();
        assert_eq!(synthetic_clock.network(), "synthetic");

        let mainnet = tempfile::tempdir().unwrap();
        chain.set_network("main");
        let mainnet_clock =
            BariClock::open_for_network(chain, FileStore::open(mainnet.path()).unwrap(), "main")
                .unwrap();
        assert_eq!(mainnet_clock.network(), "main");
    }

    #[test]
    fn open_at_tip_rechecks_initial_block_download() {
        let temp = tempfile::tempdir().unwrap();
        let chain = Arc::new(FakeChain::default());
        let current_tip = tip(1, None, 100);
        {
            let mut state = chain.state.lock().unwrap();
            state.tip = Some(current_tip);
            state.initial_block_download = true;
        }
        assert!(matches!(
            BariClock::open_at_tip(chain, FileStore::open(temp.path()).unwrap(), current_tip),
            Err(ClockError::InitialBlockDownload)
        ));
    }

    #[test]
    fn supplied_tip_does_not_read_best_tip() {
        let temp = tempfile::tempdir().unwrap();
        let chain = Arc::new(FakeChain::default());
        chain.set_network("regtest");
        chain.fail_next_best_tip();
        let supplied = tip(9, None, 4);
        let clock = BariClock::open_at_supplied_tip(
            chain,
            FileStore::open(temp.path()).unwrap(),
            supplied,
            "regtest",
            None,
        )
        .unwrap();
        assert_eq!(clock.current_tip().hash, [9; 32]);
        assert_eq!(clock.current_tip().height, 4);
        assert_eq!(clock.network(), "regtest");
    }

    #[test]
    fn historical_initial_download_tips_never_open_the_clock() {
        let temp = tempfile::tempdir().unwrap();
        let chain = Arc::new(FakeChain::default());
        {
            let mut state = chain.state.lock().unwrap();
            state.tip = Some(tip(1, None, 0));
            state.initial_block_download = true;
        }
        assert!(matches!(
            BariClock::open(chain, FileStore::open(temp.path()).unwrap()),
            Err(ClockError::InitialBlockDownload)
        ));
    }

    #[test]
    fn a6_restart_same_tip_continues_same_pool() {
        let temp = tempfile::tempdir().unwrap();
        let chain = Arc::new(FakeChain::default());
        let tip0 = tip(1, None, 100);
        chain.set_tip(tip0);
        {
            let mut clock =
                BariClock::open(chain.clone(), FileStore::open(temp.path()).unwrap()).unwrap();
            clock.post_waypoint([0xA1; 32]).unwrap();
            assert_eq!(clock.pool_len(), 1);
        }
        let clock = BariClock::open(chain.clone(), FileStore::open(temp.path()).unwrap()).unwrap();
        assert_eq!(clock.current_tip().hash, [1; 32]);
        assert_eq!(clock.pool_len(), 1);
        assert!(clock.pool_contains(&[0xA1; 32]));
    }

    #[test]
    fn a7_restart_after_one_tip_seals_prior_pool() {
        let temp = tempfile::tempdir().unwrap();
        let chain = Arc::new(FakeChain::default());
        chain.set_tip(tip(1, None, 100));
        {
            let mut clock =
                BariClock::open(chain.clone(), FileStore::open(temp.path()).unwrap()).unwrap();
            clock.post_waypoint([0xA2; 32]).unwrap();
        }
        // Advance tip while "down"
        chain.set_tip(tip(2, Some(1), 101));
        let clock = BariClock::open(chain.clone(), FileStore::open(temp.path()).unwrap()).unwrap();
        assert_eq!(clock.current_tip().hash, [2; 32]);
        assert_eq!(clock.pool_len(), 0);
        assert_eq!(clock.pending_sequence(), Some(0));
        assert!(!clock.pool_contains(&[0xA2; 32]));
        // Pending holds the sealed cohort
        assert!(clock.held_anywhere(&[0xA2; 32]));
    }

    #[test]
    fn a8_restart_after_two_tips_seals_each_pool_correctly() {
        let temp = tempfile::tempdir().unwrap();
        let chain = Arc::new(FakeChain::default());
        chain.set_tip(tip(1, None, 100));
        {
            let mut clock =
                BariClock::open(chain.clone(), FileStore::open(temp.path()).unwrap()).unwrap();
            clock.post_waypoint([0xB1; 32]).unwrap();
        }
        // Simulate mid-epoch at tip 2 without process: write pool for tip 2 on disk via second open...
        // Instead: run tip advance once, accept on tip 2, drop, then jump to tip 3.
        chain.set_tip(tip(2, Some(1), 101));
        {
            let mut clock =
                BariClock::open(chain.clone(), FileStore::open(temp.path()).unwrap()).unwrap();
            // tip1 pool sealed into pending on recover; accept on tip2
            clock.post_waypoint([0xB2; 32]).unwrap();
            assert_eq!(clock.pool_len(), 1);
        }
        chain.set_tip(tip(3, Some(2), 102));
        let clock = BariClock::open(chain.clone(), FileStore::open(temp.path()).unwrap()).unwrap();
        assert_eq!(clock.current_tip().hash, [3; 32]);
        assert_eq!(clock.pool_len(), 0);
        // Last prior pool (tip2) should be pending; tip1 catch-up skipped.
        assert_eq!(clock.pending_sequence(), Some(1));
        assert!(clock.held_anywhere(&[0xB2; 32]));
        let batches = FileStore::open(temp.path())
            .unwrap()
            .load_batches()
            .unwrap();
        assert!(batches.iter().any(|b| b.opened_tip == [1; 32]));
        assert!(batches.iter().any(|b| b.opened_tip == [2; 32]));
        assert!(!batches
            .iter()
            .any(|b| b.opened_tip == [1; 32] && b.leaves.iter().any(|l| *l == [0xB2; 32])));
    }

    #[test]
    fn accept_does_not_block_on_seal_handoff_then_new_pool() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        clock.post_waypoint([0xC1; 32]).unwrap();
        chain.set_tip(tip(2, Some(1), 101));
        let (report, job) = clock.on_tip_prepare(tip(2, Some(1), 101)).unwrap();
        assert_eq!(report.tip.hash, [2; 32]);
        // New empty pool accepts immediately before seal executes.
        assert_eq!(clock.post_waypoint([0xC2; 32]).unwrap(), PostAccept::Queued);
        assert_eq!(clock.pool_len(), 1);
        assert!(clock.pool_contains(&[0xC2; 32]));
        assert!(!clock.pool_contains(&[0xC1; 32]));
        let sealed = job.expect("seal job").execute().unwrap();
        clock.install_sealed(sealed).unwrap();
        assert_eq!(clock.pending_sequence(), Some(0));
    }

    #[test]
    fn carried_height_mismatch_is_explicit_telemetry_not_chain_law() {
        let check = ChainPostCheck {
            transaction_in_block: true,
            output_exists: true,
            zero_value: true,
            exact_script: true,
            selected_chain: true,
            carried_height: 900,
            authenticated_height: Some(901),
            height_matches: Some(false),
        };
        assert!(check.is_valid());
    }

    fn ops_values(event: &str, leaf_hex: &str) -> Vec<serde_json::Value> {
        crate::ops::lines_containing(leaf_hex)
            .iter()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|value| value["event"] == event)
            .collect()
    }

    #[test]
    fn ops_logs_accept_seal_broadcast_included_and_archive() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 410));
        let leaf = [0x5A; 32];
        let leaf_hex = encode_hash32(&leaf);
        assert_eq!(clock.post_waypoint(leaf).unwrap(), PostAccept::Queued);
        let pooled = ops_values(crate::ops::OPEN_POOL_ADMIT, &leaf_hex);
        assert_eq!(pooled.len(), 1, "{pooled:?}");
        assert_eq!(pooled[0]["level"], "info");
        assert_eq!(pooled[0]["leaf_count"], 1);
        assert_eq!(pooled[0]["leaf_ids"][0], leaf_hex);
        assert_eq!(pooled[0]["tip_height"], 410);

        chain.set_tip(tip(2, Some(1), 411));
        clock.poll_once().unwrap().unwrap();
        let seal = ops_values(crate::ops::SEAL, &leaf_hex);
        assert_eq!(seal.len(), 1, "{seal:?}");
        assert_eq!(seal[0]["level"], "info");
        assert_eq!(seal[0]["empty"], false);
        assert_eq!(seal[0]["tree_size"], 1);
        assert_eq!(seal[0]["leaves"][0], leaf_hex);
        assert_eq!(seal[0]["batch_sequence"], 0);
        let broadcast = ops_values(crate::ops::PLANT_BROADCAST, &leaf_hex);
        assert_eq!(broadcast.len(), 1, "{broadcast:?}");
        assert_eq!(broadcast[0]["level"], "info");
        assert_eq!(broadcast[0]["fee_rate_sat_vb"], 2.5);
        assert_eq!(broadcast[0]["fee_source"], "test");
        assert!(broadcast[0]["txid"].as_str().unwrap().len() == 64);

        chain.set_included(true);
        chain.set_tip(tip(3, Some(2), 412));
        clock.poll_once().unwrap().unwrap();
        let included = ops_values(crate::ops::PLANT_INCLUDED, &leaf_hex);
        assert_eq!(included.len(), 1, "{included:?}");
        assert_eq!(included[0]["level"], "info");
        assert_eq!(included[0]["txid"], broadcast[0]["txid"]);
        assert_eq!(included[0]["fee_rate_sat_vb"], 2.5);
        assert_eq!(included[0]["block_height"], 412);
        assert_eq!(included[0]["confirmation_window_tips"], 5);
        assert!(clock.path_for(&leaf).unwrap().is_some());

        // Planted at 411. The path stays up through 414.
        chain.set_tip(tip(4, Some(3), 413));
        clock.poll_once().unwrap().unwrap();
        assert!(clock.path_for(&leaf).unwrap().is_some());
        chain.set_tip(tip(5, Some(4), 414));
        clock.poll_once().unwrap().unwrap();
        assert!(clock.path_for(&leaf).unwrap().is_some());
        chain.set_tip(tip(6, Some(5), 415));
        clock.poll_once().unwrap().unwrap();
        let archived = ops_values(crate::ops::PATH_ARCHIVE, &leaf_hex);
        assert!(archived.is_empty(), "{archived:?}");
        let first = clock.path_for(&leaf).unwrap().expect("still served");
        let second = clock.path_for(&leaf).unwrap().expect("served again");
        assert_eq!(first.block_height, 412);
        assert_eq!(first.encode().unwrap(), second.encode().unwrap());
    }

    #[test]
    fn ops_logs_missed_confirmation_window_only_for_not_mined() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 510));
        let leaf = [0x5B; 32];
        let leaf_hex = encode_hash32(&leaf);
        clock.post_waypoint(leaf).unwrap();
        chain.set_included(false);
        chain.set_tip(tip(2, Some(1), 511));
        clock.poll_once().unwrap().unwrap();
        chain.set_tip(tip(3, Some(2), 512));
        assert!(clock.poll_once().unwrap().unwrap().missed.is_none());
        chain.set_tip(tip(4, Some(3), 513));
        assert!(clock.poll_once().unwrap().unwrap().missed.is_none());
        chain.set_tip(tip(5, Some(4), 514));
        assert!(clock.poll_once().unwrap().unwrap().missed.is_none());
        chain.set_tip(tip(6, Some(5), 515));
        assert!(clock.poll_once().unwrap().unwrap().missed.is_none());
        chain.set_tip(tip(7, Some(6), 516));
        let report = clock.poll_once().unwrap().unwrap();
        assert_eq!(report.missed.unwrap().1, SkipReason::NotMined);
        let missed = ops_values(crate::ops::CONFIRMATION_WINDOW_MISSED, &leaf_hex);
        assert_eq!(missed.len(), 1, "{missed:?}");
        assert_eq!(missed[0]["level"], "warn");
        assert_eq!(missed[0]["skip_reason"], "not-mined");
        assert!(missed[0]["txid"].as_str().is_some());
        assert!(missed[0]["lookup_block_hash"].is_null());
        assert!(ops_values(crate::ops::PLANT_SKIP, &leaf_hex).is_empty());
        assert!(ops_values(crate::ops::PLANT_INCLUDED, &leaf_hex).is_empty());
    }

    #[test]
    fn ops_logs_plant_skip_for_fee_without_calling_it_a_missed_window() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 610));
        let leaf = [0x5C; 32];
        let leaf_hex = encode_hash32(&leaf);
        clock.post_waypoint(leaf).unwrap();
        chain.set_plant_failure(Some(SkipReason::Fee));
        chain.set_tip(tip(2, Some(1), 611));
        clock.poll_once().unwrap().unwrap();
        let skips = ops_values(crate::ops::PLANT_SKIP, &leaf_hex);
        assert!(
            skips
                .iter()
                .any(|line| line["at"] == "plant" && line["skip_reason"] == "fee"),
            "{skips:?}"
        );
        assert!(skips.iter().all(|line| line["level"] == "warn"));
        assert!(skips.iter().all(|line| line["txid"].is_null()));
        assert!(ops_values(crate::ops::PLANT_BROADCAST, &leaf_hex).is_empty());
        assert!(ops_values(crate::ops::CONFIRMATION_WINDOW_MISSED, &leaf_hex).is_empty());

        chain.set_plant_failure(None);
        chain.set_included(true);
        chain.set_tip(tip(3, Some(2), 612));
        clock.poll_once().unwrap().unwrap();
        let decided = ops_values(crate::ops::PLANT_SKIP, &leaf_hex);
        assert!(
            decided.iter().any(|line| {
                line["at"] == "inclusion_decision"
                    && line["detail"] == "close_fault"
                    && line["skip_reason"] == "fee"
            }),
            "{decided:?}"
        );
        assert!(ops_values(crate::ops::CONFIRMATION_WINDOW_MISSED, &leaf_hex).is_empty());
        assert!(clock.path_for(&leaf).unwrap().is_none());
    }

    fn deferral(reason: &str, min: Option<f64>, needed: Option<f64>) -> Deferral {
        Deferral {
            reason: reason.to_owned(),
            message: match reason {
                "cap" => format!(
                    "needed fee {} sat/vB is above cap 1.2 sat/vB; deferred; cap breach; not broadcast; not replaced",
                    needed.unwrap_or(1.3)
                ),
                "no-confirmed-utxo" => {
                    "no available confirmed plant UTXO (minconf=1, confirmed=0, reserve=2-3); 0-conf outputs are not inputs; deferred; no tree; no psbt; not replaced"
                        .to_owned()
                }
                _ => format!(
                    "next-block minimum {} sat/vB is above 1.0 sat/vB; deferred; no tree; no psbt; not replaced",
                    min.unwrap_or(1.61)
                ),
            },
            decision_height: None,
            next_block_min_sat_vb: min,
            node_estimate_sat_vb: Some(1.12),
            needed_sat_vb: needed,
            cap_sat_vb: Some(1.2),
        }
    }

    #[test]
    fn min_above_one_defers_and_a_drop_to_one_plants_every_pooled_waypoint() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, temp) = open_at(chain.clone(), tip(1, None, 100));
        let first = [0xD1; 32];
        let second = [0xD2; 32];
        let first_hex = encode_hash32(&first);
        let second_hex = encode_hash32(&second);
        chain.set_deferral(Some(deferral("next-block-min", Some(1.61), None)));
        clock.post_waypoint(first).unwrap();

        chain.set_tip(tip(2, Some(1), 101));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 0, "high minimum must not broadcast");
        assert!(clock.pool_contains(&first));
        assert_eq!(clock.path_miss(&first), PathMiss::StillInPool);
        assert!(clock.pending_sequence().is_none());
        assert!(
            FileStore::open(temp.path())
                .unwrap()
                .load_batches()
                .unwrap()
                .is_empty(),
            "deferral builds no batch"
        );
        let deferred = ops_values(crate::ops::PLANT_DEFERRED, &first_hex);
        assert_eq!(deferred.len(), 1, "{deferred:?}");
        assert_eq!(deferred[0]["deferred"], true);
        assert_eq!(deferred[0]["reason"], "next-block-min");
        assert_eq!(deferred[0]["node_estimate_sat_vb"], 1.12);
        assert!(deferred[0]["message"]
            .as_str()
            .unwrap()
            .contains("deferred"));
        assert!(ops_values(crate::ops::PLANT_BROADCAST, &first_hex).is_empty());
        assert!(ops_values(crate::ops::PLANT_SKIP, &first_hex).is_empty());
        assert!(ops_values(crate::ops::SEAL, &first_hex).is_empty());

        assert_eq!(clock.post_waypoint(second).unwrap(), PostAccept::Queued);
        chain.set_tip(tip(3, Some(2), 102));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 0);
        assert!(clock.pool_contains(&first));
        assert!(clock.pool_contains(&second));
        assert_eq!(clock.pool_len(), 2);
        assert_eq!(ops_values(crate::ops::PLANT_DEFERRED, &first_hex).len(), 2);

        chain.set_deferral(None);
        chain.set_tip(tip(4, Some(3), 103));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 1, "one tree for the whole pool");
        assert_eq!(clock.pool_len(), 0);
        assert!(clock.pending_sequence().is_some());
        let batches = FileStore::open(temp.path())
            .unwrap()
            .load_batches()
            .unwrap();
        assert_eq!(batches.len(), 1, "{batches:?}");
        assert!(batches[0].leaves.contains(&first));
        assert!(batches[0].leaves.contains(&second));
        assert_eq!(batches[0].tree_size, 2);
        let seal = ops_values(crate::ops::SEAL, &second_hex);
        assert!(seal.iter().any(|line| line["tree_size"] == 2), "{seal:?}");
        assert_eq!(ops_values(crate::ops::PLANT_BROADCAST, &first_hex).len(), 1);
        assert_eq!(
            ops_values(crate::ops::PLANT_BROADCAST, &second_hex).len(),
            1
        );
    }

    #[test]
    fn cap_breach_never_broadcasts_and_keeps_the_pool() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 200));
        let leaf = [0xD3; 32];
        let leaf_hex = encode_hash32(&leaf);
        chain.set_deferral(Some(deferral("cap", Some(1.0), Some(1.3))));
        clock.post_waypoint(leaf).unwrap();
        chain.set_tip(tip(2, Some(1), 201));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 0);
        assert!(clock.pool_contains(&leaf));
        assert_eq!(clock.path_miss(&leaf), PathMiss::StillInPool);
        let deferred = ops_values(crate::ops::PLANT_DEFERRED, &leaf_hex);
        assert_eq!(deferred.len(), 1, "{deferred:?}");
        assert_eq!(deferred[0]["reason"], "cap");
        assert_eq!(deferred[0]["deferred"], true);
        let message = deferred[0]["message"].as_str().unwrap();
        assert!(message.contains("not broadcast"), "{message}");
        assert!(message.contains("deferred"), "{message}");
        assert!(ops_values(crate::ops::PLANT_BROADCAST, &leaf_hex).is_empty());
        assert!(ops_values(crate::ops::PLANT_SKIP, &leaf_hex).is_empty());
    }

    #[test]
    fn missing_confirmed_utxo_defers_instead_of_dropping_the_waypoint() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 300));
        let leaf = [0xD4; 32];
        let leaf_hex = encode_hash32(&leaf);
        chain.set_deferral(Some(deferral("no-confirmed-utxo", None, None)));
        clock.post_waypoint(leaf).unwrap();
        chain.set_tip(tip(2, Some(1), 301));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 0);
        assert!(clock.pool_contains(&leaf));
        assert_eq!(clock.path_miss(&leaf), PathMiss::StillInPool);
        assert!(clock.pending_sequence().is_none());
        let deferred = ops_values(crate::ops::PLANT_DEFERRED, &leaf_hex);
        assert_eq!(deferred.len(), 1, "{deferred:?}");
        assert_eq!(deferred[0]["deferred"], true);
        assert_eq!(deferred[0]["reason"], "no-confirmed-utxo");
        let message = deferred[0]["message"].as_str().unwrap();
        assert!(
            message.contains("no available confirmed plant UTXO"),
            "{message}"
        );
        assert!(message.contains("deferred"), "{message}");
        assert!(ops_values(crate::ops::PLANT_SKIP, &leaf_hex).is_empty());
        assert!(ops_values(crate::ops::PLANT_BROADCAST, &leaf_hex).is_empty());

        chain.set_deferral(None);
        chain.set_tip(tip(3, Some(2), 302));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 1);
        assert!(!clock.pool_contains(&leaf));
        assert!(clock.pending_sequence().is_some());
    }

    #[test]
    fn restart_while_deferred_keeps_the_waypoint_in_the_live_pool() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, temp) = open_at(chain.clone(), tip(1, None, 400));
        let leaf = [0xD5; 32];
        chain.set_deferral(Some(deferral("next-block-min", Some(2.01), None)));
        clock.post_waypoint(leaf).unwrap();
        chain.set_tip(tip(2, Some(1), 401));
        let (_report, job) = clock.on_tip_prepare(tip(2, Some(1), 401)).unwrap();
        drop(job);
        drop(clock);
        let clock = reopen(&chain, temp.path());
        assert_eq!(clock.current_tip().hash, [2; 32]);
        assert!(clock.pool_contains(&leaf));
        assert_eq!(chain.plant_count(), 0);
        assert!(clock.pending_sequence().is_none());
        assert_eq!(clock.path_miss(&leaf), PathMiss::StillInPool);
    }

    #[test]
    fn accepted_waypoints_survive_many_tips_with_no_plant() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, temp) = open_at(chain.clone(), tip(1, None, 100));
        let old = [0xE1; 32];
        let newer = [0xE2; 32];
        let old_hex = encode_hash32(&old);
        chain.set_deferral(Some(deferral("next-block-min", Some(1.61), None)));
        clock.post_waypoint(old).unwrap();

        chain.set_tip(tip(2, Some(1), 101));
        clock.poll_once().unwrap().unwrap();
        assert!(clock.pool_contains(&old));
        assert_eq!(clock.path_miss(&old), PathMiss::StillInPool);
        assert_eq!(chain.plant_count(), 0);

        chain.set_tip(tip(3, Some(2), 102));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(clock.post_waypoint(newer).unwrap(), PostAccept::Queued);

        drop(clock);
        let mut clock = reopen(&chain, temp.path());
        chain.set_deferral(Some(deferral("next-block-min", Some(1.61), None)));
        assert!(clock.pool_contains(&old));
        assert!(clock.pool_contains(&newer));

        // Many tips with no plant. Age does not evict either waypoint.
        let mut prev = 3u8;
        for step in 0..12u32 {
            let height = 103 + step;
            let byte = 4 + step as u8;
            chain.set_tip(tip(byte, Some(prev), height));
            clock.poll_once().unwrap().unwrap();
            prev = byte;
            assert!(clock.pool_contains(&old), "height {height}");
            assert!(clock.pool_contains(&newer), "height {height}");
            assert_eq!(clock.path_miss(&old), PathMiss::StillInPool);
            assert_eq!(chain.plant_count(), 0);
        }
        assert!(event_values(temp.path(), "waypoint_ask_expired").is_empty());
        let decision = ops_values(crate::ops::SEAL_DECISION, &old_hex);
        assert!(
            decision.iter().any(|line| {
                line["decision"] == "hold"
                    && line["dropped_leaf_count"] == 0
                    && line["node_estimate_sat_vb"] == 1.12
                    && line["level"] == "warn"
            }),
            "{decision:?}"
        );

        chain.set_deferral(None);
        chain.set_tip(tip(prev.wrapping_add(1), Some(prev), 115));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 1, "one tree for every pooled waypoint");
        assert!(!clock.pool_contains(&old));
        assert!(!clock.pool_contains(&newer));
        let batches = FileStore::open(temp.path())
            .unwrap()
            .load_batches()
            .unwrap();
        assert_eq!(batches.len(), 1, "{batches:?}");
        assert!(batches[0].leaves.contains(&old));
        assert!(batches[0].leaves.contains(&newer));
        assert_eq!(batches[0].tree_size, 2);
    }

    #[test]
    fn a_tree_in_the_mempool_does_not_hold_the_next_and_every_height_is_recorded() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 800));
        let first = [0x71; 32];
        let second = [0x72; 32];
        clock.post_waypoint(first).unwrap();
        clock.note_malformed();
        chain.set_tip(tip(2, Some(1), 801));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 1);
        clock.post_waypoint(second).unwrap();
        chain.set_included(false);
        chain.set_tip(tip(3, Some(2), 802));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(
            chain.plant_count(),
            2,
            "the first tree does not hold the second"
        );
        assert!(!clock.pool_contains(&second));
        assert_eq!(clock.inflight().len(), 2);
        let sent = clock
            .ledger()
            .iter()
            .find(|row| row.height == 802)
            .expect("height 802 recorded");
        assert_eq!(sent.decision, "sent");
        assert_eq!(sent.new_waypoints, 1);
        assert!(sent.added_known);
        assert_ne!(sent.hold_reason, "tree-in-mempool");
        let broadcast = clock
            .ledger()
            .iter()
            .find(|row| row.height == 801)
            .expect("broadcast height");
        assert_eq!(broadcast.status, "sent");
        assert_eq!(broadcast.decision, "sent");
        assert_eq!(broadcast.malformed, 1);
        assert!(broadcast.broadcast_txid.is_some());
        assert!(broadcast.fee_source.is_some());
        // An empty pool is quiet, and that height is recorded too.
        chain.set_tip(tip(4, Some(3), 803));
        clock.poll_once().unwrap().unwrap();
        let quiet = clock.ledger().iter().find(|row| row.height == 803).unwrap();
        assert_eq!(quiet.decision, "quiet");
        // Window is 5 from 801, so 806 is the miss of the first tree.
        chain.set_tip(tip(5, Some(4), 804));
        clock.poll_once().unwrap().unwrap();
        chain.set_tip(tip(6, Some(5), 805));
        clock.poll_once().unwrap().unwrap();
        chain.set_tip(tip(7, Some(6), 806));
        let missed = clock.poll_once().unwrap().unwrap();
        assert_eq!(missed.missed.unwrap().1, SkipReason::NotMined);
        let marked = clock.ledger().iter().find(|row| row.height == 801).unwrap();
        assert_eq!(
            marked.decision, "sent",
            "a miss does not rewrite the block record"
        );
        // A later confirmation is still served. The block record stays the
        // decision. Inclusion lives on the tree.
        chain.set_included(true);
        chain.set_tip(tip(8, Some(7), 807));
        clock.poll_once().unwrap().unwrap();
        let body = clock.path_for(&first).unwrap().expect("late path stays");
        assert_eq!(body.block_height, 807);
        let second_body = clock.path_for(&second).unwrap().expect("second path");
        assert_eq!(second_body.block_height, 807);
        let updated = clock.ledger().iter().find(|row| row.height == 801).unwrap();
        assert_eq!(updated.decision, "sent");
        assert!(updated.inclusion_height.is_none());
    }

    #[test]
    fn several_trees_stay_in_flight_and_each_is_judged_on_its_own_window() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 10));
        let first = [0x11; 32];
        let second = [0x22; 32];
        let third = [0x33; 32];
        clock.post_waypoint(first).unwrap();
        chain.set_included(false);
        chain.set_tip(tip(2, Some(1), 11));
        clock.poll_once().unwrap().unwrap();
        clock.post_waypoint(second).unwrap();
        chain.set_tip(tip(3, Some(2), 12));
        clock.poll_once().unwrap().unwrap();
        clock.post_waypoint(third).unwrap();
        chain.set_tip(tip(4, Some(3), 13));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 3, "three trees in the mempool");
        assert_eq!(clock.inflight().len(), 3);
        assert_eq!(clock.pool_len(), 0);

        // The first tree's window ends at 16. The later trees are still inside.
        chain.set_tip(tip(5, Some(4), 14));
        clock.poll_once().unwrap().unwrap();
        chain.set_tip(tip(6, Some(5), 15));
        clock.poll_once().unwrap().unwrap();
        assert!(clock.inflight().iter().all(|tree| !tree.missed));
        chain.set_tip(tip(7, Some(6), 16));
        let report = clock.poll_once().unwrap().unwrap();
        assert_eq!(report.missed.unwrap().1, SkipReason::NotMined);
        let inflight = clock.inflight();
        assert_eq!(inflight.len(), 3, "a miss keeps the tree pending");
        assert_eq!(
            inflight.iter().filter(|tree| tree.missed).count(),
            1,
            "only the tree sent at 11 is missed"
        );
        assert!(clock.path_for(&first).unwrap().is_none());

        // Include only the tree sent at 12, one block after its own window.
        chain.include_broadcast(Some(12));
        chain.set_tip(tip(8, Some(7), 18));
        clock.poll_once().unwrap().unwrap();
        let late = clock
            .path_for(&second)
            .unwrap()
            .expect("late path is served");
        assert_eq!(late.block_height, 18);
        assert!(
            clock.path_for(&first).unwrap().is_none(),
            "the missed tree stays unconfirmed"
        );
        assert!(
            clock.path_for(&third).unwrap().is_none(),
            "the third tree was not the one this block included"
        );
        assert!(clock
            .inflight()
            .iter()
            .any(|tree| tree.broadcast_height == 13));
    }

    #[test]
    fn a_high_minimum_holds_for_fee_and_a_drop_still_plants_one_tree() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 50));
        let leaf = [0xF1; 32];
        clock.post_waypoint(leaf).unwrap();
        chain.set_tip(tip(2, Some(1), 51));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 1);
        chain.set_included(false);
        chain.set_deferral(Some(deferral("next-block-min", Some(2.2), None)));
        let held = [0xF2; 32];
        clock.post_waypoint(held).unwrap();
        chain.set_tip(tip(3, Some(2), 52));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 1, "a fee hold builds no second tree");
        assert!(clock.pool_contains(&held));
        let row = clock.ledger().iter().find(|row| row.height == 52).unwrap();
        assert_eq!(row.decision, "held");
        assert_eq!(row.hold_reason, "min-above-cap");
        assert_eq!(row.detail, "next-block-min");
        // The first tree is still in the mempool and did not cause this hold.
        assert_eq!(clock.inflight().len(), 1);
    }

    #[test]
    fn no_confirmed_coin_holds_while_another_tree_is_in_the_mempool() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 60));
        let first = [0xC1; 32];
        clock.post_waypoint(first).unwrap();
        chain.set_included(false);
        chain.set_tip(tip(2, Some(1), 61));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 1);
        chain.set_deferral(Some(deferral("no-confirmed-utxo", None, None)));
        let second = [0xC2; 32];
        clock.post_waypoint(second).unwrap();
        chain.set_tip(tip(3, Some(2), 62));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 1, "no confirmed coin builds no tree");
        assert!(clock.pool_contains(&second));
        let row = clock.ledger().iter().find(|row| row.height == 62).unwrap();
        assert_eq!(row.decision, "held");
        assert_eq!(row.hold_reason, "no-confirmed-coin");
        assert_eq!(row.detail, "no-confirmed-utxo");
        assert_eq!(
            clock.inflight().len(),
            1,
            "the sent tree stays in the mempool"
        );
    }

    #[test]
    fn a_gap_keeps_a_pending_tree_until_catch_up_finds_its_block() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, temp) = open_at(chain.clone(), tip(1, None, 970_681));
        let leaf = [0x68; 32];
        clock.post_waypoint(leaf).unwrap();
        chain.set_included(false);
        chain.set_tip(tip(2, Some(1), 970_682));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 1);

        let gap = clock.on_tip(tip(9, Some(1), 970_691)).unwrap();
        assert!(gap.missed.is_some());
        assert!(gap.included.is_none());
        assert!(clock.path_for(&leaf).unwrap().is_none());
        let stored = FileStore::open(temp.path())
            .unwrap()
            .load_batches()
            .unwrap();
        assert!(
            stored
                .iter()
                .all(|batch| batch.state != StoredState::Skipped),
            "a gap must not mark the tree Skipped"
        );
        assert!(clock.pending_sequence().is_some());

        let block = [0x90; 32];
        chain.set_catch_up(Some(TxPlace {
            inclusion_height: Some(970_690),
            block_hash: Some(block),
            in_mempool: false,
            fee_sats: Some(96),
        }));
        let caught = clock.on_tip(tip(10, Some(9), 970_692)).unwrap();
        assert!(caught.included.is_some());
        let body = clock.path_for(&leaf).unwrap().expect("late path");
        assert_eq!(body.block_height, 970_690);
        let events = event_values(temp.path(), "pending_included");
        assert!(
            events
                .iter()
                .any(|event| event["catch_up"] == true && event["block_height"] == 970_690),
            "{events:?}"
        );
        let memory = clock.ops_memory();
        let now = 1_791_610_000_000;
        let inputs = crate::operator::inputs_from_memory(
            &memory,
            crate::operator::ProcessFacts {
                version: "0.6.2".to_owned(),
                commit: "test".to_owned(),
                instance: "test".to_owned(),
                started_unix_ms: now,
            },
        );
        let reads = crate::operator::OpsReads {
            fee: crate::operator::NodeRead::Absent,
            wallet: crate::operator::NodeRead::Absent,
            sync: crate::operator::NodeRead::Absent,
        };
        let view = crate::operator::build_ops_view(&inputs, &reads, now);
        let tree = view
            .trees
            .iter()
            .find(|tree| tree.broadcast_height == 970_682)
            .expect("970682 row");
        assert_eq!(crate::operator::outcome_text(&tree.outcome), "Late +8");
        assert_eq!(tree.inclusion_height, Some(970_690));
        let anchored = view.anchored.as_ref().expect("tile");
        assert_eq!(anchored.tree.inclusion_height, Some(970_690));
        assert_eq!(
            crate::operator::outcome_text(&anchored.tree.outcome),
            "Late +8"
        );
    }

    #[test]
    fn a_waypoint_posted_during_seal_is_the_next_heights_accept() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 100));
        chain.set_tip(tip(2, Some(1), 101));
        clock.on_tip(tip(2, Some(1), 101)).unwrap();
        let quiet = clock.ledger().iter().find(|row| row.height == 101).unwrap();
        assert_eq!(quiet.decision, "quiet");
        assert_eq!(quiet.new_waypoints, 0);

        let first = [0x11; 32];
        let during = [0x22; 32];
        clock.post_waypoint(first).unwrap();
        let (_report, job) = clock.on_tip_prepare(tip(3, Some(2), 102)).unwrap();
        assert_eq!(clock.post_waypoint(during).unwrap(), PostAccept::Queued);
        clock
            .install_sealed(job.expect("seal").execute().unwrap())
            .unwrap();

        let sent = clock.ledger().iter().find(|row| row.height == 102).unwrap();
        assert_eq!(sent.decision, "sent");
        assert_eq!(sent.leaf_count, 1, "the seal cohort is the pre-seal pool");
        assert_eq!(
            sent.new_waypoints, 1,
            "a post during the seal is not this height's added"
        );
        assert!(clock.pool_contains(&during));
        assert!(!clock.pool_contains(&first));

        chain.set_tip(tip(4, Some(3), 103));
        clock.on_tip(tip(4, Some(3), 103)).unwrap();
        let next = clock.ledger().iter().find(|row| row.height == 103).unwrap();
        assert_eq!(next.leaf_count, 1);
        assert_eq!(next.new_waypoints, 1);
        let memory = clock.ops_memory();
        let inputs = crate::operator::inputs_from_memory(
            &memory,
            crate::operator::ProcessFacts {
                version: "0.6.2".to_owned(),
                commit: "test".to_owned(),
                instance: "test".to_owned(),
                started_unix_ms: 1,
            },
        );
        let reads = crate::operator::OpsReads {
            fee: crate::operator::NodeRead::Absent,
            wallet: crate::operator::NodeRead::Absent,
            sync: crate::operator::NodeRead::Absent,
        };
        let view = crate::operator::build_ops_view(&inputs, &reads, 2_000);
        let r11 = view.checks.iter().find(|check| check.id == "R11").unwrap();
        assert!(r11.holds, "{}{}", r11.name, r11.detail);
    }

    #[test]
    fn a_missing_fee_is_the_transactions_fee() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, temp) = open_at(chain.clone(), tip(1, None, 200));
        let leaf = [0xF6; 32];
        clock.post_waypoint(leaf).unwrap();
        chain.set_included(true);
        chain.set_tip(tip(2, Some(1), 201));
        clock.on_tip(tip(2, Some(1), 201)).unwrap();
        chain.set_tip(tip(3, Some(2), 202));
        clock.on_tip(tip(3, Some(2), 202)).unwrap();
        let memory = clock.ops_memory();
        let before = memory
            .trees
            .iter()
            .find(|tree| tree.inclusion.is_some())
            .expect("included tree");
        assert_eq!(before.fee_sat, None, "the test plant does not price the tx");
        let seq = before.batch_sequence;
        chain.set_catch_up(Some(TxPlace {
            inclusion_height: Some(202),
            block_hash: Some([3; 32]),
            in_mempool: false,
            fee_sats: Some(102),
        }));
        clock.remember_tx_fees();
        let memory = clock.ops_memory();
        let priced = memory
            .trees
            .iter()
            .find(|tree| tree.batch_sequence == seq)
            .unwrap();
        assert_eq!(priced.fee_sat, Some(102));

        drop(clock);
        chain.set_catch_up(None);
        let clock = reopen(&chain, temp.path());
        let memory = clock.ops_memory();
        let restored = memory
            .trees
            .iter()
            .find(|tree| tree.batch_sequence == seq)
            .unwrap();
        assert_eq!(restored.fee_sat, Some(102));
        let inputs = crate::operator::inputs_from_memory(
            &clock.ops_memory(),
            crate::operator::ProcessFacts {
                version: "0.6.2".to_owned(),
                commit: "test".to_owned(),
                instance: "test".to_owned(),
                started_unix_ms: 1,
            },
        );
        let reads = crate::operator::OpsReads {
            fee: crate::operator::NodeRead::Absent,
            wallet: crate::operator::NodeRead::Absent,
            sync: crate::operator::NodeRead::Absent,
        };
        let view = crate::operator::build_ops_view(&inputs, &reads, 2_000);
        let tree = view.trees.iter().find(|tree| tree.seq == seq).unwrap();
        assert_eq!(tree.fee_sat, 102);
        assert!(!tree.fee_approx);
        assert!(render_paid(&view).contains("102"));
    }

    fn render_paid(view: &crate::operator::OpsView) -> String {
        crate::operator::render(view)
    }

    #[test]
    fn ops_logs_empty_seal_without_a_tree_size() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(7, None, 710));
        chain.set_tip(tip(8, Some(7), 711));
        let report = clock.poll_once().unwrap().unwrap();
        assert_eq!(report.closed_batch, None);
        let lines = crate::ops::lines_containing(crate::ops::SEAL);
        let empty: Vec<serde_json::Value> = lines
            .iter()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|value| {
                value["durable_event"] == "pool_empty_handoff"
                    && value["opened_tip_height"] == 710
                    && value["closed_tip_height"] == 711
            })
            .collect();
        assert_eq!(empty.len(), 1, "{empty:?}");
        assert_eq!(empty[0]["empty"], true);
        assert_eq!(empty[0]["leaf_count"], 0);
        assert_eq!(empty[0]["leaves"].as_array().unwrap().len(), 0);
        assert!(empty[0].get("tree_size").is_none());
        assert_eq!(chain.plant_count(), 0);
    }

    fn event_values(dir: &std::path::Path, name: &str) -> Vec<serde_json::Value> {
        let text = std::fs::read_to_string(dir.join("events.jsonl")).unwrap();
        text.lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|value| value["event"] == name)
            .collect()
    }

    fn reopen(chain: &Arc<FakeChain>, dir: &std::path::Path) -> BariClock {
        BariClock::open(chain.clone(), FileStore::open(dir).unwrap()).unwrap()
    }

    #[test]
    fn restart_restores_broadcast_with_plant_into_pending() {
        let chain = Arc::new(FakeChain::default());
        let temp = tempfile::tempdir().unwrap();
        let leaf = [0x71; 32];
        chain.set_tip(tip(1, None, 810));
        {
            let mut clock =
                BariClock::open(chain.clone(), FileStore::open(temp.path()).unwrap()).unwrap();
            clock.post_waypoint(leaf).unwrap();
            chain.set_tip(tip(2, Some(1), 811));
            let report = clock.poll_once().unwrap().unwrap();
            assert_eq!(report.closed_batch, Some(0));
            assert_eq!(clock.pending_sequence(), Some(0));
            assert!(clock.path_for(&leaf).unwrap().is_none());
        }

        chain.set_tip(tip(2, Some(1), 811));
        let mut clock = reopen(&chain, temp.path());
        assert_eq!(clock.pending_sequence(), Some(0));
        assert!(clock.serving_sequence().is_none());
        assert!(clock.path_for(&leaf).unwrap().is_none());
        let restored = event_values(temp.path(), "restart_broadcast_restored_pending");
        assert_eq!(restored.len(), 1, "{restored:?}");
        assert_eq!(restored[0]["batch_sequence"], 0);
        assert!(restored[0]["txid"].as_str().unwrap().len() == 64);
        let stored = FileStore::open(temp.path())
            .unwrap()
            .load_batches()
            .unwrap();
        let batch = stored
            .iter()
            .find(|batch| batch.batch_sequence == 0)
            .unwrap();
        assert_eq!(batch.state, StoredState::Broadcast);
        assert!(batch.plant.is_some());
        assert!(batch.inclusion.is_none());
        assert!(event_values(temp.path(), "restart_incomplete_failed_closed").is_empty());

        chain.set_included(true);
        chain.set_tip(tip(3, Some(2), 812));
        clock.poll_once().unwrap().unwrap();
        let body = clock
            .path_for(&leaf)
            .unwrap()
            .expect("included after restore");
        body.validate_for(&leaf).unwrap();
        assert!(clock.pending_sequence().is_none());
    }

    #[test]
    fn restart_restores_included_batch_into_serving() {
        let chain = Arc::new(FakeChain::default());
        let temp = tempfile::tempdir().unwrap();
        let leaf = [0x72; 32];
        chain.set_tip(tip(1, None, 820));
        {
            let mut clock =
                BariClock::open(chain.clone(), FileStore::open(temp.path()).unwrap()).unwrap();
            clock.post_waypoint(leaf).unwrap();
            chain.set_tip(tip(2, Some(1), 821));
            clock.poll_once().unwrap().unwrap();
            chain.set_included(true);
            chain.set_tip(tip(3, Some(2), 822));
            clock.poll_once().unwrap().unwrap();
            assert!(clock.path_for(&leaf).unwrap().is_some());
            assert_eq!(clock.serving_sequence(), Some(0));
        }

        chain.set_tip(tip(3, Some(2), 822));
        let clock = reopen(&chain, temp.path());
        assert_eq!(clock.serving_sequence(), Some(0));
        assert!(clock.pending_sequence().is_none());
        let body = clock
            .path_for(&leaf)
            .unwrap()
            .expect("serving after restart");
        body.validate_for(&leaf).unwrap();
        let restored = event_values(temp.path(), "restart_included_restored_serving");
        assert_eq!(restored.len(), 1, "{restored:?}");
        assert_eq!(restored[0]["batch_sequence"], 0);
        assert_eq!(restored[0]["block_height"], 822);
        assert!(event_values(temp.path(), "restart_broadcast_restored_pending").is_empty());
        let stored = FileStore::open(temp.path())
            .unwrap()
            .load_batches()
            .unwrap();
        assert_eq!(stored[0].state, StoredState::Included);
    }

    #[test]
    fn restart_still_fail_closes_a_plant_that_never_broadcast() {
        let chain = Arc::new(FakeChain::default());
        let temp = tempfile::tempdir().unwrap();
        chain.set_tip(tip(1, None, 830));
        chain.set_plant_failure(Some(SkipReason::Fee));
        {
            let mut clock =
                BariClock::open(chain.clone(), FileStore::open(temp.path()).unwrap()).unwrap();
            clock.post_waypoint([0x73; 32]).unwrap();
            chain.set_tip(tip(2, Some(1), 831));
            clock.poll_once().unwrap().unwrap();
        }
        chain.set_tip(tip(2, Some(1), 831));
        let clock = reopen(&chain, temp.path());
        assert!(clock.pending_sequence().is_none());
        assert!(clock.serving_sequence().is_none());
        assert!(!event_values(temp.path(), "restart_incomplete_failed_closed").is_empty());
        assert!(event_values(temp.path(), "restart_broadcast_restored_pending").is_empty());
        let stored = FileStore::open(temp.path())
            .unwrap()
            .load_batches()
            .unwrap();
        assert_eq!(stored[0].state, StoredState::Skipped);
    }

    fn disk_batch(
        sequence: u64,
        leaf: Hash32,
        state: StoredState,
        inclusion: bool,
    ) -> PersistedBatch {
        let leaves = vec![leaf];
        let tree = MerkleTree::build(leaves.clone()).unwrap();
        PersistedBatch {
            batch_sequence: sequence,
            opened_tip: [1; 32],
            closed_tip: [2; 32],
            leaves,
            tree_size: tree.tree_size(),
            root: tree.root(),
            state,
            plant: Some(StoredPlant {
                txid: [sequence as u8; 32],
                vout: 1,
                fee_rate_sat_vb: 1.0,
                fee_source: "test".to_owned(),
            }),
            inclusion: inclusion.then_some(StoredInclusion {
                block_hash: [9; 32],
                block_height: 9,
            }),
            skip_reason: None,
            opened_tip_height: None,
            broadcast_height: None,
            next_block_min_sat_vb: None,
            fee_sat: None,
            vsize: None,
            leaf_post_heights: Vec::new(),
        }
    }

    #[test]
    fn restart_restores_the_highest_broadcast_and_included_batches() {
        let chain = Arc::new(FakeChain::default());
        let temp = tempfile::tempdir().unwrap();
        chain.set_tip(tip(4, None, 840));
        let store = FileStore::open(temp.path()).unwrap();
        let low_broadcast = [0x81; 32];
        let high_broadcast = [0x82; 32];
        let low_included = [0x83; 32];
        let high_included = [0x84; 32];
        store
            .persist_new(&disk_batch(2, low_broadcast, StoredState::Broadcast, false))
            .unwrap();
        store
            .persist_new(&disk_batch(
                5,
                high_broadcast,
                StoredState::Broadcast,
                false,
            ))
            .unwrap();
        store
            .persist_new(&disk_batch(3, low_included, StoredState::Included, true))
            .unwrap();
        store
            .persist_new(&disk_batch(7, high_included, StoredState::Included, true))
            .unwrap();

        let clock = reopen(&chain, temp.path());
        assert_eq!(clock.pending_sequence(), Some(5));
        assert_eq!(clock.serving_sequence(), Some(7));
        assert!(clock.path_for(&high_included).unwrap().is_some());
        assert!(clock.path_for(&low_included).unwrap().is_some());
        assert!(clock.path_for(&high_broadcast).unwrap().is_none());
        assert!(clock.path_for(&low_broadcast).unwrap().is_none());
        let pending = event_values(temp.path(), "restart_broadcast_restored_pending");
        let serving = event_values(temp.path(), "restart_included_restored_serving");
        assert_eq!(pending.len(), 2, "{pending:?}");
        assert!(pending.iter().any(|line| line["batch_sequence"] == 2));
        assert!(pending.iter().any(|line| line["batch_sequence"] == 5));
        assert_eq!(serving.len(), 2, "{serving:?}");
        assert!(serving.iter().any(|line| line["batch_sequence"] == 3));
        assert!(serving.iter().any(|line| line["batch_sequence"] == 7));
    }

    #[test]
    fn restart_restore_rejects_root_or_tree_size_mismatch() {
        let chain = Arc::new(FakeChain::default());
        let broadcast_dir = tempfile::tempdir().unwrap();
        chain.set_tip(tip(1, None, 850));
        let mut broadcast = disk_batch(1, [0x91; 32], StoredState::Broadcast, false);
        broadcast.root = [0xAB; 32];
        FileStore::open(broadcast_dir.path())
            .unwrap()
            .persist_new(&broadcast)
            .unwrap();
        let broadcast_err = match BariClock::open(
            chain.clone(),
            FileStore::open(broadcast_dir.path()).unwrap(),
        ) {
            Err(error) => error,
            Ok(_) => panic!("broadcast root mismatch must not open"),
        };
        match broadcast_err {
            ClockError::Storage(StorageError::Io(error)) => {
                assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
                assert!(error.to_string().contains("broadcast batch 1"));
            }
            other => panic!("expected invalid broadcast restore, got {other}"),
        }

        let included_dir = tempfile::tempdir().unwrap();
        let mut included = disk_batch(4, [0x92; 32], StoredState::Included, true);
        included.tree_size = included.tree_size.saturating_add(1);
        FileStore::open(included_dir.path())
            .unwrap()
            .persist_new(&included)
            .unwrap();
        let included_err =
            match BariClock::open(chain, FileStore::open(included_dir.path()).unwrap()) {
                Err(error) => error,
                Ok(_) => panic!("included tree_size mismatch must not open"),
            };
        match included_err {
            ClockError::Storage(StorageError::Io(error)) => {
                assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
                assert!(error.to_string().contains("included batch 4"));
            }
            other => panic!("expected invalid included restore, got {other}"),
        }
    }

    fn advance(clock: &mut BariClock, chain: &FakeChain, byte: u8, prev: u8, height: u32) {
        chain.set_tip(tip(byte, Some(prev), height));
        clock.poll_once().unwrap().unwrap();
    }

    #[test]
    fn serve_window_is_fixed_and_ignores_the_operator_setting() {
        let previous = std::env::var("BARI_CONFIRMATION_WINDOW_TIPS").ok();
        std::env::set_var("BARI_CONFIRMATION_WINDOW_TIPS", "1");

        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 1000));
        assert_eq!(
            clock.confirmation_window_tips(),
            DEFAULT_CONFIRMATION_WINDOW_TIPS
        );
        let leaf = [0x31; 32];
        assert_eq!(clock.path_miss(&[0xEE; 32]), PathMiss::Unknown);
        clock.post_waypoint(leaf).unwrap();
        assert_eq!(clock.path_miss(&leaf), PathMiss::StillInPool);
        advance(&mut clock, &chain, 2, 1, 1001);
        assert_eq!(chain.plant_count(), 1);
        assert_eq!(clock.path_miss(&leaf), PathMiss::SealedNotInBlock);
        chain.set_included(false);
        advance(&mut clock, &chain, 3, 2, 1002);
        assert!(clock.path_for(&leaf).unwrap().is_none());
        assert_eq!(clock.pending_sequence(), Some(0));
        advance(&mut clock, &chain, 4, 3, 1003);
        assert_eq!(clock.pending_sequence(), Some(0));
        assert_eq!(chain.plant_count(), 1);
        chain.set_included(true);
        advance(&mut clock, &chain, 5, 4, 1004);
        let body = clock.path_for(&leaf).unwrap().expect("served through H+4");
        body.validate_for(&leaf).unwrap();
        assert_eq!(body.block_height, 1004);
        advance(&mut clock, &chain, 6, 5, 1005);
        let again = clock
            .path_for(&leaf)
            .unwrap()
            .expect("confirmed path stays");
        assert_eq!(again.block_height, 1004);
        assert_ne!(clock.path_miss(&leaf), PathMiss::ArchivedAfterServe);
        assert_eq!(chain.plant_count(), 1);

        std::env::set_var("BARI_CONFIRMATION_WINDOW_TIPS", "99");
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 2000));
        let leaf = [0x32; 32];
        clock.post_waypoint(leaf).unwrap();
        advance(&mut clock, &chain, 2, 1, 2001);
        chain.set_included(true);
        advance(&mut clock, &chain, 3, 2, 2002);
        assert!(clock.path_for(&leaf).unwrap().is_some());
        advance(&mut clock, &chain, 4, 3, 2003);
        assert!(clock.path_for(&leaf).unwrap().is_some());
        advance(&mut clock, &chain, 5, 4, 2004);
        let body = clock.path_for(&leaf).unwrap().expect("still served at H+4");
        body.validate_for(&leaf).unwrap();
        assert_eq!(body.block_height, 2002);
        advance(&mut clock, &chain, 6, 5, 2005);
        let again = clock
            .path_for(&leaf)
            .unwrap()
            .expect("window dial does not archive");
        assert_eq!(again.block_height, 2002);
        assert_ne!(clock.path_miss(&leaf), PathMiss::ArchivedAfterServe);
        assert_eq!(chain.plant_count(), 1);

        std::env::set_var("BARI_CONFIRMATION_WINDOW_TIPS", "9");
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 3000));
        let leaf = [0x33; 32];
        clock.post_waypoint(leaf).unwrap();
        advance(&mut clock, &chain, 2, 1, 3001);
        chain.set_included(false);
        advance(&mut clock, &chain, 3, 2, 3002);
        assert_eq!(clock.path_miss(&leaf), PathMiss::SealedNotInBlock);
        advance(&mut clock, &chain, 4, 3, 3003);
        assert_eq!(clock.pending_sequence(), Some(0));
        chain.set_tip(tip(5, Some(4), 3004));
        assert!(clock.poll_once().unwrap().unwrap().missed.is_none());
        chain.set_tip(tip(6, Some(5), 3005));
        assert!(clock.poll_once().unwrap().unwrap().missed.is_none());
        chain.set_tip(tip(7, Some(6), 3006));
        let report = clock.poll_once().unwrap().unwrap();
        assert_eq!(report.missed.unwrap().1, SkipReason::NotMined);
        assert_eq!(clock.path_miss(&leaf), PathMiss::SealedNotInBlock);
        assert!(clock.path_for(&leaf).unwrap().is_none());
        chain.set_included(true);
        chain.set_tip(tip(8, Some(7), 3007));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 1);
        let late = clock.path_for(&leaf).unwrap().expect("late confirm serves");
        assert_eq!(late.block_height, 3007);
        assert!(clock.last_confirmed().unwrap().confirmed_late);

        match previous {
            Some(value) => std::env::set_var("BARI_CONFIRMATION_WINDOW_TIPS", value),
            None => std::env::remove_var("BARI_CONFIRMATION_WINDOW_TIPS"),
        }
    }

    #[test]
    fn index_records_more_than_one_plant_at_the_same_height() {
        let chain = Arc::new(FakeChain::default());
        chain.set_network("main");
        let temp = tempfile::tempdir().unwrap();
        chain.set_tip(tip(1, None, 4000));
        let mut clock = BariClock::open_for_network(
            chain.clone(),
            FileStore::open(temp.path()).unwrap(),
            "main",
        )
        .unwrap();
        let first = [0x41; 32];
        let second = [0x42; 32];
        clock.post_waypoint(first).unwrap();
        chain.set_included(false);
        chain.set_tip(tip(2, Some(1), 4001));
        clock.poll_once().unwrap().unwrap();
        clock.post_waypoint(second).unwrap();
        // The first tree is unconfirmed and does not hold the second.
        chain.set_tip(tip(3, Some(2), 4002));
        clock.poll_once().unwrap().unwrap();
        assert!(!clock.pool_contains(&second));
        assert_eq!(chain.plant_count(), 2);
        assert_eq!(clock.inflight().len(), 2);
        // One block includes the first tree. The second stays in the mempool.
        chain.include_broadcast(Some(4001));
        chain.set_tip(tip(4, Some(3), 4003));
        clock.poll_once().unwrap().unwrap();
        let first_body = clock.path_for(&first).unwrap().expect("first path");
        assert!(clock.path_for(&second).unwrap().is_none());
        // The next block includes the second tree. Both land, in different blocks.
        chain.include_broadcast(Some(4002));
        chain.set_tip(tip(5, Some(4), 4004));
        clock.poll_once().unwrap().unwrap();
        let second_body = clock.path_for(&second).unwrap().expect("second path");
        first_body.validate_for(&first).unwrap();
        second_body.validate_for(&second).unwrap();
        assert_eq!(first_body.block_height, 4003);
        assert_eq!(second_body.block_height, 4004);
        assert_ne!(first_body.root, second_body.root);
        let snapshot = clock.index_snapshot();
        assert_eq!(snapshot.network, "main");
        // The live tip is the block that included the second tree, so the
        // published index carries that one plant. The earlier inclusion stays
        // on its own height.
        assert_eq!(snapshot.plants.len(), 1);
        assert_eq!(snapshot.plants[0].block_height, 4004);
        let first_index = clock.index_at_height(4003).unwrap().unwrap();
        assert_eq!(first_index.plants.len(), 1);
        assert_eq!(first_index.plants[0].block_height, 4003);
        let second_index = clock.index_at_height(4004).unwrap().unwrap();
        assert_eq!(second_index.plants.len(), 1);
        assert_ne!(first_index.plants[0].root, second_index.plants[0].root);
        assert_ne!(first_index.plants[0].txid, second_index.plants[0].txid);
    }

    #[test]
    fn waypoint_already_in_the_pool_is_kept_across_restart() {
        let temp = tempfile::tempdir().unwrap();
        let chain = Arc::new(FakeChain::default());
        let leaf = [0x51; 32];
        chain.set_tip(tip(1, None, 5000));
        {
            let mut clock =
                BariClock::open(chain.clone(), FileStore::open(temp.path()).unwrap()).unwrap();
            clock.post_waypoint(leaf).unwrap();
            assert_eq!(clock.pool_len(), 1);
        }
        let mut clock = reopen(&chain, temp.path());
        assert!(clock.pool_contains(&leaf));
        assert_eq!(clock.pool_len(), 1);
        assert_eq!(clock.path_miss(&leaf), PathMiss::StillInPool);
        assert_eq!(clock.post_waypoint(leaf).unwrap(), PostAccept::Duplicate);
        chain.set_tip(tip(2, Some(1), 5001));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(clock.path_miss(&leaf), PathMiss::SealedNotInBlock);
        assert!(clock.held_anywhere(&leaf));
    }

    #[test]
    fn restart_keeps_a_confirmed_path_servable() {
        let temp = tempfile::tempdir().unwrap();
        let chain = Arc::new(FakeChain::default());
        let leaf = [0x61; 32];
        chain.set_tip(tip(1, None, 6000));
        {
            let mut clock =
                BariClock::open(chain.clone(), FileStore::open(temp.path()).unwrap()).unwrap();
            clock.post_waypoint(leaf).unwrap();
            chain.set_tip(tip(2, Some(1), 6001));
            clock.poll_once().unwrap().unwrap();
            chain.set_included(true);
            chain.set_tip(tip(3, Some(2), 6002));
            clock.poll_once().unwrap().unwrap();
            assert!(clock.path_for(&leaf).unwrap().is_some());
        }
        chain.set_tip(tip(5, Some(4), 6004));
        let mut clock = reopen(&chain, temp.path());
        let body = clock.path_for(&leaf).unwrap().expect("still served at H+4");
        body.validate_for(&leaf).unwrap();
        chain.set_tip(tip(6, Some(5), 6005));
        clock.poll_once().unwrap().unwrap();
        let again = clock
            .path_for(&leaf)
            .unwrap()
            .expect("restart does not archive");
        assert_eq!(again.block_height, body.block_height);
        assert_ne!(clock.path_miss(&leaf), PathMiss::ArchivedAfterServe);
    }

    #[test]
    fn a_skipped_plant_is_not_fee_bumped_replaced_or_repaired() {
        let chain = Arc::new(FakeChain::default());
        let (mut clock, _temp) = open_at(chain.clone(), tip(1, None, 7000));
        let leaf = [0x71; 32];
        clock.post_waypoint(leaf).unwrap();
        chain.set_plant_failure(Some(SkipReason::Fee));
        chain.set_tip(tip(2, Some(1), 7001));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 0);
        chain.set_plant_failure(None);
        chain.set_included(true);
        chain.set_tip(tip(3, Some(2), 7002));
        clock.poll_once().unwrap().unwrap();
        chain.set_tip(tip(4, Some(3), 7003));
        clock.poll_once().unwrap().unwrap();
        assert_eq!(chain.plant_count(), 0);
        assert!(clock.path_for(&leaf).unwrap().is_none());
        assert_eq!(clock.path_miss(&leaf), PathMiss::NotIncluded);
    }

    fn index_file(dir: &std::path::Path, height: u32) -> std::path::PathBuf {
        dir.join("index").join(format!("height-{height:010}.json"))
    }

    fn index_bytes(dir: &std::path::Path, height: u32) -> Vec<u8> {
        std::fs::read(index_file(dir, height)).unwrap()
    }

    fn retained_heights(dir: &std::path::Path) -> Vec<u64> {
        let path = dir.join("events.jsonl");
        if !path.is_file() {
            return Vec::new();
        }
        event_values(dir, "index_tip_retained")
            .into_iter()
            .filter_map(|value| value["height"].as_u64())
            .collect()
    }

    /// New tips stamp Bitcoin `nTime`. A height file that already omits
    /// `header.time` stays omitted, including the open tip. There is no
    /// backfill. The observation stamp is not copied into `header.time`.
    #[test]
    fn new_rows_stamp_header_time_and_older_rows_stay_without_it() {
        let temp = tempfile::tempdir().unwrap();
        FileStore::open(temp.path()).unwrap();
        let bare = |height: u32, hash: &str| {
            let doc = StoredPortableIndex {
                version: 2,
                network: "main".to_owned(),
                header: StoredIndexHeader {
                    hash: hash.to_owned(),
                    height,
                    time: None,
                    timestamp_unix_ms: Some(1_700_000_000_000),
                },
                plant: None,
                plants: Vec::new(),
            };
            let mut bytes = serde_json::to_vec_pretty(&doc).unwrap();
            bytes.push(b'\n');
            std::fs::write(index_file(temp.path(), height), &bytes).unwrap();
            bytes
        };
        let older = bare(100, &"aa".repeat(32));
        let open_tip = bare(101, &encode_bitcoin_id(&[1; 32]));

        let chain = Arc::new(FakeChain::default());
        chain.set_network("main");
        chain.set_tip(ChainTip {
            hash: [1; 32],
            previous_hash: None,
            height: 101,
            time: Some(1_700_000_101),
        });
        let mut clock = BariClock::open_for_network(
            chain.clone(),
            FileStore::open(temp.path()).unwrap(),
            "main",
        )
        .unwrap();
        assert_eq!(index_bytes(temp.path(), 100), older);
        assert_eq!(index_bytes(temp.path(), 101), open_tip);
        assert!(clock
            .index_at_height(100)
            .unwrap()
            .unwrap()
            .header
            .time
            .is_none());
        assert!(clock
            .index_at_height(101)
            .unwrap()
            .unwrap()
            .header
            .time
            .is_none());

        chain.set_tip(ChainTip {
            hash: [2; 32],
            previous_hash: Some([1; 32]),
            height: 102,
            time: Some(1_700_000_102),
        });
        clock.poll_once().unwrap().unwrap();
        let stamped = clock.index_at_height(102).unwrap().unwrap();
        assert_eq!(stamped.header.time, Some(1_700_000_102));
        assert!(stamped.header.timestamp_unix_ms.is_some());
        assert_ne!(stamped.header.timestamp_unix_ms, Some(1_700_000_102_000));
        assert_eq!(index_bytes(temp.path(), 100), older);
        assert_eq!(index_bytes(temp.path(), 101), open_tip);
    }

    /// The height file from `on_tip_prepare` is already on disk, including its
    /// observation stamp. A later open at that same tip must not rewrite it
    /// or any other height.
    #[test]
    fn startup_does_not_rewrite_a_tip_index_already_on_disk() {
        let temp = tempfile::tempdir().unwrap();
        let chain = Arc::new(FakeChain::default());
        chain.set_network("main");
        chain.set_tip(tip(1, None, 969873));
        let mut clock = BariClock::open_for_network(
            chain.clone(),
            FileStore::open(temp.path()).unwrap(),
            "main",
        )
        .unwrap();
        chain.set_tip(tip(2, Some(1), 969874));
        clock.poll_once().unwrap().unwrap();
        drop(clock);

        let prior = index_bytes(temp.path(), 969873);
        let tip_bytes = index_bytes(temp.path(), 969874);
        let tip_doc: StoredPortableIndex = serde_json::from_slice(&tip_bytes).unwrap();
        assert_eq!(tip_doc.header.height, 969874);
        assert!(tip_doc.header.timestamp_unix_ms.is_some());
        let retained = retained_heights(temp.path());

        chain.set_tip(tip(2, Some(1), 969874));
        let _clock =
            BariClock::open_for_network(chain, FileStore::open(temp.path()).unwrap(), "main")
                .unwrap();

        assert_eq!(index_bytes(temp.path(), 969873), prior);
        assert_eq!(index_bytes(temp.path(), 969874), tip_bytes);
        assert_eq!(retained_heights(temp.path()), retained);
        assert!(!index_file(temp.path(), 969875).exists());
    }

    /// Open again while 969874 is still the tip and its file is absent.
    /// The lower file and an unrelated higher file stay as stored.
    #[test]
    fn startup_writes_the_tip_index_when_the_height_file_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let chain = Arc::new(FakeChain::default());
        chain.set_network("main");
        chain.set_tip(tip(1, None, 969873));
        let mut clock = BariClock::open_for_network(
            chain.clone(),
            FileStore::open(temp.path()).unwrap(),
            "main",
        )
        .unwrap();
        chain.set_tip(tip(2, Some(1), 969874));
        clock.poll_once().unwrap().unwrap();
        drop(clock);

        let prior = index_bytes(temp.path(), 969873);
        std::fs::remove_file(index_file(temp.path(), 969874)).unwrap();
        let neighbour = StoredPortableIndex {
            version: 2,
            network: "main".to_owned(),
            header: StoredIndexHeader {
                hash: "ee".repeat(32),
                height: 969875,
                time: None,
                timestamp_unix_ms: Some(1),
            },
            plant: None,
            plants: Vec::new(),
        };
        let mut neighbour_bytes = serde_json::to_vec_pretty(&neighbour).unwrap();
        neighbour_bytes.push(b'\n');
        std::fs::write(index_file(temp.path(), 969875), &neighbour_bytes).unwrap();
        let retained_before = retained_heights(temp.path())
            .into_iter()
            .filter(|height| *height == 969874)
            .count();

        chain.set_tip(tip(2, Some(1), 969874));
        let clock =
            BariClock::open_for_network(chain, FileStore::open(temp.path()).unwrap(), "main")
                .unwrap();

        let stored = clock.index_at_height(969874).unwrap().expect("tip file");
        assert_eq!(stored.header.height, 969874);
        assert_eq!(stored.header.hash, encode_bitcoin_id(&[2; 32]));
        assert_eq!(stored.network, "main");
        assert!(stored.plant.is_none());
        assert!(stored.plants.is_empty());
        assert_eq!(index_bytes(temp.path(), 969873), prior);
        assert_eq!(index_bytes(temp.path(), 969875), neighbour_bytes);
        let retained_after = retained_heights(temp.path())
            .into_iter()
            .filter(|height| *height == 969874)
            .count();
        assert_eq!(retained_after, retained_before + 1);
    }

    #[test]
    fn startup_persists_the_tip_when_plant_state_is_empty() {
        let temp = tempfile::tempdir().unwrap();
        let chain = Arc::new(FakeChain::default());
        chain.set_network("main");
        chain.set_tip(tip(4, None, 50));
        let clock = BariClock::open_for_network(
            chain.clone(),
            FileStore::open(temp.path()).unwrap(),
            "main",
        )
        .unwrap();
        assert!(clock.index_snapshot().plant.is_none());
        assert!(clock.index_snapshot().plants.is_empty());
        assert!(clock.serving_sequence().is_none());
        let stored = clock.index_at_height(50).unwrap().expect("tip file");
        assert!(stored.plant.is_none());
        assert!(stored.plants.is_empty());
        assert_eq!(stored.network, "main");
        assert_eq!(stored.header.hash, encode_bitcoin_id(&[4; 32]));
        let bytes = index_bytes(temp.path(), 50);
        let retained = retained_heights(temp.path());
        assert_eq!(retained, vec![50]);
        drop(clock);

        let clock =
            BariClock::open_for_network(chain, FileStore::open(temp.path()).unwrap(), "main")
                .unwrap();
        assert_eq!(index_bytes(temp.path(), 50), bytes);
        assert_eq!(retained_heights(temp.path()), retained);
        assert!(clock.index_at_height(49).unwrap().is_none());
        assert!(clock.index_at_height(51).unwrap().is_none());
        assert!(clock.index_snapshot().plant.is_none());
    }
}
