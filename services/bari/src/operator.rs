//! Operator page for ops.groundinglab.io.
//!
//! `GET /ops` locks the clock for [`crate::clock::OpsMemory`], reads the node
//! after the lock, then calls [`build_ops_view`] and [`render`]. Both are pure.
//! The page does not call `place_plants`, `plants_from_notes`,
//! `collect_fee_samples`, `smooth_min_series`, or `reconstruct_history`, and it
//! does not look up `tx_place` or print funding addresses.

use crate::clock::{
    BlockLedgerRow, OpsMemory, StoreIssue, WalletCoin, ANCHOR_WINDOW_BLOCKS, LEDGER_RETAIN_BLOCKS,
};
use crate::fee::{self, DEFAULT_FEE_CAP_SAT_VB, DEFAULT_FEE_MULTIPLIER};
use crate::storage::PersistedBatch;
use crate::utxo::{self, PlantUtxo, UtxoPolicy};
use crate::{encode_bitcoin_id, Hash32};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const VERSION: &str = "0.6.2";
const WINDOW: u32 = ANCHOR_WINDOW_BLOCKS;
const LEDGER_BLOCKS: u32 = LEDGER_RETAIN_BLOCKS;
const VSIZE: u64 = 159;
const CAP: f64 = DEFAULT_FEE_CAP_SAT_VB;
const TREE_COST_AT_CAP: u64 = 175;

const TIP: u32 = 970_700;
const TIP_TIME: u64 = 1_791_610_034;
const RENDERED: u64 = TIP_TIME + 4 * 60 + 8;
const FIRST_SEQ: u64 = 4403;

/// A value the node answered, a failed read, or a backend that has none.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeRead<T> {
    Ready(T),
    Failed { reason: String },
    Absent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FeeReady {
    pub min_sat_vb: Option<f64>,
    pub floor_sat_vb: Option<f64>,
    pub bid_sat_vb: Option<f64>,
    pub source: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WalletReady {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncReady {
    pub blocks: u32,
    pub headers: u32,
    pub initialblockdownload: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OpsReads {
    pub fee: NodeRead<FeeReady>,
    pub wallet: NodeRead<WalletReady>,
    pub sync: NodeRead<SyncReady>,
}

#[derive(Clone, Debug)]
pub struct ProcessFacts {
    pub version: String,
    pub commit: String,
    pub instance: String,
    pub started_unix_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldKind {
    MinAboveCap,
    FloorAboveCap,
    TreeInMempool,
    NoFeeReading,
    NoConfirmedCoin,
    SendError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionKind {
    Sent,
    Held(HoldKind),
    Quiet,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TreeInput {
    pub batch_sequence: u64,
    pub broadcast_height: u32,
    pub tree_size: u32,
    pub txid: Hash32,
    pub inclusion_height: Option<u32>,
    pub next_block_min_sat_vb: Option<f64>,
    pub bid_sat_vb: f64,
    pub fee_sat: Option<u64>,
    pub vsize: Option<u32>,
    pub fee_source: String,
    /// Tip height of the POST that accepted each waypoint, leaf order.
    /// Empty on a record written before that pin.
    pub post_heights: Vec<u32>,
    /// Leaf ids in tree order. Empty on a record that did not carry them.
    pub leaves: Vec<Hash32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockInput {
    pub height: u32,
    pub time: Option<u64>,
    pub observed_unix_ms: u64,
    pub decision: DecisionKind,
    pub next_block_min_sat_vb: Option<f64>,
    pub bid_sat_vb: Option<f64>,
    pub pool_size: u32,
    /// `None` on a pre-v2 line. R11 skips any pair that includes one.
    pub added: Option<u32>,
    pub malformed: u32,
    pub already_held: u32,
    pub tree: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct OpsInputs {
    pub tip_height: u32,
    pub tip_time: Option<u64>,
    pub observed_unix_ms: Option<u64>,
    pub trees: Vec<TreeInput>,
    pub blocks: Vec<BlockInput>,
    pub pool_len: u32,
    pub oldest_post_height: Option<u32>,
    pub added_since_tip: u32,
    pub malformed_since_tip: u32,
    pub already_held_since_tip: u32,
    pub store_issues: Vec<StoreIssue>,
    pub process: ProcessFacts,
    pub network: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    InMempool { n: u32 },
    OnTime { n: u32 },
    Late { n: u32 },
    Missed,
}

#[derive(Clone, Debug)]
pub struct TreeView {
    pub seq: u64,
    pub broadcast_height: u32,
    pub inclusion_height: Option<u32>,
    pub tree_size: u32,
    pub bid_c: u32,
    pub fee_sat: u64,
    pub fee_approx: bool,
    pub txid_display: String,
    /// Natural-order leaf ids, same order as `post_heights`. Empty when the
    /// record did not carry them. R12 uses this to reject a leaf in two trees.
    pub leaves: Vec<Hash32>,
    pub outcome: Outcome,
    pub fee_source: String,
    pub next_block_min_c: Option<u32>,
    /// Tip height of the POST that accepted each waypoint. Empty on a record
    /// written before that pin. The sequence check skips an empty list.
    pub post_heights: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct BlockView {
    pub height: u32,
    pub time: Option<u64>,
    pub observed_unix_ms: u64,
    pub decision: DecisionKind,
    pub min_c: Option<u32>,
    pub bid_c: Option<u32>,
    pub pool_size: u32,
    pub added: Option<u32>,
    pub malformed: u32,
    pub already_held: u32,
    pub tree: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct CollectingTile {
    pub next: u32,
    pub count: u32,
    pub oldest: Option<u32>,
    pub oldest_blocks: Option<u32>,
    pub forecast: &'static str,
    pub forecast_text: String,
    pub min_c: Option<u32>,
    pub min_unknown: bool,
}

#[derive(Clone, Debug)]
pub struct AnchoredTile {
    pub tree: TreeView,
    pub in_window: bool,
    pub ago: u32,
}

#[derive(Clone, Debug)]
pub enum LedgerRow {
    Collecting {
        next: u32,
        count: u32,
        forecast_text: String,
    },
    Tree {
        tree: TreeView,
        pinned: bool,
    },
    Gap {
        lo: u32,
        hi: u32,
        n: u32,
        parts: Vec<String>,
        title: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub trees: u32,
    pub on_time: u32,
    pub late: u32,
    pub missed: u32,
    pub mempool: u32,
    pub waypoints: u32,
    pub paid: u64,
    pub paid_approx: bool,
    pub pending_fee: u64,
    pub blocks: u32,
    pub sent: u32,
    pub held: u32,
    pub quiet: u32,
    pub not_seen: u32,
    pub malformed: u32,
    pub already_held: u32,
}

#[derive(Clone, Debug)]
pub struct WalletView {
    pub known: bool,
    pub reason: String,
    pub confirmed: u64,
    pub coins: u32,
    pub unconfirmed: u64,
    pub unconfirmed_coins: u32,
    pub thin: bool,
    pub runway: u64,
    pub per_day: f64,
    pub days: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct SystemView {
    pub known: bool,
    pub version: String,
    pub commit: String,
    pub instance: String,
    pub network: String,
    pub uptime: String,
    pub since: Option<u32>,
    pub node_blocks: u32,
    pub node_headers: u32,
    pub syncing: bool,
    pub tip_lag: i64,
    pub tip_seen_min: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct Check {
    pub id: String,
    pub name: String,
    pub holds: bool,
    pub detail: String,
}

#[derive(Clone, Debug)]
pub struct Banner {
    pub issue: bool,
    pub html: String,
}

#[derive(Clone, Debug)]
pub struct OpsView {
    pub tip: u32,
    pub network: String,
    pub instance: String,
    pub explorer: bool,
    pub test_plane: bool,
    pub now_unix: u64,
    pub window_start: u32,
    pub collecting: CollectingTile,
    /// Every in-flight tree. Empty when none is waiting for a block.
    pub mempool: Vec<TreeView>,
    pub anchored: Option<AnchoredTile>,
    pub ledger: Vec<LedgerRow>,
    pub trees: Vec<TreeView>,
    pub blocks: BTreeMap<u32, BlockView>,
    pub live_min_c: Option<u32>,
    pub summary: Summary,
    pub wallet: WalletView,
    pub system: SystemView,
    pub checks: Vec<Check>,
    pub banners: Vec<Banner>,
    pub pool_len: u32,
    pub added_since_tip: u32,
    /// Process start, unix milliseconds. R11's live pool equation runs only
    /// when the tip block was observed at or after this instant.
    pub started_unix_ms: u64,
    /// Quiet heights recorded in a catch-up burst, not as the live tip.
    /// The ledger reads these as `not seen (Bari offline)`.
    pub offline: BTreeSet<u32>,
}

/// The only place an outcome is computed.
///
/// `+N` is `included − sent` when the tree is in a block, and `tip − sent`
/// while it is still in the mempool. The window is 5 blocks from the
/// broadcast height: `+1` through `+5` are on time. `+5` is inside the window.
/// `+6` and later are late. A tree with no inclusion once the tip is 5 or more
/// past the broadcast is missed. `+0` is the broadcast block itself and is
/// not an inclusion.
pub fn tree_outcome(inclusion: Option<u32>, broadcast: u32, tip: u32) -> Outcome {
    if let Some(height) = inclusion {
        let n = height.saturating_sub(broadcast);
        if (1..=WINDOW).contains(&n) {
            Outcome::OnTime { n }
        } else {
            Outcome::Late { n }
        }
    } else {
        let n = tip.saturating_sub(broadcast);
        if n < WINDOW {
            Outcome::InMempool { n }
        } else {
            Outcome::Missed
        }
    }
}

pub fn outcome_text(outcome: &Outcome) -> String {
    match outcome {
        Outcome::OnTime { n } => format!("On time +{n}"),
        Outcome::Late { n } => format!("Late +{n}"),
        Outcome::InMempool { n } => format!("In mempool +{n} of {WINDOW}"),
        Outcome::Missed => "Missed".to_owned(),
    }
}

pub fn outcome_title(broadcast: u32, outcome: &Outcome) -> String {
    let end = broadcast.saturating_add(WINDOW);
    match outcome {
        Outcome::Missed => format!(
            "Not in a block by {end} (sent + {WINDOW}). These waypoints are skipped in the PCOC. Becomes Late if a later block includes it."
        ),
        Outcome::Late { .. } => format!(
            "Included after the {WINDOW}-block window ({start}–{end}). The path is served.",
            start = broadcast.saturating_add(1)
        ),
        Outcome::InMempool { .. } => format!(
            "Waiting for a block in {start}–{end}.",
            start = broadcast.saturating_add(1)
        ),
        Outcome::OnTime { .. } => format!(
            "Included inside the window {start}–{end}.",
            start = broadcast.saturating_add(1)
        ),
    }
}

fn outcome_kind(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::InMempool { .. } => "mempool",
        Outcome::OnTime { .. } => "ontime",
        Outcome::Late { .. } => "late",
        Outcome::Missed => "missed",
    }
}

fn hold_kind(raw: &str) -> HoldKind {
    match raw {
        "min-above-cap" | "next-block-min" => HoldKind::MinAboveCap,
        "floor-above-cap" | "cap" => HoldKind::FloorAboveCap,
        "tree-in-mempool" => HoldKind::TreeInMempool,
        "no-fee-reading" | "no-estimate" | "fallback" | "fee-view" => HoldKind::NoFeeReading,
        "no-confirmed-coin" | "no-confirmed-utxo" | "utxo-view" | "underfunded" => {
            HoldKind::NoConfirmedCoin
        }
        _ => HoldKind::SendError,
    }
}

fn hold_text(kind: HoldKind) -> &'static str {
    match kind {
        HoldKind::MinAboveCap => "held (min ≥ cap)",
        HoldKind::FloorAboveCap => "held (floor > cap)",
        HoldKind::TreeInMempool => "held (tree in mempool, older build)",
        HoldKind::NoFeeReading => "held (no fee reading)",
        HoldKind::NoConfirmedCoin => "held (no confirmed coin)",
        HoldKind::SendError => "held (send error)",
    }
}

fn centi(rate: f64) -> Option<u32> {
    if !rate.is_finite() || rate < 0.0 {
        return None;
    }
    let scaled = (rate * 100.0).round();
    if !(0.0..=u32::MAX as f64).contains(&scaled) {
        return None;
    }
    Some(scaled as u32)
}

/// Ledger label `held (min ≥ cap)`: the next-block minimum, in hundredths of
/// a sat/vB, is at or above the cap. Equal to the cap holds. [`forecast`] and
/// the fixture use this comparison, and the Collecting tile does too.
fn min_at_or_above_cap(min_c: u32) -> bool {
    min_c >= centi(CAP).unwrap_or(110)
}

fn rate_text(cents: Option<u32>) -> String {
    match cents {
        Some(cents) => fee::format_sat_vb(f64::from(cents) / 100.0),
        None => "–".to_owned(),
    }
}

fn group_sats(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn plural(n: u32, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

pub fn inputs_from_memory(memory: &OpsMemory, process: ProcessFacts) -> OpsInputs {
    let mut store_issues = memory.store_issues.clone();
    let mut trees = Vec::new();
    for record in &memory.trees {
        match tree_input(record) {
            Some(tree) => trees.push(tree),
            None => {
                let file = format!("batches/batch-{:020}.json", record.batch_sequence);
                if !store_issues.iter().any(|issue| issue.file == file) {
                    store_issues.push(StoreIssue {
                        file,
                        error: "broadcast height unresolved".to_owned(),
                    });
                }
            }
        }
    }
    let blocks = memory.blocks.iter().map(block_input).collect();
    OpsInputs {
        tip_height: memory.tip.height,
        tip_time: memory.tip.time,
        observed_unix_ms: memory.observed_unix_ms,
        trees,
        blocks,
        pool_len: memory.pool_len,
        oldest_post_height: memory.oldest_post_height,
        added_since_tip: memory.added_since_tip,
        malformed_since_tip: memory.malformed_since_tip,
        already_held_since_tip: memory.already_held_since_tip,
        store_issues,
        process,
        network: memory.network.clone(),
    }
}

fn tree_input(record: &PersistedBatch) -> Option<TreeInput> {
    let plant = record.plant.as_ref()?;
    let broadcast_height = record.broadcast_height?;
    Some(TreeInput {
        batch_sequence: record.batch_sequence,
        broadcast_height,
        tree_size: record.tree_size,
        txid: plant.txid,
        inclusion_height: record
            .inclusion
            .as_ref()
            .map(|inclusion| inclusion.block_height),
        next_block_min_sat_vb: record.next_block_min_sat_vb,
        bid_sat_vb: plant.fee_rate_sat_vb,
        fee_sat: record.fee_sat,
        vsize: record.vsize,
        fee_source: plant.fee_source.clone(),
        post_heights: record.leaf_post_heights.clone(),
        leaves: record.leaves.clone(),
    })
}

fn block_input(row: &BlockLedgerRow) -> BlockInput {
    let decision = match row.decision.as_str() {
        "sent" => DecisionKind::Sent,
        "quiet" => DecisionKind::Quiet,
        "held" => DecisionKind::Held(hold_kind(if row.hold_reason.is_empty() {
            row.detail.as_str()
        } else {
            row.hold_reason.as_str()
        })),
        other => match other {
            "collecting" => DecisionKind::Held(hold_kind(&row.hold_reason)),
            "in-mempool" | "confirmed" | "confirmed-late" | "missed" => DecisionKind::Sent,
            _ => DecisionKind::Quiet,
        },
    };
    BlockInput {
        height: row.height,
        time: row.time,
        observed_unix_ms: row.seen_unix_ms,
        decision,
        next_block_min_sat_vb: row.min_fee_sat_vb,
        bid_sat_vb: row.bid_sat_vb,
        pool_size: row.leaf_count,
        added: row.added_known.then_some(row.new_waypoints),
        malformed: row.malformed,
        already_held: row.duplicate,
        tree: row.tree,
    }
}

fn tree_view(tree: &TreeInput, tip: u32) -> TreeView {
    let (fee_sat, fee_approx) = match tree.fee_sat {
        Some(fee) => (fee, false),
        None => (
            fee::broadcast_fee_sats(tree.bid_sat_vb, VSIZE).unwrap_or(0),
            true,
        ),
    };
    TreeView {
        seq: tree.batch_sequence,
        broadcast_height: tree.broadcast_height,
        inclusion_height: tree.inclusion_height,
        tree_size: tree.tree_size,
        bid_c: centi(tree.bid_sat_vb).unwrap_or(0),
        fee_sat,
        fee_approx,
        txid_display: encode_bitcoin_id(&tree.txid),
        leaves: tree.leaves.clone(),
        outcome: tree_outcome(tree.inclusion_height, tree.broadcast_height, tip),
        fee_source: tree.fee_source.clone(),
        next_block_min_c: tree.next_block_min_sat_vb.and_then(centi),
        post_heights: tree.post_heights.clone(),
    }
}

fn in_window(tree: &TreeView, start: u32) -> bool {
    tree.broadcast_height >= start || tree.inclusion_height.is_some_and(|height| height >= start)
}

/// Pure. Outcome, ledger, tiles, chart data, and R1–R11 come from these inputs.
pub fn build_ops_view(inputs: &OpsInputs, reads: &OpsReads, now_unix_ms: u64) -> OpsView {
    let tip = inputs.tip_height;
    let start = tip.saturating_sub(LEDGER_BLOCKS.saturating_sub(1));
    let network = inputs.network.as_str();
    let test_plane = network == "synthetic";
    let explorer = !test_plane;
    let blocks = block_map(&inputs.blocks);
    let offline = offline_quiet_heights(&blocks, start, tip);
    let all_trees: Vec<TreeView> = inputs
        .trees
        .iter()
        .map(|tree| tree_view(tree, tip))
        .collect();
    let mut trees: Vec<TreeView> = all_trees
        .iter()
        .filter(|tree| in_window(tree, start))
        .cloned()
        .collect();
    trees.sort_by_key(|tree| std::cmp::Reverse(tree.broadcast_height));

    let mut mempool: Vec<TreeView> = trees
        .iter()
        .filter(|tree| matches!(tree.outcome, Outcome::InMempool { .. }))
        .cloned()
        .collect();
    mempool.sort_by_key(|tree| tree.broadcast_height);
    let anchored_tree = all_trees
        .iter()
        .filter(|tree| tree.inclusion_height.is_some())
        .max_by_key(|tree| (tree.inclusion_height.unwrap_or(0), tree.broadcast_height));

    // A tree in the mempool does not change the forecast. Collecting is the pool.
    let (forecast, forecast_text, live_min_c) = forecast(inputs, reads, tip);
    let collecting = CollectingTile {
        next: tip.saturating_add(1),
        count: inputs.pool_len,
        oldest: inputs.oldest_post_height,
        oldest_blocks: inputs
            .oldest_post_height
            .map(|height| tip.saturating_sub(height)),
        forecast,
        forecast_text: forecast_text.clone(),
        min_c: live_min_c,
        min_unknown: live_min_c.is_none(),
    };
    let anchored = anchored_tree.map(|tree| AnchoredTile {
        in_window: in_window(tree, start),
        ago: tip.saturating_sub(tree.inclusion_height.unwrap_or(tip)),
        tree: tree.clone(),
    });

    let mut ledger = vec![LedgerRow::Collecting {
        next: collecting.next,
        count: collecting.count,
        forecast_text: forecast_text.clone(),
    }];
    let mut cursor = tip;
    for tree in &trees {
        let broadcast = tree.broadcast_height;
        if broadcast < cursor {
            let lo = broadcast.saturating_add(1).max(start);
            if lo <= cursor {
                ledger.push(gap_row(lo, cursor, &blocks, &offline));
            }
        }
        let pinned = mempool.iter().any(|item| item.seq == tree.seq)
            || anchored
                .as_ref()
                .is_some_and(|item| item.in_window && item.tree.seq == tree.seq);
        ledger.push(LedgerRow::Tree {
            tree: tree.clone(),
            pinned,
        });
        cursor = broadcast.saturating_sub(1);
    }
    if cursor >= start {
        ledger.push(gap_row(start, cursor, &blocks, &offline));
    }

    let summary = summarize(&trees, &blocks, &ledger, &offline, start, tip);
    let wallet = wallet_view(reads, &summary);
    let system = system_view(inputs, reads, tip, now_unix_ms, &blocks, start);
    let mut view = OpsView {
        tip,
        network: network.to_owned(),
        instance: inputs.process.instance.clone(),
        explorer,
        test_plane,
        now_unix: now_unix_ms,
        window_start: start,
        collecting,
        mempool,
        anchored,
        ledger,
        trees,
        blocks,
        live_min_c,
        summary,
        wallet,
        system,
        checks: Vec::new(),
        banners: Vec::new(),
        pool_len: inputs.pool_len,
        added_since_tip: inputs.added_since_tip,
        started_unix_ms: inputs.process.started_unix_ms,
        offline,
    };
    view.checks = evaluate_checks(&view);
    view.banners = banners(&view, inputs, now_unix_ms);
    view
}
fn block_map(blocks: &[BlockInput]) -> BTreeMap<u32, BlockView> {
    let mut map = BTreeMap::new();
    for block in blocks {
        map.insert(
            block.height,
            BlockView {
                height: block.height,
                time: block.time,
                observed_unix_ms: block.observed_unix_ms,
                decision: block.decision,
                min_c: block.next_block_min_sat_vb.and_then(centi),
                bid_c: block.bid_sat_vb.and_then(centi),
                pool_size: block.pool_size,
                added: block.added,
                malformed: block.malformed,
                already_held: block.already_held,
                tree: block.tree,
            },
        );
    }
    map
}

fn gap_kind(decision: Option<DecisionKind>) -> &'static str {
    match decision {
        None => "not-seen",
        Some(DecisionKind::Quiet) => "quiet",
        Some(DecisionKind::Sent) => "sent",
        Some(DecisionKind::Held(HoldKind::MinAboveCap)) => "held:min-above-cap",
        Some(DecisionKind::Held(HoldKind::FloorAboveCap)) => "held:floor-above-cap",
        Some(DecisionKind::Held(HoldKind::TreeInMempool)) => "held:tree-in-mempool",
        Some(DecisionKind::Held(HoldKind::NoFeeReading)) => "held:no-fee-reading",
        Some(DecisionKind::Held(HoldKind::NoConfirmedCoin)) => "held:no-confirmed-coin",
        Some(DecisionKind::Held(HoldKind::SendError)) => "held:send-error",
    }
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "quiet" => "quiet",
        "offline" => "not seen (Bari offline)",
        "not-seen" => "not seen",
        "sent" => "sent",
        "held:min-above-cap" => "held (min ≥ cap)",
        "held:floor-above-cap" => "held (floor > cap)",
        "held:tree-in-mempool" => "held (tree in mempool, older build)",
        "held:no-fee-reading" => "held (no fee reading)",
        "held:no-confirmed-coin" => "held (no confirmed coin)",
        "held:send-error" => "held (send error)",
        _ => "not seen",
    }
}

/// A quiet row is a live empty pool. A catch-up written long after the
/// header, or in the same short burst as one of those, was not seen live.
const OFFLINE_LAG_MS: u64 = 30 * 60 * 1000;
const BURST_GAP_MS: u64 = 3 * 60 * 1000;

fn offline_quiet_heights(blocks: &BTreeMap<u32, BlockView>, start: u32, tip: u32) -> BTreeSet<u32> {
    let recorded: Vec<u32> = (start..=tip)
        .filter(|height| blocks.contains_key(height))
        .collect();
    let stale = |height: u32| -> bool {
        let Some(block) = blocks.get(&height) else {
            return false;
        };
        if !matches!(block.decision, DecisionKind::Quiet) || block.observed_unix_ms == 0 {
            return false;
        }
        let Some(header) = block.time else {
            return false;
        };
        block
            .observed_unix_ms
            .saturating_sub(header.saturating_mul(1000))
            > OFFLINE_LAG_MS
    };
    let mut offline = BTreeSet::new();
    for height in &recorded {
        if stale(*height) {
            offline.insert(*height);
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for pair in recorded.windows(2) {
            let (left, right) = (pair[0], pair[1]);
            if right != left + 1 {
                continue;
            }
            let (Some(earlier), Some(later)) = (blocks.get(&left), blocks.get(&right)) else {
                continue;
            };
            if earlier.observed_unix_ms == 0 || later.observed_unix_ms == 0 {
                continue;
            }
            if earlier.observed_unix_ms.abs_diff(later.observed_unix_ms) > BURST_GAP_MS {
                continue;
            }
            for (height, other) in [(left, right), (right, left)] {
                let quiet = matches!(
                    blocks.get(&height).map(|block| block.decision),
                    Some(DecisionKind::Quiet)
                );
                if quiet && offline.contains(&other) && offline.insert(height) {
                    changed = true;
                }
            }
        }
    }
    offline
}

fn gap_row(
    lo: u32,
    hi: u32,
    blocks: &BTreeMap<u32, BlockView>,
    offline: &BTreeSet<u32>,
) -> LedgerRow {
    let mut order: Vec<(&str, u32)> = Vec::new();
    let mut pools = Vec::new();
    for height in lo..=hi {
        let block = blocks.get(&height);
        let mut kind = gap_kind(block.map(|item| item.decision));
        if kind == "quiet" && offline.contains(&height) {
            kind = "offline";
        }
        if let Some(found) = order.iter_mut().find(|(name, _)| *name == kind) {
            found.1 += 1;
        } else {
            order.push((kind, 1));
        }
        if let Some(block) = block {
            if block.pool_size > 0 {
                pools.push(block.pool_size);
            }
        }
    }
    order.sort_by(|left, right| {
        let group = |kind: &str| {
            if kind.starts_with("held:") {
                0
            } else if kind == "quiet" {
                1
            } else {
                2
            }
        };
        group(left.0)
            .cmp(&group(right.0))
            .then(right.1.cmp(&left.1))
    });
    let parts = if order.len() == 1 {
        vec![kind_label(order[0].0).to_owned()]
    } else {
        order
            .iter()
            .map(|(kind, count)| format!("{count} {}", kind_label(kind)))
            .collect()
    };
    let title = if pools.is_empty() {
        "pool empty".to_owned()
    } else {
        let joined = pools
            .iter()
            .map(|pool| pool.to_string())
            .collect::<Vec<_>>()
            .join(" → ");
        format!("pool at each decision: {joined}")
    };
    LedgerRow::Gap {
        lo,
        hi,
        n: hi - lo + 1,
        parts,
        title,
    }
}

fn summarize(
    trees: &[TreeView],
    blocks: &BTreeMap<u32, BlockView>,
    ledger: &[LedgerRow],
    offline: &BTreeSet<u32>,
    start: u32,
    tip: u32,
) -> Summary {
    let mut summary = Summary {
        trees: trees.len() as u32,
        on_time: 0,
        late: 0,
        missed: 0,
        mempool: 0,
        waypoints: trees.iter().map(|tree| tree.tree_size).sum(),
        paid: 0,
        paid_approx: false,
        pending_fee: 0,
        blocks: tip - start + 1,
        sent: 0,
        held: 0,
        quiet: 0,
        not_seen: 0,
        malformed: 0,
        already_held: 0,
    };
    for tree in trees {
        match tree.outcome {
            Outcome::OnTime { .. } => {
                summary.on_time += 1;
                summary.paid += tree.fee_sat;
                summary.paid_approx |= tree.fee_approx;
            }
            Outcome::Late { .. } => {
                summary.late += 1;
                summary.paid += tree.fee_sat;
                summary.paid_approx |= tree.fee_approx;
            }
            Outcome::Missed => summary.missed += 1,
            Outcome::InMempool { .. } => {
                summary.mempool += 1;
                summary.pending_fee += tree.fee_sat;
            }
        }
    }
    // The same rows the ledger draws. A tree row is a sent height even when
    // that height has no block record (970664). A gap height uses the block
    // decision the gap text uses. Counting the block map on its own drifts.
    let mut sent_heights = BTreeSet::new();
    for row in ledger {
        match row {
            LedgerRow::Tree { tree, .. } if (start..=tip).contains(&tree.broadcast_height) => {
                if sent_heights.insert(tree.broadcast_height) {
                    summary.sent += 1;
                }
            }
            LedgerRow::Gap { lo, hi, .. } => {
                for height in *lo..=*hi {
                    match blocks.get(&height).map(|block| block.decision) {
                        Some(DecisionKind::Sent) => summary.sent += 1,
                        Some(DecisionKind::Quiet) if offline.contains(&height) => {
                            summary.not_seen += 1
                        }
                        Some(DecisionKind::Quiet) => summary.quiet += 1,
                        Some(DecisionKind::Held(_)) => summary.held += 1,
                        None => summary.not_seen += 1,
                    }
                }
            }
            _ => {}
        }
    }
    for block in blocks.values() {
        if (start..=tip).contains(&block.height) {
            summary.malformed += block.malformed;
            summary.already_held += block.already_held;
        }
    }
    summary
}

fn live_min_c(reads: &OpsReads) -> Option<u32> {
    match &reads.fee {
        NodeRead::Ready(fee) => fee.min_sat_vb.and_then(centi),
        _ => None,
    }
}

fn forecast(inputs: &OpsInputs, reads: &OpsReads, tip: u32) -> (&'static str, String, Option<u32>) {
    let min_c = live_min_c(reads);
    if inputs.pool_len == 0 {
        return ("empty", "Empty · nothing to send".to_owned(), min_c);
    }
    let Some(fee) = (match &reads.fee {
        NodeRead::Ready(fee) => Some(fee),
        NodeRead::Absent => None,
        NodeRead::Failed { .. } => {
            return ("unknown", "Holding · no fee reading".to_owned(), None);
        }
    }) else {
        return finish_forecast(inputs, reads, tip, None, None);
    };
    let Some(min) = fee.min_sat_vb else {
        return ("unknown", "Holding · no fee reading".to_owned(), None);
    };
    let cap_c = centi(CAP).unwrap_or(110);
    if min_at_or_above_cap(centi(min).unwrap_or(0)) {
        return (
            "hold",
            format!(
                "Holding · min {} ≥ {} cap",
                fee::format_sat_vb(min),
                fee::format_sat_vb(CAP)
            ),
            min_c,
        );
    }
    let floor = fee.floor_sat_vb.unwrap_or(0.0);
    if centi(floor).unwrap_or(0) > cap_c {
        return (
            "hold",
            format!(
                "Holding · floor {} > {} cap",
                fee::format_sat_vb(floor),
                fee::format_sat_vb(CAP)
            ),
            min_c,
        );
    }
    let bid = fee
        .bid_sat_vb
        .unwrap_or_else(|| fee::would_bid(min, CAP, DEFAULT_FEE_MULTIPLIER));
    finish_forecast(inputs, reads, tip, Some(bid), min_c)
}

fn finish_forecast(
    _inputs: &OpsInputs,
    reads: &OpsReads,
    tip: u32,
    bid: Option<f64>,
    min_c: Option<u32>,
) -> (&'static str, String, Option<u32>) {
    let bid = bid.unwrap_or_else(|| fee::would_bid(0.42, CAP, DEFAULT_FEE_MULTIPLIER));
    let needed = fee::broadcast_fee_sats(bid, VSIZE).unwrap_or(0);
    let covered = match &reads.wallet {
        NodeRead::Ready(wallet) => covers_bid(wallet, bid),
        NodeRead::Absent => true,
        NodeRead::Failed { .. } => true,
    };
    if !covered {
        return (
            "hold",
            format!("Holding · no confirmed coin covers {needed} sat"),
            min_c,
        );
    }
    (
        "fire",
        format!(
            "Will fire at {} · bid {}",
            tip.saturating_add(1),
            fee::format_sat_vb(bid)
        ),
        min_c,
    )
}

fn covers_bid(wallet: &WalletReady, bid: f64) -> bool {
    let policy = UtxoPolicy {
        min_conf: wallet.min_conf,
        reserve_min: wallet.reserve_min,
        reserve_max: wallet.reserve_max,
        consolidate: wallet.consolidate,
    };
    let utxos = wallet
        .coins
        .iter()
        .enumerate()
        .map(|(index, coin)| PlantUtxo {
            txid: format!("{index:064x}"),
            vout: 0,
            amount_sat: coin.amount_sat,
            confirmations: coin.confirmations,
            spendable: coin.spendable,
            safe: coin.safe,
        })
        .collect::<Vec<_>>();
    utxo::select_plant_inputs(&utxos, &policy, bid, 0).is_ok()
}

fn wallet_view(reads: &OpsReads, summary: &Summary) -> WalletView {
    match &reads.wallet {
        NodeRead::Ready(wallet) => {
            let total = wallet.confirmed_sat.saturating_add(wallet.unconfirmed_sat);
            let runway = total / TREE_COST_AT_CAP;
            let per_day = f64::from(summary.trees) / 2.0;
            WalletView {
                known: true,
                reason: String::new(),
                confirmed: wallet.confirmed_sat,
                coins: wallet.confirmed_coins,
                unconfirmed: wallet.unconfirmed_sat,
                unconfirmed_coins: wallet.unconfirmed_coins,
                thin: (wallet.confirmed_coins as usize) < wallet.reserve_min,
                runway,
                per_day,
                days: (per_day > 0.0).then_some(runway as f64 / per_day),
            }
        }
        NodeRead::Failed { reason } => wallet_unknown(reason),
        NodeRead::Absent => wallet_unknown("test plane has no wallet"),
    }
}

fn wallet_unknown(reason: &str) -> WalletView {
    WalletView {
        known: false,
        reason: format!("Wallet not read · {reason}"),
        confirmed: 0,
        coins: 0,
        unconfirmed: 0,
        unconfirmed_coins: 0,
        thin: false,
        runway: 0,
        per_day: 0.0,
        days: None,
    }
}

fn system_view(
    inputs: &OpsInputs,
    reads: &OpsReads,
    tip: u32,
    now_unix_ms: u64,
    blocks: &BTreeMap<u32, BlockView>,
    start: u32,
) -> SystemView {
    let elapsed = now_unix_ms.saturating_sub(inputs.process.started_unix_ms) / 1000;
    let since = blocks
        .values()
        .filter(|block| {
            (start..=tip).contains(&block.height)
                && block.observed_unix_ms >= inputs.process.started_unix_ms
                && block.observed_unix_ms > 0
        })
        .map(|block| block.height)
        .min();
    let tip_seen_min = inputs
        .observed_unix_ms
        .map(|observed| now_unix_ms.saturating_sub(observed) / 60_000);
    let (known, node_blocks, node_headers, syncing) = match &reads.sync {
        NodeRead::Ready(sync) => (
            true,
            sync.blocks,
            sync.headers,
            sync.initialblockdownload || sync.blocks < sync.headers,
        ),
        _ => (false, 0, 0, false),
    };
    SystemView {
        known,
        version: format!("operator v{}", inputs.process.version),
        commit: inputs.process.commit.clone(),
        instance: inputs.process.instance.clone(),
        network: inputs.network.clone(),
        uptime: format_uptime(elapsed),
        since,
        node_blocks,
        node_headers,
        syncing,
        tip_lag: i64::from(node_blocks) - i64::from(tip),
        tip_seen_min,
    }
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let mins = (seconds % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{mins} min")
    }
}

pub fn evaluate_checks(view: &OpsView) -> Vec<Check> {
    let start = view.window_start;
    let tip = view.tip;
    let tree_rows: Vec<&TreeView> = view
        .ledger
        .iter()
        .filter_map(|row| match row {
            LedgerRow::Tree { tree, .. } => Some(tree),
            _ => None,
        })
        .collect();
    let _row_ids: Vec<u64> = tree_rows.iter().map(|tree| tree.seq).collect();
    let collecting_count = view.ledger.iter().find_map(|row| match row {
        LedgerRow::Collecting { count, .. } => Some(*count),
        _ => None,
    });
    let mempool_rows: Vec<u64> = tree_rows
        .iter()
        .filter(|tree| matches!(tree.outcome, Outcome::InMempool { .. }))
        .map(|tree| tree.seq)
        .collect();
    let mut covered = BTreeMap::<u32, u32>::new();
    for row in &view.ledger {
        match row {
            LedgerRow::Gap { lo, hi, .. } => {
                for height in *lo..=*hi {
                    *covered.entry(height).or_insert(0) += 1;
                }
            }
            LedgerRow::Tree { tree, .. } if (start..=tip).contains(&tree.broadcast_height) => {
                *covered.entry(tree.broadcast_height).or_insert(0) += 1;
            }
            _ => {}
        }
    }
    let window: Vec<u32> = (start..=tip).collect();
    let r5 = window.iter().all(|height| covered.get(height) == Some(&1));
    let mut r6 = true;
    let mut r6_detail = String::new();
    for block in view.blocks.values() {
        if !(start..=tip).contains(&block.height) || !matches!(block.decision, DecisionKind::Sent) {
            continue;
        }
        // A v2 line names its tree. A line from before that field does not;
        // the tree sent at this height is the one on the ledger row.
        let linked = match block.tree {
            Some(seq) => tree_rows.iter().copied().find(|tree| tree.seq == seq),
            None => tree_rows
                .iter()
                .copied()
                .find(|tree| tree.broadcast_height == block.height),
        };
        match linked {
            Some(tree) if tree.tree_size == block.pool_size => {}
            Some(tree) => {
                r6 = false;
                r6_detail.push_str(&format!(
                    " {} tree {} has {}, pool was {}",
                    block.height,
                    tree.seq,
                    plural(tree.tree_size, "waypoint", "waypoints"),
                    block.pool_size
                ));
            }
            None => {
                r6 = false;
                r6_detail.push_str(&format!(" {} tree none", block.height));
            }
        }
    }
    let outcomes =
        view.summary.on_time + view.summary.late + view.summary.missed + view.summary.mempool;
    let r7 = view.summary.trees == tree_rows.len() as u32
        && outcomes == view.summary.trees
        && view.summary.waypoints == tree_rows.iter().map(|tree| tree.tree_size).sum::<u32>();
    let r8 = view.summary.sent + view.summary.held + view.summary.quiet + view.summary.not_seen
        == view.summary.blocks
        && view.summary.blocks == LEDGER_BLOCKS;
    // +N is included − sent. +1 through +5, including +5, are on time.
    // A later inclusion is late. A tree with no inclusion is not on time.
    let mut r4 = true;
    let mut r4_detail = String::new();
    for tree in &view.trees {
        let n = tree
            .inclusion_height
            .map(|height| height.saturating_sub(tree.broadcast_height));
        let on_time = matches!(tree.outcome, Outcome::OnTime { .. });
        let expected = n.is_some_and(|n| (1..=WINDOW).contains(&n));
        if on_time != expected {
            r4 = false;
            r4_detail.push_str(&format!(" {}", tree.seq));
        }
        if let Outcome::OnTime { n: labelled } = tree.outcome {
            if Some(labelled) != n {
                r4 = false;
                r4_detail.push_str(&format!(" {} +{labelled}", tree.seq));
            }
        }
    }
    if !r4_detail.is_empty() {
        r4_detail = format!(" trees{r4_detail}");
    }
    // Waypoints come from pool blocks before the send. A leaf in two trees fails.
    // A record with no post heights is older than the pin and is not checked.
    let mut r12 = true;
    let mut r12_detail = String::new();
    let mut seen_leaves: BTreeMap<Hash32, u64> = BTreeMap::new();
    for tree in &view.trees {
        if !tree.post_heights.is_empty() && tree.post_heights.len() != tree.tree_size as usize {
            r12 = false;
            r12_detail.push_str(&format!(" {} post count", tree.seq));
        }
        for post in &tree.post_heights {
            if *post >= tree.broadcast_height {
                r12 = false;
                r12_detail.push_str(&format!(" {} post {post}", tree.seq));
            }
        }
        for leaf in &tree.leaves {
            if let Some(prior) = seen_leaves.insert(*leaf, tree.seq) {
                r12 = false;
                r12_detail.push_str(&format!(
                    " waypoint {} in {prior} and {}",
                    crate::encode_hash32(leaf),
                    tree.seq
                ));
            }
        }
    }
    let r9 = view.trees.iter().all(|tree| {
        tree.inclusion_height
            .is_none_or(|height| height > tree.broadcast_height)
    });
    let mut r9_detail = view
        .trees
        .iter()
        .filter(|tree| {
            tree.inclusion_height
                .is_some_and(|height| height <= tree.broadcast_height)
        })
        .map(|tree| tree.seq.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    if !r9_detail.is_empty() {
        r9_detail = format!(" trees {r9_detail}");
    }
    let mut r10 = true;
    let mut r10_detail = String::new();
    for tree in &view.trees {
        let Some(block) = view.blocks.get(&tree.broadcast_height) else {
            // No block line and no min on the tree: the record predates #273.
            if tree.next_block_min_c.is_none() {
                continue;
            }
            r10 = false;
            r10_detail.push_str(&format!(
                " {} missing block {}",
                tree.seq, tree.broadcast_height
            ));
            continue;
        };
        if block.bid_c.is_some() && block.bid_c != Some(tree.bid_c) {
            r10 = false;
            r10_detail.push_str(&format!(" {} bid", tree.seq));
            continue;
        }
        if block.bid_c.is_none() && (block.min_c.is_some() || tree.next_block_min_c.is_some()) {
            r10 = false;
            r10_detail.push_str(&format!(" {} bid", tree.seq));
            continue;
        }
        if tree.fee_source == "floor" {
            continue;
        }
        // The block line gained `next_block_min` in #276. The tree record
        // gained it in #273. Either one is the min the bid was taken from.
        // A record with neither is legacy history, not a failed bid.
        let Some(min_c) = block.min_c.or(tree.next_block_min_c) else {
            continue;
        };
        let expected = centi(fee::would_bid(
            f64::from(min_c) / 100.0,
            CAP,
            DEFAULT_FEE_MULTIPLIER,
        ));
        if expected != Some(tree.bid_c) {
            r10 = false;
            r10_detail.push_str(&format!(" {} would-bid", tree.seq));
        }
    }
    let mut r11 = true;
    let mut r11_detail = String::new();
    let window_blocks: Vec<&BlockView> = view
        .blocks
        .values()
        .filter(|block| (start..=tip).contains(&block.height))
        .collect();
    let mut previous: Option<&BlockView> = None;
    for block in &window_blocks {
        if let Some(prev) = previous {
            if let (Some(added), Some(_)) = (block.added, prev.added) {
                // `held (tree in mempool)` is a line from an older build. That build
                // sampled `added` after the seal had released the accept lock,
                // so a waypoint posted during the broadcast could sit in
                // `added` and not in `pool_size`. This clock does not write
                // that hold. The logged pair is history, not an eviction.
                let legacy = matches!(block.decision, DecisionKind::Held(HoldKind::TreeInMempool));
                if !legacy {
                    // Held and quiet leave the pool. A sent tree takes the pool it
                    // was built from, so nothing carries from a sent decision.
                    let carried = match prev.decision {
                        DecisionKind::Sent => 0,
                        _ => prev.pool_size,
                    };
                    if block.pool_size != carried.saturating_add(added) {
                        r11 = false;
                        r11_detail.push_str(&format!(" {}", block.height));
                    }
                }
            }
        }
        previous = Some(block);
    }
    match view.blocks.get(&tip) {
        Some(block) => {
            // `added_since_tip` counts accepts in this process only. After a
            // restart it is 0 while the open pool still holds waypoints
            // accepted since the tip was recorded. That is not an eviction.
            // The equation runs once this process has observed the tip.
            let observed_here =
                block.observed_unix_ms > 0 && block.observed_unix_ms >= view.started_unix_ms;
            if observed_here {
                let carried = match block.decision {
                    DecisionKind::Held(_) => block.pool_size,
                    _ => 0,
                };
                let expected = view.added_since_tip.saturating_add(carried);
                if view.pool_len != expected {
                    r11 = false;
                    r11_detail.push_str(&format!(
                        " pool has {}, expected {} ({} added since tip {} + {} carried at {})",
                        view.pool_len, expected, view.added_since_tip, tip, carried, tip
                    ));
                }
            }
        }
        None => {
            r11 = false;
            r11_detail.push_str(" tip has no block record");
        }
    }
    let highest = tree_rows
        .iter()
        .filter(|tree| tree.inclusion_height.is_some())
        .max_by_key(|tree| (tree.inclusion_height, tree.broadcast_height));
    let r3 = match &view.anchored {
        None => highest.is_none(),
        Some(anchored) if !anchored.in_window => true,
        Some(anchored) => highest.is_some_and(|tree| tree.seq == anchored.tree.seq),
    };
    let r3_detail = view
        .anchored
        .as_ref()
        .map(|anchored| format!(" tree {}", anchored.tree.seq))
        .unwrap_or_default();
    // The tile is oldest first. The ledger is newest first. The set is the check.
    let mut tile_ids: Vec<u64> = view.mempool.iter().map(|tree| tree.seq).collect();
    let mut row_ids = mempool_rows.clone();
    tile_ids.sort_unstable();
    row_ids.sort_unstable();
    let r2 = tile_ids == row_ids;
    let names = [
        (
            "R1",
            "R1 Collecting tile count equals the ledger's next-block row",
            view.collecting.count == collecting_count.unwrap_or(u32::MAX)
                && view.collecting.count == view.pool_len,
            String::new(),
        ),
        (
            "R2",
            "R2 In mempool tile lists every ledger In mempool row",
            r2,
            mempool_rows
                .iter()
                .map(|seq| seq.to_string())
                .collect::<Vec<_>>()
                .join(" "),
        ),
        (
            "R3",
            "R3 Last anchored is the ledger row with the highest inclusion height",
            r3,
            r3_detail,
        ),
        (
            "R4",
            "R4 On time is included − sent from 1 through 5, and +5 is inside the window",
            r4,
            r4_detail,
        ),
        (
            "R5",
            "R5 Every height in the window appears exactly once",
            r5,
            String::new(),
        ),
        (
            "R6",
            "R6 Each sent block names its tree and the tree size equals the pool at that decision",
            r6,
            r6_detail,
        ),
        (
            "R7",
            "R7 48 h counts equal the ledger counts",
            r7,
            String::new(),
        ),
        (
            "R8",
            "R8 Block counts add up to the window",
            r8,
            String::new(),
        ),
        (
            "R9",
            "R9 No tree is included at or below its broadcast height",
            r9,
            r9_detail,
        ),
        (
            "R10",
            "R10 Every bid is min(1.5 × min, cap) of its own block record",
            r10,
            r10_detail,
        ),
        (
            "R11",
            "R11 The pool only grows until a tree takes it (no eviction)",
            r11,
            r11_detail,
        ),
        (
            "R12",
            "R12 Every sent tree's waypoints were pooled before its send height, and no waypoint is in two trees",
            r12,
            r12_detail,
        ),
    ];
    names
        .into_iter()
        .map(|(id, name, holds, detail)| Check {
            id: id.to_owned(),
            name: name.to_owned(),
            holds,
            detail,
        })
        .collect()
}

fn banners(view: &OpsView, inputs: &OpsInputs, now_unix_ms: u64) -> Vec<Banner> {
    let mut banners = Vec::new();
    if let Some(observed) = inputs.observed_unix_ms {
        let age_min = now_unix_ms.saturating_sub(observed) / 60_000;
        if age_min >= 60 {
            let node = match &view.system.known {
                true => view.system.node_blocks.to_string(),
                false => "not reachable".to_owned(),
            };
            banners.push(Banner {
                issue: false,
                html: format!(
                    "<b>No new tip for {age_min} min.</b> Last tip {} was seen at {} UTC. Figures below are as of that tip. Node reports {node}.",
                    view.tip,
                    hhmm_utc(observed)
                ),
            });
        }
    }
    if view.system.known && view.system.tip_lag >= 2 {
        banners.push(Banner {
            issue: false,
            html: format!(
                "<b>Bari is {} behind its node.</b>",
                plural(
                    u32::try_from(view.system.tip_lag).unwrap_or(u32::MAX),
                    "block",
                    "blocks"
                )
            ),
        });
    }
    let issues: Vec<&Check> = view.checks.iter().filter(|check| !check.holds).collect();
    if !issues.is_empty() {
        let counted = plural(
            u32::try_from(issues.len()).unwrap_or(u32::MAX),
            "issue",
            "issues",
        );
        let body = issues
            .iter()
            .map(|check| format!("{}{}.", check.name, check.detail))
            .collect::<Vec<_>>()
            .join(" ");
        banners.push(Banner {
            issue: true,
            html: format!(
                "<b>Data check: {counted}.</b> {body} The tiles and the ledger may disagree until this is fixed."
            ),
        });
    }
    for issue in &inputs.store_issues {
        banners.push(Banner {
            issue: true,
            html: format!(
                "<b>Ledger incomplete.</b> {} could not be read ({}). Rows for that tree are missing.",
                esc(&issue.file),
                esc(&issue.error)
            ),
        });
    }
    banners
}

fn hhmm_utc(unix_ms: u64) -> String {
    let (_y, _m, _d, hour, min, _sec) = civil_utc(unix_ms / 1000);
    format!("{hour:02}:{min:02}")
}

fn format_utc(unix: u64) -> String {
    let (year, month, day, hour, min, sec) = civil_utc(unix);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{min:02}:{sec:02} UTC")
}

fn civil_utc(secs: u64) -> (i32, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = (rem / 3_600) as u32;
    let min = ((rem % 3_600) / 60) as u32;
    let sec = (rem % 60) as u32;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = (if month <= 2 { y + 1 } else { y }) as i32;
    (year, month, day, hour, min, sec)
}

fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(ch),
        }
    }
    out
}
fn chart_html(view: &OpsView) -> String {
    if view.test_plane {
        return "<section class=\"panel fees\" aria-label=\"Fees\"><header><h2>Fees · 48 h</h2><span class=\"sub\">sat/vB against block height</span></header><p class=\"empty-chart\">No fee gate on the test plane.</p><p class=\"note\">The decision strip and tree markers stay on the live plane. This plane does not read a fee.</p></section>".to_owned();
    }
    if view.blocks.values().all(|block| block.min_c.is_none()) && view.live_min_c.is_none() {
        return "<section class=\"panel fees\" aria-label=\"Fees\"><header><h2>Fees · 48 h</h2><span class=\"sub\">sat/vB against block height</span></header><p class=\"empty-chart\">No fee readings in this window. The test plane has no fee gate; on the live plane this means the node was not read at any block.</p></section>".to_owned();
    }
    format!(
        "<section class=\"panel fees\" aria-label=\"Fees\"><header><h2>Fees · 48 h</h2><span class=\"sub\">sat/vB against block height</span></header>{}{}{}</section>",
        chart_svg(view, true),
        chart_svg(view, false),
        legend_html()
    )
}

fn legend_html() -> String {
    format!(
        "<ul class=\"legend\"><li><i class=\"sw l-min\"></i>next-block min, read at each block</li><li><i class=\"sw l-cap\"></i>cap {}</li><li><i class=\"sw d-ontime\"></i>bid · on time (+1 through +{WINDOW})</li><li><i class=\"sw d-late\"></i>late</li><li><i class=\"sw d-missed\"></i>missed</li><li><i class=\"sw d-mempool\"></i>in mempool</li></ul><ul class=\"legend\"><li><i class=\"sw s-sent\"></i>sent</li><li><i class=\"sw s-held\"></i>held: fee</li><li><i class=\"sw s-coin\"></i>held: no confirmed coin</li><li><i class=\"sw s-quiet\"></i>quiet</li><li><i class=\"sw s-unseen\"></i>not seen</li><li><i class=\"sw s-next\"></i>collecting</li></ul><p class=\"note\">A dot is the bid when a tree was sent. Its line runs to the block that included it. +{WINDOW} is inside the window, so a longer line is a late tree. × ends a miss at sent + {WINDOW}. Values above 2 sat/vB sit in the hatched band with the peak labelled. A quiet row written long after the header, in one catch-up, reads not seen (Bari offline). held (tree in mempool, older build) is history from an older build.</p>",
        rate_text(centi(CAP))
    )
}

fn f1(value: f64) -> String {
    format!("{value:.1}")
}

fn f2(value: f64) -> String {
    format!("{value:.2}")
}

fn chart_svg(view: &OpsView, wide: bool) -> String {
    let (width, height) = if wide { (860.0, 300.0) } else { (360.0, 262.0) };
    let (left, right) = if wide { (40.0, 56.0) } else { (30.0, 46.0) };
    let over_top = 22.0;
    let over_h = 10.0;
    let top = over_top + over_h + 12.0;
    let bottom = height - if wide { 64.0 } else { 60.0 };
    let strip_y = bottom + 8.0;
    let strip_h = 8.0;
    let label_y = strip_y + strip_h + 15.0;
    let rel_y = label_y + 13.0;
    let start = view.window_start;
    let next = view.tip.saturating_add(1);
    let slots = f64::from(next - start + 1);
    let plot_w = width - left - right;
    let sw = plot_w / slots;
    let y_max = 2.0;
    let font = if wide { 11 } else { 10 };
    let x0 = |height: u32| left + f64::from(height - start) * sw;
    let xc = |height: u32| x0(height) + sw / 2.0;
    let y = |cents: u32| {
        let value = f64::from(cents) / 100.0;
        if value > y_max {
            over_top + over_h / 2.0
        } else {
            bottom - (value / y_max) * (bottom - top)
        }
    };
    let hatch_id = if wide { "hatchw" } else { "hatchc" };
    let class = if wide { "wide" } else { "compact" };
    let mut out = format!(
        "<svg class=\"chart {class}\" viewBox=\"0 0 {width} {height}\" role=\"img\" aria-label=\"Next-block minimum per block, the 1.1 cap, and each tree's bid with where it was included, over 48 hours\"><defs><pattern id=\"{hatch_id}\" width=\"4\" height=\"4\" patternUnits=\"userSpaceOnUse\" patternTransform=\"rotate(45)\"><line x1=\"0\" y1=\"0\" x2=\"0\" y2=\"4\" stroke=\"#3a4558\" stroke-width=\"1.2\"/></pattern></defs>"
    );
    let cap_c = centi(CAP).unwrap_or(110);
    out.push_str(&format!(
        "<rect x=\"{left}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#5cc98a\" fill-opacity=\"0.035\"/>",
        f1(y(cap_c)),
        f1(plot_w),
        f1(bottom - y(cap_c))
    ));
    let ticks: &[u32] = if wide {
        &[50, 100, 150, 200]
    } else {
        &[100, 200]
    };
    for cents in std::iter::once(0).chain(ticks.iter().copied()) {
        let yy = y(cents);
        out.push_str(&format!(
            "<line x1=\"{left}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#1f2734\" stroke-width=\"1\"/>",
            f1(yy),
            f1(left + plot_w),
            f1(yy)
        ));
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"{font}\" fill=\"#8b97a9\">{}</text>",
            f1(left - 6.0),
            f1(yy + 3.5),
            rate_text(Some(cents))
        ));
    }
    out.push_str(&format!(
        "<rect x=\"{left}\" y=\"{over_top}\" width=\"{}\" height=\"{over_h}\" fill=\"url(#{hatch_id})\" opacity=\"0.7\"/>",
        f1(plot_w)
    ));
    out.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"{}\" fill=\"#8b97a9\">&gt;2</text>",
        f1(left - 6.0),
        f1(over_top + over_h / 2.0 + 3.5),
        font - 1
    ));
    out.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"{font}\" fill=\"#8b97a9\">sat/vB</text>",
        f1(left - 6.0),
        f1(over_top - 10.0)
    ));
    let cy = y(cap_c);
    out.push_str(&format!(
        "<line x1=\"{left}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#e2b15a\" stroke-width=\"1.2\" stroke-dasharray=\"5 4\"/>",
        f1(cy),
        f1(left + plot_w),
        f1(cy)
    ));
    out.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" font-size=\"{font}\" fill=\"#e2b15a\">cap {}</text>",
        f1(left + plot_w + 5.0),
        f1(cy + 3.5),
        rate_text(Some(cap_c))
    ));
    let mut path = String::new();
    let mut prev = false;
    let mut peaks: Vec<(u32, u32)> = Vec::new();
    let mut run_peak: Option<(u32, u32)> = None;
    for height in start..=view.tip {
        let cents = view.blocks.get(&height).and_then(|block| block.min_c);
        let Some(cents) = cents else {
            prev = false;
            if let Some(peak) = run_peak.take() {
                peaks.push(peak);
            }
            continue;
        };
        let yy = y(cents);
        if !prev {
            path.push_str(&format!("M{},{}", f1(x0(height)), f1(yy)));
        } else {
            path.push_str(&format!("V{}", f1(yy)));
        }
        path.push_str(&format!("H{}", f1(x0(height) + sw)));
        prev = true;
        if f64::from(cents) > y_max * 100.0 {
            run_peak = Some(match run_peak {
                Some((at, peak)) if peak >= cents => (at, peak),
                _ => (height, cents),
            });
        } else if let Some(peak) = run_peak.take() {
            peaks.push(peak);
        }
    }
    if let Some(peak) = run_peak {
        peaks.push(peak);
    }
    out.push_str(&format!(
        "<path d=\"{path}\" fill=\"none\" stroke=\"#c3cfdd\" stroke-width=\"1.4\" stroke-linejoin=\"round\"/>"
    ));
    for (height, cents) in peaks {
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\" fill=\"#c3cfdd\">peak {}</text>",
            f1(xc(height)),
            f1(over_top - 3.0),
            font - 1,
            rate_text(Some(cents))
        ));
    }
    if let Some(cents) = view.live_min_c {
        let ly = y(cents);
        out.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#e2b15a\" fill-opacity=\"0.12\"/>",
            f1(x0(next)),
            f1(top),
            f1(sw),
            f1(bottom - top)
        ));
        out.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#c3cfdd\" stroke-width=\"1.4\" stroke-dasharray=\"2 2\"/>",
            f1(x0(next) - 1.0),
            f1(ly),
            f1(x0(next) + sw + 4.0),
            f1(ly)
        ));
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"{font}\" fill=\"#c3cfdd\">now {}</text>",
            f1(left + plot_w + 5.0),
            f1(ly + 3.5),
            rate_text(Some(cents))
        ));
    }
    let mut trees = view.trees.clone();
    trees.sort_by_key(|tree| tree.broadcast_height);
    for tree in trees {
        let kind = outcome_kind(&tree.outcome);
        let color = match tree.outcome {
            Outcome::OnTime { .. } => "#5cc98a",
            Outcome::Late { .. } => "#b48ef0",
            Outcome::Missed => "#ef7d7d",
            Outcome::InMempool { .. } => "#6ea8fe",
        };
        let yy = y(tree.bid_c);
        let title = format!(
            "Sent at {} · {} · bid {} · {}{}",
            tree.broadcast_height,
            plural(tree.tree_size, "waypoint", "waypoints"),
            rate_text(Some(tree.bid_c)),
            tree.inclusion_height
                .map(|height| format!("included {height} · "))
                .unwrap_or_default(),
            outcome_text(&tree.outcome)
        );
        out.push_str(&format!(
            "<g data-tree=\"{}\"><title>{}</title>",
            tree.seq,
            esc(&title)
        ));
        match tree.outcome {
            Outcome::OnTime { .. } | Outcome::Late { .. } => {
                let end = xc(tree.inclusion_height.unwrap_or(tree.broadcast_height));
                out.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{color}\" stroke-width=\"1.6\"/>",
                    f1(xc(tree.broadcast_height)),
                    f1(yy),
                    f1(end),
                    f1(yy)
                ));
                out.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{color}\" stroke-width=\"1.6\"/>",
                    f1(end),
                    f1(yy - 4.0),
                    f1(end),
                    f1(yy + 4.0)
                ));
            }
            Outcome::Missed => {
                let end = xc(tree.broadcast_height.saturating_add(WINDOW));
                out.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{color}\" stroke-width=\"1.6\"/>",
                    f1(xc(tree.broadcast_height)),
                    f1(yy),
                    f1(end),
                    f1(yy)
                ));
                out.push_str(&format!(
                    "<path d=\"M{},{}l6,6m0,-6l-6,6\" stroke=\"{color}\" stroke-width=\"1.6\"/>",
                    f1(end - 3.0),
                    f1(yy - 3.0)
                ));
            }
            Outcome::InMempool { .. } => {
                let end = xc(next);
                out.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{color}\" stroke-width=\"1.6\" stroke-dasharray=\"2 2\"/>",
                    f1(xc(tree.broadcast_height)),
                    f1(yy),
                    f1(end),
                    f1(yy)
                ));
                out.push_str(&format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"2.6\" fill=\"none\" stroke=\"{color}\" stroke-width=\"1.4\"/>",
                    f1(end),
                    f1(yy)
                ));
            }
        }
        let radius = if wide { 4.0 } else { 3.0 };
        out.push_str(&format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{radius}\" fill=\"{color}\" stroke=\"#11161f\" stroke-width=\"1.2\"/></g>",
            f1(xc(tree.broadcast_height)),
            f1(yy)
        ));
        let _ = kind;
    }
    out.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"{}\" fill=\"#8b97a9\">block</text>",
        f1(left - 6.0),
        f1(strip_y + strip_h - 1.0),
        font - 1
    ));
    for height in start..=next {
        let (fill, opacity, extra, label) = if height == next {
            (
                "none",
                "1",
                " stroke=\"#e2b15a\" stroke-width=\"1\"",
                "collecting",
            )
        } else {
            match view.blocks.get(&height).map(|block| block.decision) {
                Some(DecisionKind::Sent) => ("#cfd8e3", "1", "", "sent"),
                Some(DecisionKind::Held(kind)) => {
                    let colour = if matches!(kind, HoldKind::TreeInMempool) {
                        "#6ea8fe"
                    } else {
                        "#e2b15a"
                    };
                    let opacity = if matches!(kind, HoldKind::TreeInMempool) {
                        "0.55"
                    } else {
                        "0.85"
                    };
                    (colour, opacity, "", hold_text(kind))
                }
                Some(DecisionKind::Quiet) if view.offline.contains(&height) => {
                    ("", "1", "", "not seen (Bari offline)")
                }
                Some(DecisionKind::Quiet) => ("#263040", "1", "", "quiet"),
                None => ("", "1", "", "not seen"),
            }
        };
        let unseen = height != next
            && (!view.blocks.contains_key(&height) || view.offline.contains(&height));
        let fill_attr = if unseen {
            format!("url(#{hatch_id})")
        } else {
            fill.to_owned()
        };
        let title = if wide {
            format!("<title>{height} · {label}</title>")
        } else {
            String::new()
        };
        out.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{strip_h}\" fill=\"{fill_attr}\" fill-opacity=\"{opacity}\"{extra}>{title}</rect>",
            f2(x0(height)),
            f1(strip_y),
            f2(sw + 0.25)
        ));
    }
    let step = if wide { 50 } else { 100 };
    let tip_time = view
        .blocks
        .get(&view.tip)
        .and_then(|block| block.time)
        .unwrap_or(0);
    let mut tick = start + (step - start % step) % step;
    while tick <= view.tip {
        let x = xc(tick);
        out.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#8b97a9\" stroke-width=\"1\"/>",
            f1(x),
            f1(strip_y + strip_h),
            f1(x),
            f1(strip_y + strip_h + 4.0)
        ));
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"{font}\" fill=\"#8b97a9\">{tick}</text>",
            f1(x),
            f1(label_y)
        ));
        if tip_time > 0 {
            if let Some(block_time) = view.blocks.get(&tick).and_then(|block| block.time) {
                let hours = ((tip_time as i64 - block_time as i64) as f64 / 3600.0).round() as i64;
                let rel = if tick == view.tip {
                    "tip".to_owned()
                } else {
                    format!("−{hours} h")
                };
                out.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\" fill=\"#5e6a7c\">{rel}</text>",
                    f1(x),
                    f1(rel_y),
                    font - 1
                ));
            }
        }
        tick += step;
    }
    out.push_str("</svg>");
    out
}
fn tx_href(display: &str) -> String {
    format!("https://mempool.space/tx/{display}")
}

fn block_href(height: u32) -> String {
    format!("https://mempool.space/block/{height}")
}

fn h_link(height: u32, txid: &str, explorer: bool) -> String {
    if explorer {
        format!(
            "<a href=\"{}\" title=\"mempool.space tx {}…{}\">{height}</a>",
            tx_href(txid),
            &txid[..8.min(txid.len())],
            &txid[txid.len().saturating_sub(8)..]
        )
    } else {
        height.to_string()
    }
}

fn header_html(tip: u32, instance: &str, explorer: bool) -> String {
    let tip_html = if explorer {
        format!("<a href=\"{}\">{tip}</a>", block_href(tip))
    } else {
        tip.to_string()
    };
    format!(
        "<header class=\"head\"><p><span>groundinglab</span><span class=\"sep\">·</span><b>BARI</b><span class=\"long\">, the Bitcoin Anchored Registry Index</span><span class=\"sep\">·</span><span><span class=\"word\">operator </span>v{VERSION}</span><span class=\"sep\">·</span><span>{}</span><span class=\"sep\">·</span><span>tip {tip_html}</span></p></header>",
        esc(instance)
    )
}

fn kv(rows: &[(&str, String)]) -> String {
    let mut inner = String::new();
    for (label, value) in rows {
        inner.push_str(&format!("<div><dt>{label}</dt><dd>{value}</dd></div>"));
    }
    format!("<dl class=\"kv\">{inner}</dl>")
}

fn collecting_html(tile: &CollectingTile, tip: u32) -> String {
    let oldest = match tile.oldest {
        Some(height) => format!(
            "posted at {height} · {} ago",
            plural(tip.saturating_sub(height), "block", "blocks")
        ),
        None => "–".to_owned(),
    };
    // `min_at_or_above_cap` is the ledger's `held (min ≥ cap)` gate. Name a
    // bid only while the minimum is under the cap, which is when a tree can go out.
    let min_text = match tile.min_c {
        Some(cents) if min_at_or_above_cap(cents) => format!(
            "{} sat/vB · above cap {} · holding",
            rate_text(Some(cents)),
            rate_text(centi(CAP))
        ),
        Some(cents) => format!(
            "{} sat/vB · would bid {} · cap {}",
            rate_text(Some(cents)),
            rate_text(Some(
                centi(fee::would_bid(
                    f64::from(cents) / 100.0,
                    CAP,
                    DEFAULT_FEE_MULTIPLIER
                ))
                .unwrap_or(0)
            )),
            rate_text(centi(CAP))
        ),
        None => "<span class=\"dim\">not read · node not reachable</span>".to_owned(),
    };
    let pool_words = plural(tile.count, "waypoint", "waypoints");
    let noun = pool_words
        .split_once(' ')
        .map(|(_, noun)| noun)
        .unwrap_or("waypoints");
    format!(
        "<article class=\"stage collecting\" data-view=\"collecting\"><header><h2>Collecting</h2><span class=\"meta\">next block <b>{}</b></span></header><p class=\"big\"><span data-count=\"pool\">{}</span> <small>{noun}</small></p><p class=\"forecast {}\">{}</p>{}</article>",
        tile.next,
        tile.count,
        tile.forecast,
        esc(&tile.forecast_text),
        kv(&[("Oldest", oldest), ("Min now", min_text)])
    )
}

fn window_bar(n: u32) -> String {
    let mut cells = String::new();
    for index in 0..WINDOW {
        let class = match index.cmp(&n) {
            std::cmp::Ordering::Less => " class=\"on\"",
            std::cmp::Ordering::Equal => " class=\"next\"",
            std::cmp::Ordering::Greater => "",
        };
        cells.push_str(&format!("<i{class}></i>"));
    }
    format!(
        "<div class=\"wbar\" role=\"img\" aria-label=\"{n} of {WINDOW} blocks passed\">{cells}</div>"
    )
}

fn mempool_entry(tree: &TreeView, explorer: bool) -> String {
    let n = match tree.outcome {
        Outcome::InMempool { n } => n,
        _ => 0,
    };
    format!(
        "<div class=\"inflight\" data-tree=\"{}\"><p class=\"big\">+{n} <small>of {WINDOW}</small></p>{}{}</div>",
        tree.seq,
        window_bar(n),
        kv(&[
            (
                "Sent at",
                h_link(tree.broadcast_height, &tree.txid_display, explorer),
            ),
            ("Waypoints", tree.tree_size.to_string()),
            ("Bid", format!("{} sat/vB", rate_text(Some(tree.bid_c)))),
            (
                "Window",
                format!(
                    "{}–{} · +{WINDOW} is on time",
                    tree.broadcast_height.saturating_add(1),
                    tree.broadcast_height.saturating_add(WINDOW)
                ),
            ),
        ])
    )
}

fn mempool_html(trees: &[TreeView], explorer: bool) -> String {
    if trees.is_empty() {
        return format!(
            "<article class=\"stage mempool empty\" data-view=\"mempool\"><header><h2>In mempool</h2><span class=\"meta\">–</span></header><p class=\"big dim\">None</p><p class=\"forecast empty\">No tree waiting for a block</p>{}</article>",
            kv(&[("Window", format!("{WINDOW} blocks from the broadcast height. +{WINDOW} is on time"))])
        );
    }
    let count = trees.len() as u32;
    let counted = plural(count, "tree", "trees");
    let entries = trees
        .iter()
        .map(|tree| mempool_entry(tree, explorer))
        .collect::<String>();
    format!(
        "<article class=\"stage mempool\" data-view=\"mempool\" data-count=\"{count}\"><header><h2>In mempool</h2><span class=\"meta\">{counted}</span></header>{entries}</article>"
    )
}

fn anchored_html(tile: Option<&AnchoredTile>, explorer: bool) -> String {
    let Some(tile) = tile else {
        return "<article class=\"stage anchored empty\" data-view=\"last-anchored\"><header><h2>Last anchored</h2><span class=\"meta\">–</span></header><p class=\"big dim\">None yet</p><p class=\"forecast empty\">No tree has been included in a block</p></article>".to_owned();
    };
    let tree = &tile.tree;
    let kind = outcome_kind(&tree.outcome);
    let inclusion = tree.inclusion_height.unwrap_or(0);
    let older = if tile.in_window {
        String::new()
    } else {
        "<p class=\"note\">Older than the 48 h ledger.</p>".to_owned()
    };
    let ago = format!("{} ago", plural(tile.ago, "block", "blocks"));
    format!(
        "<article class=\"stage anchored {kind}\" data-view=\"last-anchored\" data-tree=\"{}\"><header><h2>Last anchored</h2><span class=\"meta\">{ago}</span></header><p class=\"big\">{}</p><p class=\"result {kind}\" title=\"{}\">{}</p>{}{older}</article>",
        tree.seq,
        h_link(inclusion, &tree.txid_display, explorer),
        esc(&outcome_title(tree.broadcast_height, &tree.outcome)),
        outcome_text(&tree.outcome),
        kv(&[
            ("Waypoints", tree.tree_size.to_string()),
            (
                "Sent at",
                h_link(tree.broadcast_height, &tree.txid_display, explorer),
            ),
        ])
    )
}

fn pipeline_html(view: &OpsView) -> String {
    format!(
        "<section class=\"pipe\" aria-label=\"Pipeline\">{}<div class=\"link\" aria-hidden=\"true\"><span>›</span></div>{}<div class=\"divide\" aria-hidden=\"true\"><span>in a block</span></div>{}</section>",
        collecting_html(&view.collecting, view.tip),
        mempool_html(&view.mempool, view.explorer),
        anchored_html(view.anchored.as_ref(), view.explorer)
    )
}

fn ledger_html(view: &OpsView, rows: &[LedgerRow], sub: &str) -> String {
    let mut body = String::new();
    for row in rows {
        match row {
            LedgerRow::Collecting {
                next,
                count,
                forecast_text,
            } => {
                body.push_str(&format!(
                    "<tr class=\"r-collecting\" data-row=\"collecting\" title=\"{}\"><td><span class=\"hgt\">{next}</span><span class=\"tag\">next</span></td><td class=\"r\" data-count=\"pool\">{count}</td><td class=\"dim\">–</td><td><span class=\"res collecting\">Collecting</span></td></tr>",
                    esc(forecast_text)
                ));
            }
            LedgerRow::Tree { tree, pinned } => {
                let kind = outcome_kind(&tree.outcome);
                let inc = match tree.inclusion_height {
                    Some(height) => h_link(height, &tree.txid_display, view.explorer),
                    None => "<span class=\"dim\">–</span>".to_owned(),
                };
                let pin = if *pinned { " pinned" } else { "" };
                body.push_str(&format!(
                    "<tr class=\"r-tree {kind}{pin}\" data-tree=\"{}\"><td class=\"hgt\">{}</td><td class=\"r\">{}</td><td>{inc}</td><td><span class=\"res {kind}\" title=\"{}\">{}</span></td></tr>",
                    tree.seq,
                    h_link(tree.broadcast_height, &tree.txid_display, view.explorer),
                    tree.tree_size,
                    esc(&outcome_title(tree.broadcast_height, &tree.outcome)),
                    outcome_text(&tree.outcome)
                ));
            }
            LedgerRow::Gap {
                lo,
                hi,
                n,
                parts,
                title,
            } => {
                let range = if hi > lo {
                    format!("{lo}–{hi}")
                } else {
                    lo.to_string()
                };
                let parts = parts
                    .iter()
                    .map(|part| esc(part))
                    .collect::<Vec<_>>()
                    .join(" · ");
                body.push_str(&format!(
                    "<tr class=\"r-gap\" title=\"{}\"><td colspan=\"4\"><span class=\"rng\">{range}</span> · {} · {parts}</td></tr>",
                    esc(title),
                    plural(*n, "block", "blocks")
                ));
            }
        }
    }
    format!(
        "<section class=\"panel ledger\" aria-label=\"Ledger\"><header><h2>Ledger · 48 h</h2><span class=\"sub\">{sub}</span></header><div class=\"scroll\"><table><colgroup><col class=\"c-sent\"><col class=\"c-wp\"><col class=\"c-inc\"><col class=\"c-res\"></colgroup><thead><tr><th>Sent at</th><th class=\"r\"><span class=\"wide\">Waypoints</span><span class=\"narrow\">Wpts</span></th><th>Included</th><th>Result</th></tr></thead><tbody>{body}</tbody></table></div></section>"
    )
}

fn summary_html(summary: &Summary) -> String {
    let paid = if summary.pending_fee > 0 {
        format!(
            "{} sat <span class=\"dim\">+ {} in mempool</span>",
            group_sats(summary.paid),
            group_sats(summary.pending_fee)
        )
    } else if summary.paid_approx {
        format!("≈ {} sat", group_sats(summary.paid))
    } else {
        format!("{} sat", group_sats(summary.paid))
    };
    let paid = if summary.paid_approx && summary.pending_fee > 0 {
        format!(
            "≈ {} sat <span class=\"dim\">+ {} in mempool</span>",
            group_sats(summary.paid),
            group_sats(summary.pending_fee)
        )
    } else {
        paid
    };
    format!(
        "<article class=\"panel tile\" data-view=\"summary\"><h2>48 h</h2>{}</article>",
        kv(&[
            ("Trees sent", format!("<b>{}</b>", summary.trees)),
            (
                "On time",
                format!("<span class=\"c-ontime\">{}</span>", summary.on_time)
            ),
            (
                "Late",
                format!("<span class=\"c-late\">{}</span>", summary.late)
            ),
            (
                "Missed",
                format!("<span class=\"c-missed\">{}</span>", summary.missed)
            ),
            (
                "In mempool",
                format!("<span class=\"c-mempool\">{}</span>", summary.mempool)
            ),
            ("Waypoints in trees", summary.waypoints.to_string()),
            ("Fees paid", paid),
            (
                "Blocks",
                format!(
                    "{} <span class=\"dim\">· {} sent · {} held · {} quiet · {} not seen</span>",
                    summary.blocks, summary.sent, summary.held, summary.quiet, summary.not_seen
                ),
            ),
            (
                "Posts not added",
                format!(
                    "{} malformed · {} already held",
                    summary.malformed, summary.already_held
                ),
            ),
        ])
    )
}

fn days_text(days: f64) -> String {
    if days >= 10.0 {
        format!("{days:.0}")
    } else {
        format!("{days:.1}")
    }
}

fn pace_text(per_day: f64) -> String {
    if per_day.fract() == 0.0 {
        format!("{}", per_day as u64)
    } else {
        let text = format!("{per_day:.4}");
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

fn wallet_html(wallet: &WalletView) -> String {
    if !wallet.known {
        return format!(
            "<article class=\"panel tile\" data-view=\"wallet\"><h2>Wallet</h2>{}<p class=\"note warn\">{}</p></article>",
            kv(&[
                ("Confirmed", "–".to_owned()),
                ("Unconfirmed", "–".to_owned()),
                ("Runway", "–".to_owned()),
            ]),
            esc(&wallet.reason)
        );
    }
    let thin = if wallet.thin {
        " <span class=\"c-held\">thin</span>"
    } else {
        ""
    };
    let pace = match wallet.days {
        Some(days) => format!(
            "<span class=\"dim\">≈ {} days at the 48 h pace ({} trees/day)</span>",
            days_text(days),
            pace_text(wallet.per_day)
        ),
        None => "<span class=\"dim\">no trees in 48 h, so no pace</span>".to_owned(),
    };
    let runway_words = plural(u32::from(wallet.runway == 1), "tree", "trees");
    let runway_noun = runway_words
        .split_once(' ')
        .map(|(_, noun)| noun)
        .unwrap_or("trees");
    format!(
        "<article class=\"panel tile\" data-view=\"wallet\"><h2>Wallet</h2>{}<p class=\"note\">{TREE_COST_AT_CAP} sat a tree = {} sat/vB × {VSIZE} vB. Runway counts confirmed and unconfirmed sats. The plant spends only confirmed coins. Several trees can be in the mempool, so keep about 6 confirmed coins. Split the wallet once; Bari does not split it.</p></article>",
        kv(&[
            (
                "Confirmed",
                format!(
                    "<b>{}</b> sat · {}{thin}",
                    group_sats(wallet.confirmed),
                    plural(wallet.coins, "coin", "coins")
                ),
            ),
            (
                "Unconfirmed",
                format!(
                    "{} sat · {}",
                    group_sats(wallet.unconfirmed),
                    plural(wallet.unconfirmed_coins, "coin", "coins")
                ),
            ),
            (
                "Runway",
                format!(
                    "<b>{}</b> {runway_noun} at the {} cap<br>{pace}",
                    wallet.runway,
                    rate_text(centi(CAP))
                ),
            ),
        ]),
        rate_text(centi(CAP))
    )
}

fn lag_html(system: &SystemView) -> String {
    if system.tip_lag == 0 {
        return plural(0, "block", "blocks");
    }
    let text = if system.tip_lag > 0 {
        format!(
            "Bari {} behind the node",
            plural(system.tip_lag as u32, "block", "blocks")
        )
    } else {
        format!(
            "node {} behind Bari",
            plural((-system.tip_lag) as u32, "block", "blocks")
        )
    };
    if system.tip_lag.abs() > 1 {
        format!("<span class=\"c-held\">{text}</span>")
    } else {
        text
    }
}

fn system_html(system: &SystemView) -> String {
    let (node, lag) = if !system.known {
        (
            "<span class=\"c-missed\">not reachable</span>".to_owned(),
            "<span class=\"dim\">–</span>".to_owned(),
        )
    } else if system.syncing {
        (
            format!(
                "<span class=\"c-held\">syncing</span> · {} of {} headers",
                system.node_blocks, system.node_headers
            ),
            lag_html(system),
        )
    } else {
        (format!("synced · {}", system.node_blocks), lag_html(system))
    };
    let seen = match system.tip_seen_min {
        Some(mins) => format!(" <span class=\"dim\">· tip seen {mins} min ago</span>"),
        None => String::new(),
    };
    let since = match system.since {
        Some(height) => format!(" <span class=\"dim\">· since {height}</span>"),
        None => String::new(),
    };
    format!(
        "<article class=\"panel tile\" data-view=\"system\"><h2>System</h2>{}</article>",
        kv(&[
            ("Version", system.version.clone()),
            ("Commit", format!("<code>{}</code>", esc(&system.commit))),
            (
                "Instance",
                format!("{} · {}", esc(&system.instance), esc(&system.network))
            ),
            ("Uptime", format!("{}{since}", system.uptime)),
            ("Node sync", node),
            ("Tip lag", format!("{lag}{seen}")),
        ])
    )
}

fn footer_html(view: &OpsView) -> String {
    let ok = view.checks.iter().filter(|check| check.holds).count();
    let issues = view.checks.iter().filter(|check| !check.holds).count();
    let state = if issues == 0 {
        format!("{ok} of {} hold", view.checks.len())
    } else {
        format!(
            "<span class=\"c-missed\">{}</span>",
            plural(u32::try_from(issues).unwrap_or(u32::MAX), "issue", "issues")
        )
    };
    let mut detail = String::new();
    for check in &view.checks {
        detail.push_str(&format!("<li>{}</li>", esc(&check.name)));
    }
    format!(
        "<footer class=\"foot\"><span>Rendered {} at tip {}</span><details><summary>Data checks: {state}</summary><ul>{detail}</ul></details></footer>",
        format_utc(view.now_unix / 1000),
        view.tip
    )
}

fn age_script(now_unix_ms: u64) -> String {
    format!(
        "<script>(function(){{var at={now_unix_ms};function mark(){{var age=Math.floor((Date.now()-at)/60000);if(age<5)return;var host=document.querySelector('.banners');if(!host||host.querySelector('.js-age'))return;var banner=document.createElement('div');banner.className='banner js-age';banner.innerHTML='<b>This view is '+age+' min old.</b> Reload for the current tip.';host.prepend(banner);}}setInterval(mark,15000);mark();}})();</script>"
    )
}

pub fn render(view: &OpsView) -> String {
    let sub = format!(
        "{}–{} · {LEDGER_BLOCKS} blocks · newest first",
        view.window_start, view.tip
    );
    let mut banners = String::new();
    for banner in &view.banners {
        let class = if banner.issue {
            "banner issue"
        } else {
            "banner"
        };
        banners.push_str(&format!("<div class=\"{class}\">{}</div>", banner.html));
    }
    let body = format!(
        "{}<main><div class=\"banners\">{banners}</div>{}{}{}<div class=\"tiles\">{}{}{}</div></main>{}{}",
        header_html(view.tip, &view.instance, view.explorer),
        pipeline_html(view),
        ledger_html(view, &view.ledger, &sub),
        chart_html(view),
        summary_html(&view.summary),
        wallet_html(&view.wallet),
        system_html(&view.system),
        footer_html(view),
        age_script(view.now_unix)
    );
    page(
        &format!("BARI operator · {} · tip {}", view.instance, view.tip),
        &body,
        "",
    )
}

fn page(title: &str, body: &str, extra_css: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<style>{CSS}{extra_css}</style>\n</head>\n<body>\n{body}\n</body>\n</html>\n",
        esc(title)
    )
}
fn arrivals(height: u32) -> u32 {
    match height {
        970_439 => 3,
        970_471 => 2,
        970_499 => 7,
        970_541 => 3,
        970_599 => 6,
        970_639 => 5,
        970_650 => 3,
        970_664 => 8,
        970_674 => 3,
        970_676 => 4,
        970_678 => 2,
        970_680 => 3,
        970_683 => 2,
        970_685 => 3,
        970_686 => 2,
        970_690 => 4,
        970_693 => 4,
        970_698 => 3,
        970_699 => 4,
        970_700 => 2,
        _ => 0,
    }
}

fn inclusion_offset(height: u32) -> Option<u32> {
    match height {
        970_440 | 970_479 | 970_665 => Some(1),
        970_557 => Some(2),
        970_640 => Some(3),
        970_500 => Some(5),
        970_682 => Some(8),
        _ => None,
    }
}

fn header_time(height: u32) -> u64 {
    let jitter = ((u64::from(height).wrapping_mul(2_654_435_761)) % 481) as i64 - 240;
    let base = TIP_TIME as i64 - (TIP as i64 - height as i64) * 600;
    let time = if height == TIP { base } else { base + jitter };
    time as u64
}

fn next_block_min_c(height: u32) -> u32 {
    if matches!(height, 970_682 | 970_665 | 970_698) {
        return match height {
            970_682 => 40,
            970_665 => 52,
            _ => 42,
        };
    }
    let runs = [
        (970_470, 970_478, 1.55),
        (970_541, 970_556, 3.80),
        (970_651, 970_664, 1.45),
        (970_675, 970_681, 1.75),
        (970_686, 970_697, 2.40),
    ];
    for (start, end, peak) in runs {
        if (start..=end).contains(&height) {
            let t = f64::from(height - start) / f64::from((end - start).max(1));
            let value = 1.12 + (peak - 1.12) * (std::f64::consts::PI * t).sin();
            return (value * 100.0).round() as u32;
        }
    }
    let h = f64::from(height);
    let base = 0.40 + 0.07 * (h / 9.0).sin() + 0.05 * (h / 23.0).sin();
    (base * 100.0).round().max(26.0) as u32
}

fn malformed_at(height: u32) -> u32 {
    match height {
        970_512 => 2,
        970_633 => 1,
        _ => 0,
    }
}

fn already_held_at(height: u32) -> u32 {
    match height {
        970_499 => 2,
        970_664 => 1,
        970_686 => 2,
        _ => 0,
    }
}

fn fixture_txid(seq: u64) -> Hash32 {
    let digest = Sha256::digest(format!("bari-mockup-tree-{seq}").as_bytes());
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&digest);
    bytes.reverse();
    bytes
}

struct SimTree {
    seq: u64,
    broadcast: u32,
    size: u32,
    min_c: u32,
    bid_c: u32,
    inclusion: Option<u32>,
}

fn fixture_parts(instance: &str, commit: &str) -> (OpsInputs, OpsReads, u64) {
    let start = TIP - LEDGER_BLOCKS + 1;
    let mut sim: Vec<SimTree> = Vec::new();
    let mut plan: BTreeMap<u64, Option<u32>> = BTreeMap::new();
    let mut pool: Vec<u32> = Vec::new();
    let mut blocks = Vec::new();
    let mut seq = FIRST_SEQ;
    let started_ms = RENDERED * 1000 - 104_400 * 1000;
    for height in start..=TIP {
        if (970_520..970_524).contains(&height) {
            continue;
        }
        for tree in &mut sim {
            if tree.inclusion.is_none() {
                if let Some(offset) = plan.get(&tree.seq).copied().flatten() {
                    if tree.broadcast.saturating_add(offset) <= height {
                        tree.inclusion = Some(tree.broadcast + offset);
                    }
                }
            }
        }
        let min_c = next_block_min_c(height);
        let cohort = pool.len() as u32;
        // A tree already in the mempool does not hold this block. Fee does.
        let (decision, bid_c, tree_id) = if cohort == 0 {
            (DecisionKind::Quiet, None, None)
        } else if min_at_or_above_cap(min_c) {
            let bid = centi(fee::would_bid(
                f64::from(min_c) / 100.0,
                CAP,
                DEFAULT_FEE_MULTIPLIER,
            ))
            .unwrap_or(110);
            (DecisionKind::Held(HoldKind::MinAboveCap), Some(bid), None)
        } else {
            let bid = centi(fee::would_bid(
                f64::from(min_c) / 100.0,
                CAP,
                DEFAULT_FEE_MULTIPLIER,
            ))
            .unwrap_or(0);
            let id = seq;
            sim.push(SimTree {
                seq,
                broadcast: height,
                size: cohort,
                min_c,
                bid_c: bid,
                inclusion: None,
            });
            plan.insert(seq, inclusion_offset(height));
            seq += 1;
            pool.clear();
            (DecisionKind::Sent, Some(bid), Some(id))
        };
        blocks.push(BlockInput {
            height,
            time: Some(header_time(height)),
            // One stamp for the whole window looks like a catch-up burst.
            // The header time is when this height was observed.
            observed_unix_ms: header_time(height).saturating_mul(1000),
            decision,
            next_block_min_sat_vb: Some(f64::from(min_c) / 100.0),
            bid_sat_vb: bid_c.map(|cents| f64::from(cents) / 100.0),
            pool_size: cohort,
            added: Some(arrivals(height.saturating_sub(1))),
            malformed: malformed_at(height),
            already_held: already_held_at(height),
            tree: tree_id,
        });
        for _ in 0..arrivals(height) {
            pool.push(height);
        }
    }
    let trees = sim
        .iter()
        .map(|tree| {
            let bid = f64::from(tree.bid_c) / 100.0;
            TreeInput {
                batch_sequence: tree.seq,
                broadcast_height: tree.broadcast,
                tree_size: tree.size,
                txid: fixture_txid(tree.seq),
                inclusion_height: tree.inclusion,
                next_block_min_sat_vb: Some(f64::from(tree.min_c) / 100.0),
                bid_sat_vb: bid,
                fee_sat: fee::broadcast_fee_sats(bid, VSIZE),
                vsize: Some(VSIZE as u32),
                fee_source: "mempool".to_owned(),
                post_heights: (0..tree.size)
                    .map(|index| tree.broadcast.saturating_sub(1 + index))
                    .collect(),
                leaves: (0..tree.size)
                    .map(|index| {
                        let mut leaf = [0u8; 32];
                        leaf[..8].copy_from_slice(&tree.seq.to_le_bytes());
                        leaf[8..12].copy_from_slice(&index.to_le_bytes());
                        leaf
                    })
                    .collect(),
            }
        })
        .collect();
    let inputs = OpsInputs {
        tip_height: TIP,
        tip_time: Some(TIP_TIME),
        observed_unix_ms: Some(TIP_TIME * 1000),
        trees,
        blocks,
        pool_len: pool.len() as u32,
        oldest_post_height: pool.iter().copied().min(),
        added_since_tip: arrivals(TIP),
        malformed_since_tip: 0,
        already_held_since_tip: 0,
        store_issues: Vec::new(),
        process: ProcessFacts {
            version: VERSION.to_owned(),
            commit: commit.to_owned(),
            instance: instance.to_owned(),
            started_unix_ms: started_ms,
        },
        network: "mainnet".to_owned(),
    };
    let reads = OpsReads {
        fee: NodeRead::Ready(FeeReady {
            min_sat_vb: Some(0.42),
            floor_sat_vb: Some(0.1),
            bid_sat_vb: Some(fee::would_bid(0.42, CAP, DEFAULT_FEE_MULTIPLIER)),
            source: Some("mempool".to_owned()),
        }),
        wallet: NodeRead::Ready(WalletReady {
            confirmed_sat: 12_863,
            unconfirmed_sat: 8_585,
            confirmed_coins: 3,
            unconfirmed_coins: 1,
            coins: vec![
                WalletCoin {
                    amount_sat: 5_000,
                    confirmations: 6,
                    spendable: true,
                    safe: true,
                },
                WalletCoin {
                    amount_sat: 4_000,
                    confirmations: 4,
                    spendable: true,
                    safe: true,
                },
                WalletCoin {
                    amount_sat: 3_863,
                    confirmations: 2,
                    spendable: true,
                    safe: true,
                },
                WalletCoin {
                    amount_sat: 8_585,
                    confirmations: 0,
                    spendable: true,
                    safe: true,
                },
            ],
            min_conf: 1,
            reserve_min: 2,
            reserve_max: 3,
            consolidate: false,
        }),
        sync: NodeRead::Ready(SyncReady {
            blocks: TIP,
            headers: TIP,
            initialblockdownload: false,
        }),
    };
    (inputs, reads, RENDERED * 1000)
}

/// Built-in fixture. Heights and trees are the mockup seed.
pub fn fixture_view(instance: &str, commit: &str) -> OpsView {
    let (inputs, reads, now) = fixture_parts(instance, commit);
    build_ops_view(&inputs, &reads, now)
}

pub fn render_states(view: &OpsView) -> String {
    let base = view.collecting.clone();
    let mempool_tree = view
        .mempool
        .first()
        .cloned()
        .or_else(|| view.trees.first().cloned());
    let on_time = view
        .trees
        .iter()
        .find(|tree| matches!(tree.outcome, Outcome::OnTime { .. }));
    let mut specs = String::new();
    specs.push_str(&state_group(
        "Banners (above the pipeline)",
        &[
            spec("Stale: no new tip for 60 min or more (amber); the whole page is as of that tip", "<div class=\"banner\"><b>No new tip for 74 min.</b> Last tip 970700 was seen at 04:13 UTC. Figures below are as of that tip. Node reports 970700.</div>"),
            spec("Data check issue: a reconciliation rule does not hold (red)", "<div class=\"banner issue\"><b>Data check: 1 issue.</b> R3 Last anchored tree 4410 is not a ledger row. The tiles and the ledger may disagree until this is fixed.</div>"),
            spec("Store read issue: a record could not be read (red)", "<div class=\"banner issue\"><b>Ledger incomplete.</b> batches/batch-00000000000000004409.json could not be read (JSON error at line 1). Rows for that tree are missing.</div>"),
            spec("Page left open: the script marks the render old after 5 min", "<div class=\"banner\"><b>This view is 12 min old.</b> Reload for the current tip.</div>"),
        ],
    ));
    let mut fire = base.clone();
    fire.forecast = "fire";
    fire.forecast_text = format!("Will fire at {} · bid 0.63", view.tip + 1);
    fire.min_c = Some(42);
    let mut fee_hold = base.clone();
    fee_hold.count = 14;
    fee_hold.oldest = Some(view.tip.saturating_sub(6));
    fee_hold.forecast = "hold";
    fee_hold.forecast_text = "Holding · min 2.2 ≥ 1.1 cap".to_owned();
    fee_hold.min_c = Some(220);
    let mut coin = base.clone();
    coin.forecast = "hold";
    coin.forecast_text = "Holding · no confirmed coin covers 101 sat".to_owned();
    coin.min_c = Some(42);
    let mut unread = base.clone();
    unread.forecast = "unknown";
    unread.forecast_text = "Holding · no fee reading".to_owned();
    unread.min_c = None;
    let mut empty = base.clone();
    empty.count = 0;
    empty.oldest = None;
    empty.forecast = "empty";
    empty.forecast_text = "Empty · nothing to send".to_owned();
    specs.push_str(&state_group(
        "Collecting",
        &[
            spec(
                "Will fire: pool not empty, min below cap, a confirmed coin covers the bid",
                &collecting_html(&fire, view.tip),
            ),
            spec(
                "Holding on fee: the next-block min is at or above the cap",
                &collecting_html(&fee_hold, view.tip),
            ),
            spec(
                "A tree in the mempool does not hold collecting",
                &collecting_html(&base, view.tip),
            ),
            spec(
                "Holding on coins: no confirmed coin covers the bid",
                &collecting_html(&coin, view.tip),
            ),
            spec(
                "Node not reachable: no live fee reading, so no forecast",
                &collecting_html(&unread, view.tip),
            ),
            spec("Empty pool", &collecting_html(&empty, view.tip)),
        ],
    ));
    if let Some(tree) = mempool_tree {
        let mut just = tree.clone();
        just.broadcast_height = view.tip;
        just.outcome = Outcome::InMempool { n: 0 };
        let mut last = tree.clone();
        last.broadcast_height = view.tip.saturating_sub(4);
        last.outcome = Outcome::InMempool { n: 4 };
        let mut second = last.clone();
        second.seq = second.seq.saturating_add(1);
        second.broadcast_height = view.tip.saturating_sub(2);
        second.outcome = Outcome::InMempool { n: 2 };
        specs.push_str(&state_group(
            "In mempool",
            &[
                spec("Just sent: +0 of 5", &mempool_html(&[just], view.explorer)),
                spec(
                    "Last block of the window: +4 of 5, and +5 is still on time",
                    &mempool_html(&[last.clone()], view.explorer),
                ),
                spec(
                    "Two trees in the mempool, each with its own +N of 5",
                    &mempool_html(&[last, second], view.explorer),
                ),
                spec("Nothing in mempool", &mempool_html(&[], view.explorer)),
            ],
        ));
    }
    if let Some(tree) = on_time {
        let n = match tree.outcome {
            Outcome::OnTime { n } => n,
            _ => 0,
        };
        let mut older = view
            .anchored
            .as_ref()
            .map(|tile| tile.tree.clone())
            .unwrap_or_else(|| tree.clone());
        older.broadcast_height = 970_311;
        older.inclusion_height = Some(970_313);
        older.outcome = Outcome::OnTime { n: 2 };
        specs.push_str(&state_group(
            "Last anchored",
            &[
                spec(
                    "On time",
                    &anchored_html(
                        Some(&AnchoredTile {
                            tree: tree.clone(),
                            in_window: true,
                            ago: view
                                .tip
                                .saturating_sub(tree.inclusion_height.unwrap_or(view.tip)),
                        }),
                        view.explorer,
                    ),
                ),
                spec(
                    "Older than the ledger window",
                    &anchored_html(
                        Some(&AnchoredTile {
                            tree: older,
                            in_window: false,
                            ago: view.tip - 970_313,
                        }),
                        view.explorer,
                    ),
                ),
                spec(
                    "No anchor yet (fresh data directory)",
                    &anchored_html(None, view.explorer),
                ),
            ],
        ));
        let _ = n;
    }
    let mut thin = view.wallet.clone();
    thin.confirmed = 1_204;
    thin.coins = 1;
    thin.unconfirmed = 0;
    thin.unconfirmed_coins = 0;
    thin.thin = true;
    thin.runway = 1_204 / TREE_COST_AT_CAP;
    thin.days = view
        .wallet
        .per_day
        .gt(&0.0)
        .then(|| thin.runway as f64 / view.wallet.per_day);
    let unread_wallet = wallet_unknown("node RPC timed out after 2 s");
    specs.push_str(&state_group(
        "Wallet",
        &[
            spec("Normal", &wallet_html(&view.wallet)),
            spec(
                "Thin: one confirmed coin; the plant still spends it",
                &wallet_html(&thin),
            ),
            spec("Not read: never shown as 0", &wallet_html(&unread_wallet)),
        ],
    ));
    let mut syncing = view.system.clone();
    syncing.known = true;
    syncing.node_blocks = 970_650;
    syncing.node_headers = view.tip;
    syncing.syncing = true;
    syncing.tip_lag = 970_650_i64 - i64::from(view.tip);
    let mut behind = view.system.clone();
    behind.known = true;
    behind.node_blocks = view.tip + 3;
    behind.node_headers = view.tip + 3;
    behind.syncing = false;
    behind.tip_lag = 3;
    behind.tip_seen_min = Some(38);
    let mut down = view.system.clone();
    down.known = false;
    specs.push_str(&state_group(
        "System",
        &[
            spec("Synced", &system_html(&view.system)),
            spec(
                "Node still syncing: inclusion lookups wait for it",
                &system_html(&syncing),
            ),
            spec("Bari behind its node", &system_html(&behind)),
            spec("Node not reachable", &system_html(&down)),
        ],
    ));
    let empty_rows = vec![
        LedgerRow::Collecting {
            next: base.next,
            count: 0,
            forecast_text: "Empty".to_owned(),
        },
        LedgerRow::Gap {
            lo: view.window_start,
            hi: view.tip,
            n: LEDGER_BLOCKS,
            parts: vec!["270 quiet".into(), "18 not seen".into()],
            title: "pool empty".into(),
        },
    ];
    let fresh_rows = vec![
        LedgerRow::Collecting {
            next: base.next,
            count: 2,
            forecast_text: "Holding".to_owned(),
        },
        LedgerRow::Gap {
            lo: view.tip - 2,
            hi: view.tip,
            n: 3,
            parts: vec!["2 quiet".into(), "1 held (min ≥ cap)".into()],
            title: "pool 2".into(),
        },
        LedgerRow::Gap {
            lo: view.window_start,
            hi: view.tip - 3,
            n: LEDGER_BLOCKS - 3,
            parts: vec!["not seen".into()],
            title: "no block records".into(),
        },
    ];
    let empty_chart = "<section class=\"panel fees\"><header><h2>Fees · 48 h</h2><span class=\"sub\">sat/vB against block height</span></header><p class=\"empty-chart\">No fee readings in this window. The test plane has no fee gate; on the live plane this means the node was not read at any block.</p></section>";
    specs.push_str(&state_group(
        "Ledger and chart",
        &[
            spec(
                "No trees in 48 h: one gap row covers the window",
                &ledger_html(view, &empty_rows, "no trees sent in this window"),
            ),
            spec(
                "Fresh data directory: heights before the first record are not seen",
                &ledger_html(view, &fresh_rows, "records start at 970698"),
            ),
            spec("No fee readings", empty_chart),
        ],
    ));
    let body = format!(
        "{}<p class=\"intro\">Empty, error and stale states for each part of the page. Each specimen is the same component the main mockup uses, fed the state named in its caption. Unknown values are shown as – with the reason, never as 0.</p>{specs}",
        header_html(view.tip, &view.instance, view.explorer)
    );
    page("BARI operator · states", &body, STATES_CSS)
}

fn spec(caption: &str, inner: &str) -> String {
    format!("<figure class=\"spec\"><figcaption>{caption}</figcaption>{inner}</figure>")
}

fn state_group(title: &str, specs: &[String]) -> String {
    format!(
        "<section class=\"group\"><h3>{title}</h3><div class=\"specs\">{}</div></section>",
        specs.concat()
    )
}

const STATES_CSS: &str = r#"
.group { padding: 4px 24px 18px; }
.group h3 { margin: 6px 0 10px; font-size: 13px; font-weight: 600; color: var(--muted); }
.specs { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 14px; align-items: start; }
.spec { margin: 0; }
.spec figcaption { color: var(--faint); font-size: 12px; margin: 0 0 6px; min-height: 2.9em; }
.spec .ledger { min-height: 0; display: block; }
.spec .ledger .scroll { position: static; }
.empty-chart { margin: 0; padding: 30px 14px 34px; color: var(--muted); text-align: center; }
.intro { padding: 12px 24px 0; color: var(--muted); max-width: 980px; }
@media (max-width: 1100px) { .specs { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
@media (max-width: 700px) { .group { padding: 4px 16px 14px; } .intro { padding: 10px 16px 0; } .specs { grid-template-columns: minmax(0, 1fr); } .spec figcaption { min-height: 0; } }
"#;
const CSS: &str = r#"

:root {
  color-scheme: dark;
  --bg: #0b0e13; --panel: #11161f; --panel-2: #151b26; --line: #222a38; --line-2: #2d3748;
  --text: #e4e9f0; --muted: #8b97a9; --faint: #5e6a7c;
  --collect: #e2b15a; --mempool: #6ea8fe; --ontime: #5cc98a; --late: #b48ef0; --missed: #ef7d7d;
  --held: #e2b15a; --quiet: #263040;
}
* { box-sizing: border-box; }
html, body { margin: 0; background: var(--bg); color: var(--text); }
body { font: 13px/1.45 ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif; font-variant-numeric: tabular-nums; }
a { color: inherit; text-decoration: none; border-bottom: 1px solid rgba(110, 168, 254, 0.5); }
a:hover { color: var(--mempool); }
code { font: 12px ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; color: var(--text); overflow-wrap: anywhere; }
h2 { margin: 0; font-size: 11px; font-weight: 600; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); }
.dim { color: var(--faint); }
.c-ontime { color: var(--ontime); } .c-late { color: var(--late); } .c-missed { color: var(--missed); }
.c-mempool { color: var(--mempool); } .c-held { color: var(--held); }

.head { padding: 16px 24px 0; }
.head p { margin: 0; font-size: 14px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.head b { font-weight: 650; letter-spacing: 0.03em; }
.head .long { color: var(--muted); }
.head .sep { color: var(--faint); padding: 0 0.5em; }

main { padding: 14px 24px 8px; display: grid; gap: 14px;
  grid-template-columns: minmax(0, 5fr) minmax(0, 7fr);
  grid-template-areas: "banner banner" "pipe pipe" "ledger fees" "ledger tiles"; }
.banners { grid-area: banner; display: grid; gap: 8px; }
.banners:empty { display: none; }
.pipe { grid-area: pipe; }
.ledger { grid-area: ledger; min-width: 0; }
.fees { grid-area: fees; min-width: 0; }
.tiles { grid-area: tiles; min-width: 0; display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 14px; }

.panel { background: var(--panel); border: 1px solid var(--line); border-radius: 6px; }
.panel > header { display: flex; flex-wrap: wrap; align-items: baseline; justify-content: space-between; gap: 2px 12px; padding: 10px 14px 8px; }
.panel > header h2 { white-space: nowrap; }
.panel .sub { color: var(--faint); font-size: 12px; }

.banner { border: 1px solid var(--line-2); border-left: 3px solid var(--held); background: #17140d; border-radius: 6px; padding: 9px 14px; }
.banner.issue { border-left-color: var(--missed); background: #1a1013; }
.banner b { font-weight: 600; }

/* pipeline */
.pipe { display: grid; grid-template-columns: minmax(0, 1.3fr) 28px minmax(0, 1fr) 44px minmax(0, 1fr); align-items: stretch; }
.stage { background: var(--panel); border: 1px solid var(--line); border-top: 3px solid var(--line-2); border-radius: 6px; padding: 10px 14px 12px; min-width: 0; }
.stage > header { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; margin-bottom: 6px; }
.stage .meta { color: var(--muted); font-size: 12px; white-space: nowrap; }
.stage .meta b { color: var(--text); font-weight: 600; }
.stage .big { margin: 0 0 4px; font-size: 26px; line-height: 1.15; font-weight: 600; letter-spacing: -0.01em; }
.stage .big small { font-size: 13px; font-weight: 500; color: var(--muted); letter-spacing: 0; }
.stage .big a { border-bottom-width: 1px; }
.stage.collecting { border-top-color: var(--collect); background: linear-gradient(180deg, rgba(226, 177, 90, 0.07), rgba(226, 177, 90, 0) 60%), var(--panel); }
.stage.mempool { border-top-color: var(--mempool); }
.stage.anchored { border-top-color: var(--ontime); }
.stage.anchored.late { border-top-color: var(--late); }
.stage.empty { border-top-color: var(--line-2); }
.forecast { margin: 0 0 8px; font-weight: 600; }
.forecast.hold { color: var(--held); } .forecast.fire { color: var(--ontime); }
.forecast.empty, .forecast.unknown { color: var(--muted); font-weight: 500; }
.result { margin: 0 0 8px; font-weight: 600; }
.result.ontime { color: var(--ontime); } .result.late { color: var(--late); }
.link, .divide { display: flex; align-items: center; justify-content: center; color: var(--faint); }
.link span { font-size: 22px; }
.divide { position: relative; }
.divide::before { content: ""; position: absolute; top: 6px; bottom: 6px; left: 50%; border-left: 1px dashed var(--line-2); }
.divide span { position: relative; writing-mode: vertical-rl; transform: rotate(180deg); background: var(--bg); padding: 6px 0; font-size: 10px; letter-spacing: 0.08em; text-transform: uppercase; }
.wbar { display: grid; grid-template-columns: repeat(5, 1fr); gap: 3px; margin: 2px 0 10px; }
.wbar i { height: 6px; border-radius: 2px; background: var(--quiet); }
.wbar i.on { background: var(--mempool); }
.wbar i.next { background: transparent; box-shadow: inset 0 0 0 1px var(--mempool); }

.kv { margin: 0; display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 3px 14px; }
.kv > div { display: contents; }
.kv dt { color: var(--muted); white-space: nowrap; }
.kv dd { margin: 0; min-width: 0; }
.note { margin: 8px 0 0; color: var(--faint); font-size: 12px; }
.note.warn { color: var(--held); }

/* ledger */
.ledger { position: relative; min-height: 360px; display: flex; flex-direction: column; container: ledger / inline-size; }
.ledger .scroll { position: absolute; top: 38px; left: 0; right: 0; bottom: 0; overflow: auto; }
table { width: 100%; border-collapse: collapse; table-layout: fixed; }
col.c-sent { width: 22%; } col.c-wp { width: 21%; } col.c-inc { width: 21%; } col.c-res { width: 36%; }
th, td { text-align: left; padding: 6px 10px; border-bottom: 1px solid #1a212d; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
th:first-child, td:first-child { padding-left: 14px; }
th { position: sticky; top: 0; z-index: 1; background: var(--panel); color: var(--muted); font-size: 11px; font-weight: 600; letter-spacing: 0.06em; text-transform: uppercase; border-bottom-color: var(--line-2); }
.r { text-align: right; padding-right: 22px; }
.narrow { display: none; }
.r-tree td:first-child { box-shadow: inset 2px 0 0 transparent; }
.r-tree.pinned td { background: var(--panel-2); }
.r-tree.pinned td:first-child { box-shadow: inset 2px 0 0 var(--muted); }
.r-collecting td { background: rgba(226, 177, 90, 0.07); }
.r-collecting td:first-child { box-shadow: inset 2px 0 0 var(--collect); }
.tag { margin-left: 6px; font-size: 10px; letter-spacing: 0.06em; text-transform: uppercase; color: var(--collect); }
.res { font-weight: 600; }
.res::before { content: ""; display: inline-block; width: 7px; height: 7px; border-radius: 50%; margin-right: 7px; vertical-align: 1px; background: currentColor; }
.res.collecting { color: var(--collect); } .res.mempool { color: var(--mempool); } .res.ontime { color: var(--ontime); }
.res.late { color: var(--late); } .res.missed { color: var(--missed); }
.r-gap td { color: var(--faint); font-size: 12px; padding-top: 4px; padding-bottom: 4px; white-space: normal; }
.r-gap .rng { color: var(--muted); }

/* fees */
.fees { padding-bottom: 10px; }
.chart { display: block; width: 100%; height: auto; padding: 0 6px; }
.chart text { font-family: inherit; }
.chart.compact { display: none; }
.legend { list-style: none; margin: 6px 14px 0; padding: 0; display: flex; flex-wrap: wrap; gap: 4px 16px; color: var(--muted); font-size: 12px; }
.legend li { display: inline-flex; align-items: center; gap: 6px; white-space: nowrap; }
.sw { display: inline-block; width: 14px; height: 8px; }
.sw.l-min { height: 0; border-top: 2px solid #c3cfdd; }
.sw.l-cap { height: 0; border-top: 2px dashed var(--held); }
.sw.d-ontime, .sw.d-late, .sw.d-missed, .sw.d-mempool { width: 8px; border-radius: 50%; }
.sw.d-ontime { background: var(--ontime); } .sw.d-late { background: var(--late); }
.sw.d-missed { background: var(--missed); } .sw.d-mempool { background: var(--mempool); }
.sw.s-sent { background: #cfd8e3; } .sw.s-held { background: var(--held); }
.sw.s-coin { background: #e2b15a; } .sw.s-quiet { background: var(--quiet); }
.inflight + .inflight { margin-top: 10px; padding-top: 8px; border-top: 1px solid var(--line); }
.sw.s-unseen { background: repeating-linear-gradient(45deg, #3a4558 0 1.5px, transparent 1.5px 4px); }
.sw.s-next { box-shadow: inset 0 0 0 1px var(--collect); }
.fees .note { margin: 8px 14px 0; }

.tile { padding: 10px 14px 12px; }
.tile h2 { margin-bottom: 8px; }

.foot { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 6px 24px; padding: 4px 24px 24px; color: var(--faint); font-size: 12px; }
.foot details summary { cursor: pointer; }
.foot ul { margin: 6px 0 0; padding-left: 18px; }

/* tablet: one column, tiles still side by side */
@media (max-width: 1100px) {
  main { grid-template-columns: minmax(0, 1fr); grid-template-areas: "banner" "pipe" "ledger" "fees" "tiles"; }
  .ledger { min-height: 0; display: block; }
  .ledger .scroll { position: static; max-height: 560px; }
}

/* phone: everything stacks */
@media (max-width: 700px) {
  body { font-size: 13.5px; }
  .head { padding: 12px 16px 0; }
  .head p { font-size: 13px; }
  .head .long, .head .word { display: none; }
  .head .sep { padding: 0 0.35em; }
  main { padding: 12px 16px 8px; gap: 12px; }
  .pipe { grid-template-columns: minmax(0, 1fr); gap: 10px; }
  .link { display: none; }
  .divide { height: 18px; }
  .divide::before { top: 50%; bottom: auto; left: 8px; right: 8px; border-left: 0; border-top: 1px dashed var(--line-2); }
  .divide span { writing-mode: horizontal-tb; transform: none; padding: 0 8px; }
  .stage .big { font-size: 24px; }
  .tiles { grid-template-columns: minmax(0, 1fr); gap: 12px; }
  .ledger .scroll { max-height: none; overflow: visible; }
  .chart.wide { display: none; } .chart.compact { display: block; padding: 0 2px; }
  .legend { margin: 6px 12px 0; gap: 4px 12px; }
  .fees .note { margin: 8px 12px 0; }
  .foot { padding: 4px 16px 20px; }
}

/* the ledger adapts to its own width, so 390 px needs no sideways scroll */
@container ledger (max-width: 480px) {
  th, td { padding: 6px 5px; font-size: 13px; }
  th { font-size: 10.5px; letter-spacing: 0.03em; }
  th:first-child, td:first-child { padding-left: 12px; }
  .r { padding-right: 9px; }
  .wide, .tag { display: none; } .narrow { display: inline; }
  col.c-sent { width: 23%; } col.c-wp { width: 13%; } col.c-inc { width: 22%; } col.c-res { width: 42%; }
  .res::before { width: 6px; height: 6px; margin-right: 5px; }
  .r-gap td { font-size: 12px; }
}

"#;
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn fixture() -> OpsView {
        fixture_view("example-host", "0123456")
    }

    fn reads_fee(min: Option<f64>, floor: f64) -> OpsReads {
        OpsReads {
            fee: match min {
                Some(min) => NodeRead::Ready(FeeReady {
                    min_sat_vb: Some(min),
                    floor_sat_vb: Some(floor),
                    bid_sat_vb: Some(fee::would_bid(min, CAP, DEFAULT_FEE_MULTIPLIER)),
                    source: Some(if floor > min { "floor" } else { "mempool" }.to_owned()),
                }),
                None => NodeRead::Failed {
                    reason: "node not reachable".to_owned(),
                },
            },
            wallet: NodeRead::Ready(WalletReady {
                confirmed_sat: 10_000,
                unconfirmed_sat: 0,
                confirmed_coins: 3,
                unconfirmed_coins: 0,
                coins: vec![WalletCoin {
                    amount_sat: 10_000,
                    confirmations: 6,
                    spendable: true,
                    safe: true,
                }],
                min_conf: 1,
                reserve_min: 2,
                reserve_max: 3,
                consolidate: false,
            }),
            sync: NodeRead::Ready(SyncReady {
                blocks: 100,
                headers: 100,
                initialblockdownload: false,
            }),
        }
    }

    fn lone_tree(broadcast: u32, inclusion: Option<u32>, tip: u32) -> OpsInputs {
        let start = tip.saturating_sub(LEDGER_BLOCKS - 1);
        let mut blocks = Vec::new();
        for height in start..=tip {
            blocks.push(BlockInput {
                height,
                time: Some(1),
                observed_unix_ms: 1,
                decision: if height == broadcast {
                    DecisionKind::Sent
                } else {
                    DecisionKind::Quiet
                },
                next_block_min_sat_vb: Some(0.40),
                bid_sat_vb: (height == broadcast).then_some(0.60),
                pool_size: u32::from(height == broadcast),
                added: Some(u32::from(height == broadcast)),
                malformed: 0,
                already_held: 0,
                tree: (height == broadcast).then_some(1),
            });
        }
        OpsInputs {
            tip_height: tip,
            tip_time: Some(1),
            observed_unix_ms: Some(1),
            trees: vec![TreeInput {
                batch_sequence: 1,
                broadcast_height: broadcast,
                tree_size: 1,
                txid: [7; 32],
                inclusion_height: inclusion,
                next_block_min_sat_vb: Some(0.40),
                bid_sat_vb: 0.60,
                fee_sat: Some(96),
                vsize: Some(159),
                fee_source: "mempool".to_owned(),
                post_heights: vec![broadcast.saturating_sub(1)],
                leaves: vec![[1; 32]],
            }],
            blocks,
            pool_len: 0,
            oldest_post_height: None,
            added_since_tip: 0,
            malformed_since_tip: 0,
            already_held_since_tip: 0,
            store_issues: Vec::new(),
            process: ProcessFacts {
                version: VERSION.to_owned(),
                commit: "abc".to_owned(),
                instance: "example-host".to_owned(),
                started_unix_ms: 1,
            },
            network: "mainnet".to_owned(),
        }
    }

    #[test]
    fn outcome_boundaries_match_the_clock() {
        let b = 100u32;
        assert_eq!(
            outcome_text(&tree_outcome(None, b, b + 4)),
            "In mempool +4 of 5"
        );
        assert_eq!(outcome_text(&tree_outcome(None, b, b + 5)), "Missed");
        assert_eq!(
            outcome_text(&tree_outcome(Some(b + 5), b, b + 5)),
            "On time +5"
        );
        assert_eq!(
            outcome_text(&tree_outcome(Some(b + 6), b, b + 6)),
            "Late +6"
        );
        assert_eq!(
            outcome_text(&tree_outcome(Some(b + 8), b, b + 9)),
            "Late +8"
        );
    }

    #[test]
    fn fixture_reconciles_and_matches_the_mockup_seed() {
        let view = fixture();
        assert!(
            view.checks.iter().all(|check| check.holds),
            "{:?}",
            view.checks
                .iter()
                .filter(|check| !check.holds)
                .map(|check| format!("{}{}", check.name, check.detail))
                .collect::<Vec<_>>()
        );
        // Eligible blocks send. The old one-tree hold no longer suppresses them,
        // so this seed has more trees than the mockup that held for one in flight.
        assert!(view.summary.trees > 9, "{}", view.summary.trees);
        assert!(view.summary.mempool > 1, "{}", view.summary.mempool);
        assert_eq!(
            view.summary.on_time + view.summary.late + view.summary.missed + view.summary.mempool,
            view.summary.trees
        );
        assert_eq!(
            view.summary.sent + view.summary.held + view.summary.quiet + view.summary.not_seen,
            view.summary.blocks
        );
        assert!(view.summary.held > 0, "a fee above the cap still holds");
        assert_eq!(view.collecting.count, view.pool_len);
        assert!(
            view.collecting.forecast_text.starts_with("Will fire at"),
            "a tree in the mempool does not hold collecting: {}",
            view.collecting.forecast_text
        );
        assert!(
            !view.collecting.forecast_text.contains("in mempool"),
            "{}",
            view.collecting.forecast_text
        );
        assert!(
            view.mempool.len() > 1,
            "several trees can be in the mempool"
        );
        assert!(view
            .mempool
            .windows(2)
            .all(|pair| { pair[0].broadcast_height <= pair[1].broadcast_height }));
        let late = view
            .trees
            .iter()
            .find(|tree| tree.broadcast_height == 970_682)
            .unwrap();
        assert_eq!(outcome_text(&late.outcome), "Late +8");
        assert_eq!(late.inclusion_height, Some(970_690));
        let html = render(&view);
        assert!(html.contains(&format!(
            "Data checks: {} of {} hold",
            view.checks.len(),
            view.checks.len()
        )));
        assert!(view.checks.iter().any(|check| check.id == "R12"));
        assert!(html.contains("groundinglab"));
        assert!(html.contains("the Bitcoin Anchored Registry Index"));
        assert!(html.contains("operator v0.6.2"));
        assert!(html.contains("example-host"));
        assert!(html.contains("https://mempool.space/block/970700"));
        assert!(html.contains(">Sent at<"));
        assert!(html.contains(">Waypoints<"));
        assert!(html.contains(">Included<"));
        assert!(html.contains(">Result<"));
        assert!(!html.contains(">Fee<"));
        assert!(!html.contains(">Tx<"));
        assert!(html.contains("Late +8"));
        assert!(html.contains("Missed"));
        assert!(html.contains("On time +"));
        assert!(html.contains("In mempool +2 of 5"));
        assert!(html.contains("12,863"));
        assert!(html.contains("peak "));
        assert!(html.contains("cap 1.1"));
        assert!(html.contains("since 970527"));
        assert!(html.contains("1d 5h"));
        for word in [
            "sealed", "planted", "deferred", "archived", "failed", "rejected", "admit", "deny",
        ] {
            assert!(!html.to_ascii_lowercase().contains(word), "{word}");
        }
        assert!(!html.contains("Anchored +"));
        assert!(!html.contains("bc1"));
        let states = render_states(&view);
        assert!(states.contains("Holding · no fee reading"));
        assert!(states.contains("Empty · nothing to send"));
        assert!(states.contains("No tree waiting for a block"));
        assert!(states.contains("None yet"));
        assert!(states.contains("Older than the 48 h ledger."));
        assert!(states.contains("Wallet not read"));
        assert!(states.contains("not reachable"));
        assert!(states.contains("syncing"));
        assert!(states.contains("No new tip for 74 min."));
        assert!(states.contains("This view is 12 min old."));
        assert!(states.contains("not seen"));
    }

    fn check<'a>(view: &'a OpsView, id: &str) -> &'a Check {
        view.checks.iter().find(|item| item.id == id).expect(id)
    }

    /// Sent heights as the ledger draws them: a tree row, or a gap height whose
    /// block decision is sent. This is the view, not a second pass over inputs.
    fn ledger_sent_count(view: &OpsView) -> u32 {
        let mut sent = 0u32;
        let mut heights = BTreeSet::new();
        for row in &view.ledger {
            match row {
                LedgerRow::Tree { tree, .. }
                    if (view.window_start..=view.tip).contains(&tree.broadcast_height)
                        && heights.insert(tree.broadcast_height) =>
                {
                    sent += 1;
                }
                LedgerRow::Gap { lo, hi, .. } => {
                    for height in *lo..=*hi {
                        if view
                            .blocks
                            .get(&height)
                            .is_some_and(|block| matches!(block.decision, DecisionKind::Sent))
                        {
                            sent += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        sent
    }

    #[test]
    fn system_tile_shows_commit_and_uptime_from_process_start() {
        let (mut inputs, reads, now) = fixture_parts("example-host", "abcdef0");
        let started = now - 7 * 60 * 1000;
        inputs.process.commit = "abcdef0".to_owned();
        inputs.process.started_unix_ms = started;
        for block in &mut inputs.blocks {
            block.observed_unix_ms = started - 1;
        }
        let view = build_ops_view(&inputs, &reads, now);
        assert_eq!(view.system.commit, "abcdef0");
        assert_eq!(view.system.since, None);
        assert_eq!(view.system.uptime, "7 min");
        let html = render(&view);
        assert!(html.contains("<code>abcdef0</code>"));
        assert!(html.contains("7 min"));
        assert!(!html.contains("since –"));
        assert!(!html.contains("<code>–</code>"));
    }

    #[test]
    fn blocks_sent_count_equals_ledger_sent_rows() {
        let mut inputs = lone_tree(970_664, Some(970_666), 970_739);
        inputs.blocks.retain(|block| block.height != 970_664);
        let view = build_ops_view(&inputs, &reads_fee(Some(0.40), 0.1), 2_000);
        assert!(
            !view.blocks.contains_key(&970_664),
            "970664 has no block record"
        );
        assert!(view.ledger.iter().any(|row| matches!(
            row,
            LedgerRow::Tree { tree, .. } if tree.broadcast_height == 970_664
        )));
        assert_eq!(view.summary.sent, ledger_sent_count(&view));
        assert_eq!(view.summary.sent, 1);
        let recounted = view
            .blocks
            .values()
            .filter(|block| {
                (view.window_start..=view.tip).contains(&block.height)
                    && matches!(block.decision, DecisionKind::Sent)
            })
            .count();
        assert_eq!(recounted, 0);
        assert_eq!(
            view.summary.sent + view.summary.held + view.summary.quiet + view.summary.not_seen,
            view.summary.blocks
        );
        assert!(render(&view).contains("· 1 sent"));

        let fixture = fixture();
        assert_eq!(fixture.summary.sent, ledger_sent_count(&fixture));
        assert!(
            fixture.summary.sent > 9,
            "an eligible block sends even while another tree is in the mempool"
        );
    }

    #[test]
    fn legacy_rows_skip_r10_and_named_trees_still_check() {
        let fixture = fixture();
        for id in ["R6", "R10", "R11"] {
            let item = check(&fixture, id);
            assert!(item.holds, "{}{}", item.name, item.detail);
        }

        let mut inputs = lone_tree(90, Some(91), 100);
        let sent = inputs
            .blocks
            .iter_mut()
            .find(|block| block.height == 90)
            .unwrap();
        sent.tree = None;
        sent.next_block_min_sat_vb = None;
        sent.bid_sat_vb = None;
        inputs.trees[0].next_block_min_sat_vb = None;
        inputs.trees[0].fee_sat = None;
        inputs.trees.push(TreeInput {
            batch_sequence: 701,
            broadcast_height: 80,
            tree_size: 4,
            txid: [9; 32],
            inclusion_height: Some(88),
            next_block_min_sat_vb: None,
            bid_sat_vb: 0.50,
            fee_sat: None,
            vsize: None,
            fee_source: "mempool".to_owned(),
            post_heights: Vec::new(),
            leaves: Vec::new(),
        });
        inputs.blocks.retain(|block| block.height != 80);
        let view = build_ops_view(&inputs, &reads_fee(Some(0.40), 0.1), 2_000);
        let r6 = check(&view, "R6");
        let r10 = check(&view, "R10");
        assert!(r6.holds, "{}{}", r6.name, r6.detail);
        assert!(
            r10.holds,
            "legacy missing min and missing block must not fail R10: {}{}",
            r10.name, r10.detail
        );

        // Complete data still fails a wrong bid and a named tree that is not there.
        let mut inputs = lone_tree(90, Some(91), 100);
        inputs.trees[0].bid_sat_vb = 0.10;
        inputs
            .blocks
            .iter_mut()
            .find(|block| block.height == 90)
            .unwrap()
            .bid_sat_vb = Some(0.10);
        let view = build_ops_view(&inputs, &reads_fee(Some(0.40), 0.1), 2_000);
        let r10 = check(&view, "R10");
        assert!(!r10.holds, "a complete bid that is not 1.5× min must fail");
        assert!(r10.detail.contains("would-bid"), "{}", r10.detail);

        let mut inputs = lone_tree(90, Some(91), 100);
        inputs
            .blocks
            .iter_mut()
            .find(|block| block.height == 90)
            .unwrap()
            .tree = Some(99);
        let view = build_ops_view(&inputs, &reads_fee(Some(0.40), 0.1), 2_000);
        let r6 = check(&view, "R6");
        assert!(!r6.holds, "a named tree id that is absent must fail");
        assert!(r6.detail.contains("90"), "{}", r6.detail);
        assert!(r6.detail.contains("none"), "{}", r6.detail);
    }

    #[test]
    fn r11_skips_a_tip_from_before_this_process_and_names_a_live_mismatch() {
        let mut view = fixture();
        let observed = view.blocks.get(&view.tip).unwrap().observed_unix_ms;
        assert!(observed > 0);
        view.started_unix_ms = observed + 1;
        view.pool_len = 10_000;
        let skipped = evaluate_checks(&view);
        let r11 = skipped.iter().find(|item| item.id == "R11").unwrap();
        assert!(
            r11.holds,
            "a pool carried across restart is not an eviction: {}",
            r11.detail
        );

        view.started_unix_ms = observed;
        view.pool_len = 1;
        let checked = evaluate_checks(&view);
        let r11 = checked.iter().find(|item| item.id == "R11").unwrap();
        assert!(!r11.holds);
        assert!(r11.detail.contains("pool has 1"), "{}", r11.detail);
        assert!(r11.detail.contains("expected"), "{}", r11.detail);
        assert!(r11.detail.contains("carried"), "{}", r11.detail);
    }

    #[test]
    fn each_forecast_line_is_reachable_in_gate_order() {
        let mut inputs = lone_tree(90, Some(91), 100);
        inputs.pool_len = 0;
        let view = build_ops_view(&inputs, &reads_fee(Some(2.2), 0.1), 1_000);
        assert_eq!(view.collecting.forecast_text, "Empty · nothing to send");

        inputs.pool_len = 4;
        inputs.oldest_post_height = Some(98);
        inputs.trees[0].inclusion_height = None;
        inputs.trees[0].broadcast_height = 98;
        // A tree in the mempool is listed on its own tile. It does not hold collecting.
        let view = build_ops_view(&inputs, &reads_fee(Some(0.42), 0.1), 1_000);
        assert!(
            view.collecting.forecast_text.starts_with("Will fire at"),
            "{}",
            view.collecting.forecast_text
        );
        assert_eq!(view.mempool.len(), 1);
        assert!(render(&view).contains("In mempool +2 of 5"));
        assert!(!render(&view).contains("Holding · tree"));

        inputs.trees[0].inclusion_height = Some(99);
        inputs.trees[0].broadcast_height = 90;
        let view = build_ops_view(&inputs, &reads_fee(None, 0.1), 1_000);
        assert_eq!(view.collecting.forecast_text, "Holding · no fee reading");
        assert!(render(&view).contains("not read · node not reachable"));

        let view = build_ops_view(&inputs, &reads_fee(Some(2.2), 0.1), 1_000);
        assert_eq!(view.collecting.forecast_text, "Holding · min 2.2 ≥ 1.1 cap");

        let mut high_floor = reads_fee(Some(0.40), 1.2);
        if let NodeRead::Ready(fee) = &mut high_floor.fee {
            fee.floor_sat_vb = Some(1.2);
            fee.bid_sat_vb = Some(0.60);
        }
        let view = build_ops_view(&inputs, &high_floor, 1_000);
        assert_eq!(
            view.collecting.forecast_text,
            "Holding · floor 1.2 > 1.1 cap"
        );

        let mut poor = reads_fee(Some(0.42), 0.1);
        if let NodeRead::Ready(wallet) = &mut poor.wallet {
            wallet.coins[0].amount_sat = 10;
            wallet.confirmed_sat = 10;
        }
        let view = build_ops_view(&inputs, &poor, 1_000);
        assert!(
            view.collecting
                .forecast_text
                .starts_with("Holding · no confirmed coin covers "),
            "{}",
            view.collecting.forecast_text
        );

        let view = build_ops_view(&inputs, &reads_fee(Some(0.42), 0.1), 1_000);
        assert!(
            view.collecting
                .forecast_text
                .starts_with("Will fire at 101 · bid "),
            "{}",
            view.collecting.forecast_text
        );
    }

    fn tile_at(min_c: Option<u32>) -> CollectingTile {
        CollectingTile {
            next: 101,
            count: 1,
            oldest: Some(98),
            oldest_blocks: Some(2),
            forecast: "fire",
            forecast_text: "Will fire".to_owned(),
            min_c,
            min_unknown: min_c.is_none(),
        }
    }

    #[test]
    fn collecting_min_line_holds_when_min_meets_cap_and_bids_below_it() {
        let held = collecting_html(&tile_at(Some(207)), 100);
        assert!(
            held.contains("<dt>Min now</dt><dd>2.07 sat/vB · above cap 1.1 · holding</dd>"),
            "{held}"
        );
        assert!(!held.contains("would bid"), "{held}");

        let equal = collecting_html(&tile_at(Some(110)), 100);
        assert!(
            equal.contains("<dt>Min now</dt><dd>1.1 sat/vB · above cap 1.1 · holding</dd>"),
            "{equal}"
        );
        assert!(!equal.contains("would bid"), "{equal}");

        let bid = collecting_html(&tile_at(Some(42)), 100);
        assert!(
            bid.contains("<dt>Min now</dt><dd>0.42 sat/vB · would bid 0.63 · cap 1.1</dd>"),
            "{bid}"
        );
        assert!(!bid.contains("holding"), "{bid}");
        assert!(
            bid.contains("<span data-count=\"pool\">1</span> <small>waypoint</small>"),
            "{bid}"
        );
        assert!(!bid.contains("1 waypoints"), "{bid}");

        let unread = collecting_html(&tile_at(None), 100);
        assert!(unread.contains("not read · node not reachable"), "{unread}");
        assert!(!unread.contains("would bid"), "{unread}");
        assert!(!unread.contains("above cap"), "{unread}");

        let inputs = lone_tree(90, Some(91), 100);
        let page = render(&build_ops_view(&inputs, &reads_fee(Some(2.07), 0.1), 1_000));
        assert!(
            page.contains("2.07 sat/vB · above cap 1.1 · holding"),
            "live min at or above the cap"
        );
        let page = render(&build_ops_view(&inputs, &reads_fee(Some(0.42), 0.1), 1_000));
        assert!(
            page.contains("0.42 sat/vB · would bid 0.63 · cap 1.1"),
            "live min under the cap"
        );
    }

    #[test]
    fn fee_chart_tooltip_uses_singular_waypoint() {
        let inputs = lone_tree(90, Some(91), 100);
        let html = render(&build_ops_view(&inputs, &reads_fee(Some(0.42), 0.1), 1_000));
        assert!(
            html.contains("Sent at 90 · 1 waypoint · bid 0.6 · included 91 · On time +1"),
            "{html}"
        );
        assert!(!html.contains("1 waypoints"), "{html}");

        let mut many = inputs;
        many.trees[0].tree_size = 2;
        let html = render(&build_ops_view(&many, &reads_fee(Some(0.42), 0.1), 1_000));
        assert!(
            html.contains("Sent at 90 · 2 waypoints · bid 0.6 · included 91 · On time +1"),
            "{html}"
        );
    }

    fn tree_at(
        seq: u64,
        sent: u32,
        size: u32,
        included: Option<u32>,
        posts: &[u32],
        leaf: u8,
    ) -> TreeInput {
        TreeInput {
            batch_sequence: seq,
            broadcast_height: sent,
            tree_size: size,
            txid: [leaf; 32],
            inclusion_height: included,
            next_block_min_sat_vb: Some(0.41),
            bid_sat_vb: 0.62,
            fee_sat: Some(98),
            vsize: Some(159),
            fee_source: "mempool".to_owned(),
            post_heights: posts.to_vec(),
            leaves: (0..size)
                .map(|index| {
                    let mut id = [leaf; 32];
                    id[31] = index as u8;
                    id
                })
                .collect(),
        }
    }

    fn block_at(
        height: u32,
        decision: DecisionKind,
        pool: u32,
        added: u32,
        tree: Option<u64>,
    ) -> BlockInput {
        BlockInput {
            height,
            time: Some(1),
            observed_unix_ms: 1,
            decision,
            next_block_min_sat_vb: Some(0.41),
            bid_sat_vb: matches!(decision, DecisionKind::Sent).then_some(0.62),
            pool_size: pool,
            added: Some(added),
            malformed: 0,
            already_held: 0,
            tree,
        }
    }

    /// The live page showed block 970741 as both "last anchored" and
    /// "in mempool". A ledger row is one tree: sent at X, included at Y.
    #[test]
    fn a_ledger_row_is_one_tree_and_several_trees_can_be_in_the_mempool() {
        let tip = 970_745u32;
        let first = tree_at(1, 970_741, 1, None, &[970_740], 0x11);
        let second = tree_at(2, 970_743, 1, None, &[970_742], 0x22);
        let late = tree_at(3, 970_730, 2, Some(970_738), &[970_729, 970_728], 0x33);
        let on_time = tree_at(4, 970_736, 1, Some(970_741), &[970_735], 0x44);
        let inputs = OpsInputs {
            tip_height: tip,
            tip_time: Some(1),
            observed_unix_ms: Some(1),
            trees: vec![first, second, late, on_time],
            blocks: vec![
                block_at(970_730, DecisionKind::Sent, 2, 2, Some(3)),
                block_at(970_736, DecisionKind::Sent, 1, 1, Some(4)),
                block_at(
                    970_739,
                    DecisionKind::Held(HoldKind::MinAboveCap),
                    1,
                    1,
                    None,
                ),
                block_at(
                    970_740,
                    DecisionKind::Held(HoldKind::NoConfirmedCoin),
                    1,
                    0,
                    None,
                ),
                block_at(970_741, DecisionKind::Sent, 1, 0, Some(1)),
                block_at(970_742, DecisionKind::Quiet, 0, 0, None),
                block_at(970_743, DecisionKind::Sent, 1, 1, Some(2)),
                block_at(970_744, DecisionKind::Quiet, 0, 0, None),
                block_at(
                    970_745,
                    DecisionKind::Held(HoldKind::MinAboveCap),
                    3,
                    3,
                    None,
                ),
            ],
            pool_len: 6,
            oldest_post_height: Some(970_744),
            added_since_tip: 3,
            malformed_since_tip: 0,
            already_held_since_tip: 0,
            store_issues: Vec::new(),
            process: ProcessFacts {
                version: VERSION.to_owned(),
                commit: "abc".to_owned(),
                instance: "example-host".to_owned(),
                started_unix_ms: 1,
            },
            network: "mainnet".to_owned(),
        };
        let view = build_ops_view(&inputs, &reads_fee(Some(0.41), 0.1), 2_000);
        assert_eq!(view.mempool.len(), 2);
        assert_eq!(view.mempool[0].broadcast_height, 970_741);
        assert_eq!(view.mempool[1].broadcast_height, 970_743);
        assert_eq!(outcome_text(&view.mempool[0].outcome), "In mempool +4 of 5");
        assert_eq!(outcome_text(&view.mempool[1].outcome), "In mempool +2 of 5");
        // Last anchored is the newest inclusion, 970741, and that row says
        // sent at 970736. It is not the tree that is still in the mempool.
        let anchored = view.anchored.as_ref().unwrap();
        assert_eq!(anchored.tree.broadcast_height, 970_736);
        assert_eq!(anchored.tree.inclusion_height, Some(970_741));
        assert_eq!(outcome_text(&anchored.tree.outcome), "On time +5");
        let late_row = view
            .trees
            .iter()
            .find(|tree| tree.broadcast_height == 970_730)
            .unwrap();
        assert_eq!(outcome_text(&late_row.outcome), "Late +8");

        let html = render(&view);
        assert!(html.contains("data-count=\"2\""));
        assert!(html.contains("In mempool +4 of 5"));
        assert!(html.contains("In mempool +2 of 5"));
        assert!(html.contains("On time +5"));
        assert!(html.contains("Late +8"));
        assert!(html.contains("held (min ≥ cap)"));
        assert!(html.contains("held (no confirmed coin)"));
        assert!(!html.contains("Holding · tree"));
        assert!(view.collecting.forecast_text.starts_with("Will fire at"));
        assert_eq!(view.collecting.min_c, Some(41));
        assert!(!html.contains("not read · node not reachable"));
        // Sent and included are different columns of the same tree row.
        assert!(html.contains(">Sent at<"));
        assert!(html.contains(">Included<"));
        for check in &view.checks {
            // This fixture is a few heights, not a 48 h window.
            if check.id == "R5" || check.id == "R8" {
                continue;
            }
            assert!(check.holds, "{}{}", check.name, check.detail);
        }

        // A waypoint posted at the send height, and a leaf in two trees, fail R12.
        let mut broken = inputs.clone();
        broken.trees[0].post_heights = vec![970_741];
        broken.trees[1].leaves = broken.trees[0].leaves.clone();
        let broken_view = build_ops_view(&broken, &reads_fee(Some(0.41), 0.1), 2_000);
        let r12 = check(&broken_view, "R12");
        assert!(!r12.holds, "{}", r12.detail);
        assert!(r12.detail.contains("post 970741"), "{}", r12.detail);
        assert!(r12.detail.contains("waypoint"), "{}", r12.detail);

        // Included at the send height is not on time, and not an anchor.
        let mut same = inputs;
        same.trees[3].inclusion_height = Some(970_736);
        let same_view = build_ops_view(&same, &reads_fee(Some(0.41), 0.1), 2_000);
        let r4 = check(&same_view, "R4");
        let r9 = check(&same_view, "R9");
        assert!(r4.holds, "included − sent of 0 is not labelled on time");
        assert!(!r9.holds, "included must be after sent");
    }

    #[test]
    fn each_block_decision_renders_in_a_gap() {
        let mut inputs = lone_tree(100, Some(101), 100);
        inputs.trees.clear();
        let kinds = [
            DecisionKind::Held(HoldKind::MinAboveCap),
            DecisionKind::Held(HoldKind::FloorAboveCap),
            DecisionKind::Held(HoldKind::TreeInMempool),
            DecisionKind::Held(HoldKind::NoFeeReading),
            DecisionKind::Held(HoldKind::NoConfirmedCoin),
            DecisionKind::Held(HoldKind::SendError),
            DecisionKind::Quiet,
        ];
        for (index, kind) in kinds.into_iter().enumerate() {
            inputs.blocks[index].decision = kind;
        }
        // one height has no record
        inputs.blocks.pop();
        let view = build_ops_view(&inputs, &reads_fee(Some(0.4), 0.1), 1_000);
        let html = render(&view);
        for text in [
            "held (min ≥ cap)",
            "held (floor > cap)",
            "held (tree in mempool, older build)",
            "held (no fee reading)",
            "held (no confirmed coin)",
            "held (send error)",
            "quiet",
            "not seen",
        ] {
            assert!(html.contains(text), "{text}");
        }
    }

    /// 970742 was logged by an older build. `added` was sampled after the
    /// seal released the accept lock, so the pool equation does not hold.
    /// That hold is history. A fee hold with the same numbers is an eviction.
    #[test]
    fn r11_treats_an_older_build_tree_in_mempool_row_as_history() {
        let tip = 970_745u32;
        let mut inputs = lone_tree(970_741, Some(970_744), tip);
        inputs.trees[0].tree_size = 13;
        inputs.trees[0].post_heights = vec![970_740];
        inputs.blocks.clear();
        inputs.tip_height = 970_743;
        inputs
            .blocks
            .push(block_at(970_741, DecisionKind::Sent, 13, 3, Some(1)));
        // Pool 3, added 4: the logged mismatch at 970742.
        inputs.blocks.push(block_at(
            970_742,
            DecisionKind::Held(HoldKind::TreeInMempool),
            3,
            4,
            None,
        ));
        inputs.blocks.push(block_at(
            970_743,
            DecisionKind::Held(HoldKind::TreeInMempool),
            6,
            3,
            None,
        ));
        // The tip row was logged before this process. The live pool equation
        // stays out of the check.
        inputs.process.started_unix_ms = 2;
        let view = build_ops_view(&inputs, &reads_fee(Some(0.41), 0.1), 2_000);
        let r11 = check(&view, "R11");
        assert!(
            r11.holds,
            "older-build tree-in-mempool is not an eviction: {}",
            r11.detail
        );
        assert!(!r11.detail.contains("970742"), "{}", r11.detail);
        let html = render(&view);
        assert!(html.contains("held (tree in mempool, older build)"));
        assert!(html.contains("970742–970743"));

        inputs.blocks[1].decision = DecisionKind::Held(HoldKind::MinAboveCap);
        let view = build_ops_view(&inputs, &reads_fee(Some(0.41), 0.1), 2_000);
        let r11 = check(&view, "R11");
        assert!(
            !r11.holds,
            "a fee hold with the same numbers is an eviction"
        );
        assert!(r11.detail.contains("970742"), "{}", r11.detail);
    }

    /// Blocks 970749–970753 were recorded quiet after the LAN outage. Bari's
    /// node did not see them live. A later empty block, observed with its
    /// header, stays quiet.
    #[test]
    fn blocks_not_observed_live_read_as_bari_offline() {
        let tip = 970_756u32;
        let observed = 1_791_629_700_000u64;
        let mut inputs = lone_tree(970_748, None, tip);
        inputs.trees.clear();
        inputs.blocks.clear();
        let mut push = |height: u32, decision: DecisionKind, header: u64, seen: u64| {
            let mut block = block_at(height, decision, 0, 0, None);
            block.time = Some(header);
            block.observed_unix_ms = seen;
            inputs.blocks.push(block);
        };
        // Live quiet, observed with the header.
        push(
            970_731,
            DecisionKind::Quiet,
            1_791_623_000,
            1_791_623_030_000,
        );
        // Outage catch-up. One observation, headers up to 50 min earlier.
        // 970753's own header is only 2 min before the observation.
        let headers = [
            (970_749, 1_791_626_051u64),
            (970_750, 1_791_626_878),
            (970_751, 1_791_629_003),
            (970_752, 1_791_629_250),
            (970_753, 1_791_629_601),
        ];
        for (height, header) in headers {
            push(height, DecisionKind::Quiet, header, observed);
        }
        // The next empty block was seen live, ten minutes later.
        push(
            970_756,
            DecisionKind::Quiet,
            1_791_630_300,
            1_791_630_320_000,
        );
        let view = build_ops_view(&inputs, &reads_fee(Some(0.28), 0.1), observed);
        let html = render(&view);
        assert!(html.contains("not seen (Bari offline)"));
        assert!(html.contains("970731 · quiet"));
        assert!(html.contains("quiet"));
        assert!(view.offline.contains(&970_749));
        assert!(view.offline.contains(&970_753));
        assert!(!view.offline.contains(&970_731));
        assert!(!view.offline.contains(&970_756));
        assert_eq!(
            view.summary.quiet, 2,
            "the live empty blocks stay quiet: {:?}",
            view.offline
        );
        let title = html
            .split("<title>")
            .find(|part| part.starts_with("970753 ·"))
            .unwrap_or("");
        assert!(
            title.starts_with("970753 · not seen (Bari offline)"),
            "{title}"
        );
    }

    /// The 10 Oct window's confirmed trees paid 760 sat. `ceil(bid × 159)` is
    /// the approximation the page marked `≈`.
    #[test]
    fn fees_paid_use_the_transaction_fee_not_bid_times_vsize() {
        let specs = [
            (970_664u32, 970_666u32, 0.60f64, 102u64),
            (970_729, 970_730, 0.51, 82),
            (970_730, 970_731, 0.42, 68),
            (970_732, 970_733, 0.44, 71),
            (970_733, 970_734, 0.39, 63),
            (970_734, 970_735, 0.42, 68),
            (970_735, 970_736, 0.39, 63),
            (970_736, 970_741, 0.30, 48),
            (970_741, 970_744, 0.41, 66),
            (970_744, 970_746, 0.42, 68),
            (970_746, 970_748, 0.38, 61),
        ];
        let tip = 970_754u32;
        let mut inputs = lone_tree(970_664, Some(970_666), tip);
        inputs.trees.clear();
        inputs.blocks.clear();
        for (index, (sent, included, bid, fee)) in specs.iter().copied().enumerate() {
            let mut tree = tree_at(
                index as u64 + 1,
                sent,
                1,
                Some(included),
                &[sent.saturating_sub(1)],
                index as u8 + 1,
            );
            tree.bid_sat_vb = bid;
            tree.fee_sat = Some(fee);
            tree.vsize = Some(159);
            inputs.trees.push(tree);
            inputs.blocks.push(block_at(
                sent,
                DecisionKind::Sent,
                1,
                1,
                Some(index as u64 + 1),
            ));
        }
        let view = build_ops_view(&inputs, &reads_fee(Some(0.40), 0.1), 2_000);
        assert_eq!(view.summary.paid, 760);
        assert!(!view.summary.paid_approx);
        let html = render(&view);
        assert!(
            html.contains("<dt>Fees paid</dt><dd>760 sat</dd>"),
            "{html}"
        );

        for tree in &mut inputs.trees {
            tree.fee_sat = None;
        }
        let view = build_ops_view(&inputs, &reads_fee(Some(0.40), 0.1), 2_000);
        let approx: u64 = specs
            .iter()
            .map(|(_, _, bid, _)| fee::broadcast_fee_sats(*bid, 159).unwrap_or(0))
            .sum();
        assert_ne!(approx, 760);
        assert_eq!(view.summary.paid, approx);
        assert!(view.summary.paid_approx);
        let html = render(&view);
        assert!(
            html.contains(&format!("<dt>Fees paid</dt><dd>≈ {approx} sat</dd>")),
            "{html}"
        );
    }

    /// 970748 was not in a block during the outage. With no inclusion the row
    /// stays Missed. An inclusion, including one inside that gap, is Late and
    /// names the included height.
    #[test]
    fn a_970748_tree_stays_missed_until_its_inclusion_is_known() {
        let mut inputs = lone_tree(970_748, None, 970_753);
        inputs.trees[0].tree_size = 6;
        inputs.trees[0].post_heights = vec![970_747];
        let view = build_ops_view(&inputs, &reads_fee(Some(0.41), 0.1), 2_000);
        assert_eq!(outcome_text(&view.trees[0].outcome), "Missed");
        assert!(render(&view).contains("Missed"));
        assert!(view.trees[0].inclusion_height.is_none());

        // 970751 is inside 970749–970753, so +3 is on time. The chain did not
        // include this tree there. A later block did.
        inputs.trees[0].inclusion_height = Some(970_751);
        inputs.tip_height = 970_753;
        let view = build_ops_view(&inputs, &reads_fee(Some(0.41), 0.1), 2_000);
        assert_eq!(outcome_text(&view.trees[0].outcome), "On time +3");
        let html = render(&view);
        assert!(html.contains("On time +3"));
        assert!(html.contains(">970751<"));

        inputs.trees[0].inclusion_height = Some(970_755);
        inputs.tip_height = 970_755;
        let view = build_ops_view(&inputs, &reads_fee(Some(0.41), 0.1), 2_000);
        assert_eq!(outcome_text(&view.trees[0].outcome), "Late +7");
        assert_eq!(view.trees[0].inclusion_height, Some(970_755));
        assert!(render(&view).contains("Late +7"));
    }

    #[test]
    fn breaking_each_rule_is_named() {
        let view = fixture();
        for index in 0..view.checks.len() {
            let mut broken = view.clone();
            match index {
                0 => broken.pool_len = 999,
                1 => broken.mempool.clear(),
                2 => {
                    if let Some(anchored) = broken.anchored.as_mut() {
                        anchored.tree.seq = 1;
                        anchored.in_window = true;
                    }
                }
                3 => {
                    // +5 is inside the window. Labelling it late breaks R4.
                    if let Some(tree) = broken.trees.iter_mut().find(|tree| {
                        matches!(tree.outcome, Outcome::OnTime { n: 5 })
                            || tree
                                .inclusion_height
                                .is_some_and(|height| height - tree.broadcast_height == 5)
                    }) {
                        tree.outcome = Outcome::Late { n: 5 };
                    } else if let Some(tree) = broken.trees.first_mut() {
                        tree.inclusion_height = Some(tree.broadcast_height.saturating_add(5));
                        tree.outcome = Outcome::Late { n: 5 };
                    }
                }
                4 => {
                    broken.ledger.pop();
                }
                5 => {
                    if let Some(block) = broken
                        .blocks
                        .values_mut()
                        .find(|block| matches!(block.decision, DecisionKind::Sent))
                    {
                        block.tree = Some(1);
                    }
                }
                6 => broken.summary.trees = 0,
                7 => broken.summary.blocks = 1,
                8 => {
                    if let Some(tree) = broken.trees.first_mut() {
                        tree.inclusion_height = Some(tree.broadcast_height);
                    }
                }
                9 => {
                    if let Some(tree) = broken.trees.first_mut() {
                        tree.bid_c = 1;
                    }
                }
                10 => broken.pool_len = 1,
                _ => {
                    if let Some(tree) = broken.trees.first_mut() {
                        tree.post_heights = vec![tree.broadcast_height];
                    }
                }
            }
            let checks = evaluate_checks(&broken);
            assert!(
                !checks[index].holds,
                "R{} still holds: {}",
                index + 1,
                checks[index].name
            );
        }
    }

    #[test]
    fn stale_banner_starts_at_sixty_minutes() {
        let (mut inputs, reads, now) = fixture_parts("example-host", "0123456");
        inputs.observed_unix_ms = Some(now - 59 * 60_000);
        let view = build_ops_view(&inputs, &reads, now);
        assert!(view.banners.is_empty(), "{:?}", view.banners);
        inputs.observed_unix_ms = Some(now - 60 * 60_000);
        let view = build_ops_view(&inputs, &reads, now);
        let html = render(&view);
        assert!(html.contains("No new tip for 60 min."));
    }

    #[test]
    fn unread_wallet_and_sync_are_dashes() {
        let (inputs, mut reads, now) = fixture_parts("example-host", "0123456");
        reads.wallet = NodeRead::Failed {
            reason: "node RPC timed out after 2 s".to_owned(),
        };
        reads.sync = NodeRead::Failed {
            reason: "down".to_owned(),
        };
        let html = render(&build_ops_view(&inputs, &reads, now));
        assert!(html.contains("Wallet not read · node RPC timed out after 2 s"));
        assert!(html.contains("not reachable"));
        assert!(!html.contains(">0 sat<"));
    }

    #[test]
    fn render_of_three_hundred_trees_stays_under_50ms() {
        let mut inputs = lone_tree(100, Some(101), 400);
        inputs.trees.clear();
        inputs.blocks.clear();
        let start = 400 - LEDGER_BLOCKS + 1;
        for height in start..=400 {
            inputs.blocks.push(BlockInput {
                height,
                time: Some(u64::from(height)),
                observed_unix_ms: 1,
                decision: DecisionKind::Quiet,
                next_block_min_sat_vb: Some(0.4),
                bid_sat_vb: None,
                pool_size: 0,
                added: Some(0),
                malformed: 0,
                already_held: 0,
                tree: None,
            });
        }
        for index in 0..300 {
            let broadcast = start + (index % 200);
            inputs.trees.push(TreeInput {
                batch_sequence: index as u64,
                broadcast_height: broadcast,
                tree_size: 2,
                txid: [index as u8; 32],
                inclusion_height: Some(broadcast + 1),
                next_block_min_sat_vb: Some(0.4),
                bid_sat_vb: 0.6,
                fee_sat: Some(96),
                vsize: Some(159),
                fee_source: "mempool".to_owned(),
                post_heights: Vec::new(),
                leaves: Vec::new(),
            });
        }
        let started = std::time::Instant::now();
        let view = build_ops_view(&inputs, &reads_fee(Some(0.4), 0.1), 2_000);
        let html = render(&view);
        assert!(
            started.elapsed().as_millis() < 50,
            "{:?}",
            started.elapsed()
        );
        assert!(html.len() > 1000);
    }

    #[test]
    fn generated_fixtures_hold_every_rule() {
        let mut state = 0x1234_5678_9abc_def0u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..8 {
            let tip = 1_000u32;
            let start = tip - LEDGER_BLOCKS + 1;
            let mut carried: u32 = 0;
            let mut blocks = Vec::new();
            let mut trees = Vec::new();
            let mut seq = 1u64;
            let mut in_flight: Option<u32> = None;
            for height in start..=tip {
                let unseen = next() % 17 == 0 && height > start + 5 && height < tip - 5;
                if unseen {
                    continue;
                }
                let added = (next() % 4) as u32;
                let cohort = carried.saturating_add(added);
                let (decision, tree_id, bid) = if cohort == 0 {
                    (DecisionKind::Quiet, None, None)
                } else if in_flight.is_some_and(|sent| height.saturating_sub(sent) < WINDOW) {
                    (DecisionKind::Held(HoldKind::TreeInMempool), None, None)
                } else if next() % 5 == 0 {
                    (DecisionKind::Held(HoldKind::MinAboveCap), None, Some(1.1))
                } else {
                    in_flight = Some(height);
                    let bid = 0.60;
                    let id = seq;
                    trees.push(TreeInput {
                        batch_sequence: id,
                        broadcast_height: height,
                        tree_size: cohort,
                        txid: [(id as u8).wrapping_add(1); 32],
                        inclusion_height: None,
                        next_block_min_sat_vb: Some(0.40),
                        bid_sat_vb: bid,
                        fee_sat: fee::broadcast_fee_sats(bid, VSIZE),
                        vsize: Some(159),
                        fee_source: "mempool".to_owned(),
                        post_heights: Vec::new(),
                        leaves: Vec::new(),
                    });
                    seq += 1;
                    (DecisionKind::Sent, Some(id), Some(bid))
                };
                if let Some(sent) = in_flight {
                    if height > sent {
                        let offset = (next() % 9) as u32;
                        if let Some(tree) =
                            trees.iter_mut().find(|tree| tree.broadcast_height == sent)
                        {
                            if offset == 0 && height >= sent + WINDOW {
                                // leave missed
                            } else if offset >= 1 && height >= sent + offset {
                                tree.inclusion_height = Some(sent + offset.max(1));
                                if tree.inclusion_height.unwrap() > sent {
                                    in_flight = None;
                                }
                            }
                        }
                    }
                }
                blocks.push(BlockInput {
                    height,
                    time: Some(u64::from(height) * 600),
                    observed_unix_ms: 1,
                    decision,
                    next_block_min_sat_vb: Some(0.40),
                    bid_sat_vb: bid,
                    pool_size: cohort,
                    added: Some(added),
                    malformed: 0,
                    already_held: 0,
                    tree: tree_id,
                });
                carried = if matches!(decision, DecisionKind::Sent) {
                    0
                } else {
                    cohort
                };
            }
            // close any still-open inclusion so the outcome is stable at the tip
            for tree in &mut trees {
                if tree.inclusion_height.is_none()
                    && tip.saturating_sub(tree.broadcast_height) < WINDOW
                {
                    // still in mempool; only one is allowed. drop extras by including them late? keep one.
                }
            }
            let inputs = OpsInputs {
                tip_height: tip,
                tip_time: Some(u64::from(tip) * 600),
                observed_unix_ms: Some(1),
                trees,
                blocks,
                pool_len: carried,
                oldest_post_height: (carried > 0).then_some(tip),
                added_since_tip: 0,
                malformed_since_tip: 0,
                already_held_since_tip: 0,
                store_issues: Vec::new(),
                process: ProcessFacts {
                    version: VERSION.to_owned(),
                    commit: "gen".to_owned(),
                    instance: "gen".to_owned(),
                    started_unix_ms: 1,
                },
                network: "mainnet".to_owned(),
            };
            // The generator's added_since_tip is 0 and the last decision may have carried pool.
            // Rebuild pool_len from the rule so the fixture is consistent, then check.
            let mut fixed = inputs;
            if let Some(block) = fixed.blocks.iter().find(|block| block.height == tip) {
                let carried = match block.decision {
                    DecisionKind::Held(_) => block.pool_size,
                    _ => 0,
                };
                fixed.pool_len = carried;
            }
            // one in-mempool tree at most: include any extra
            let mut seen_mempool = false;
            for tree in &mut fixed.trees {
                let outcome = tree_outcome(tree.inclusion_height, tree.broadcast_height, tip);
                if matches!(outcome, Outcome::InMempool { .. }) {
                    if seen_mempool {
                        tree.inclusion_height = Some(tree.broadcast_height + 1);
                    }
                    seen_mempool = true;
                }
            }
            let view = build_ops_view(&fixed, &reads_fee(Some(0.42), 0.1), 2_000);
            let failed: Vec<_> = view
                .checks
                .iter()
                .filter(|check| !check.holds)
                .map(|check| format!("{}{}", check.id, check.detail))
                .collect();
            assert!(failed.is_empty(), "{failed:?}");
        }
    }

    #[test]
    fn tip_time_formats_as_the_mockup_clock() {
        assert_eq!(format_utc(TIP_TIME), "2026-10-10 05:27:14 UTC");
        assert_eq!(format_utc(RENDERED), "2026-10-10 05:31:22 UTC");
    }
}
