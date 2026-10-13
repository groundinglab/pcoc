//! Per-broadcast fee selection for the Bitcoin Core tree.
//!
//! The bid is `max(min(1.5 × next-block minimum, 1.1 sat/vB), relay floor)`.
//! The next-block minimum is the lowest ancestor feerate that fits in one
//! block of `getrawmempool`, not `estimatesmartfee`. `estimatesmartfee` is
//! logged and shown. A failure of that call does not hold the plant. A tree
//! is broadcast only while the mempool minimum is below the 1.1 sat/vB cap.
//! At or above the cap the cohort stays collected. The hold reason is
//! recorded. It is not a stage.
//!
//! The relay floor raises a bid that would otherwise sit under it, and it
//! never raises a bid above the cap. `mempoolminfee` does not add to the bid.
//! When the higher of the two acceptance floors is above the cap, the plant
//! holds. No next-block answer is not a bid. Recent-block history is not a fee.
//!
//! `BARI_FEE_CAP_SAT_VB` defaults to 1.1. `BARI_FEE_MULTIPLIER` defaults to 1.5.
//! `BARI_FEE_BID_SAT_VB` and `BARI_DEFER_ABOVE_SAT_VB` are not read.

use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Cap when `BARI_FEE_CAP_SAT_VB` is unset. A broadcast is never above this.
pub const DEFAULT_FEE_CAP_SAT_VB: f64 = 1.1;

/// Multiplier when `BARI_FEE_MULTIPLIER` is unset.
pub const DEFAULT_FEE_MULTIPLIER: f64 = 1.5;

pub const HOLD_REASON_NEXT_BLOCK_MIN: &str = "next-block-min";
pub const HOLD_REASON_CAP: &str = "cap";
pub const HOLD_REASON_NO_CONFIRMED_UTXO: &str = "no-confirmed-utxo";
pub const HOLD_REASON_NO_ESTIMATE: &str = "no-estimate";
pub const HOLD_REASON_FALLBACK: &str = "fallback";

/// Older name for [`HOLD_REASON_CAP`]. Call sites that still say defer use this.
pub const DEFER_REASON_CAP: &str = HOLD_REASON_CAP;
/// Older name for [`HOLD_REASON_NO_CONFIRMED_UTXO`].
pub const DEFER_REASON_NO_CONFIRMED_UTXO: &str = HOLD_REASON_NO_CONFIRMED_UTXO;

/// Next-block confirmation target passed to `estimatesmartfee`.
/// This is not the 5-block anchor window.
pub const FEE_ESTIMATE_CONF_TARGET: u64 = 1;

/// Low end of the next-block target. Not `conservative`.
pub const FEE_ESTIMATE_MODE: &str = "economical";

/// Status of a height that broadcast a tree. Not a stage name from 0.6.1.
pub const DECISION_BROADCAST: &str = "broadcast";
/// Status of a height that kept collecting. The hold reason is a field.
pub const DECISION_HOLD: &str = "hold";
pub const DECISION_EMPTY: &str = "empty";

/// Older decision token. New lines use [`DECISION_BROADCAST`].
pub const DECISION_PLANT: &str = DECISION_BROADCAST;
/// Older decision token. New lines use [`DECISION_HOLD`].
pub const DECISION_DEFER: &str = DECISION_HOLD;

/// The bid came from the mempool next-block minimum (`getrawmempool`), not
/// from `estimatesmartfee`. The old label `estimate` named the wrong source.
pub const FEE_SOURCE_MEMPOOL: &str = "mempool";
/// Older name. The value is [`FEE_SOURCE_MEMPOOL`].
pub const FEE_SOURCE_ESTIMATE: &str = FEE_SOURCE_MEMPOOL;
pub const FEE_SOURCE_FLOOR: &str = "floor";
pub const FEE_SOURCE_CONFIGURED: &str = "configured";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FeePolicy {
    /// `BARI_FEE_DYNAMIC=0`. The rate must be strictly below the cap.
    Fixed {
        rate_sat_vb: f64,
        source: &'static str,
        cap_sat_vb: f64,
    },
    /// `min(multiplier × next-block minimum, cap)`. Default.
    Dynamic { cap_sat_vb: f64, multiplier: f64 },
}

/// What the node reported for this broadcast, in sat/vB.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeFeeView {
    /// Mempool next-block minimum. This is the gate. `None` is no answer.
    pub estimate_sat_vb: Option<f64>,
    /// `estimatesmartfee` 1 economical. Logged and displayed. Not the gate.
    /// `None` when the node had no answer or the call failed.
    pub node_estimate_sat_vb: Option<f64>,
    /// Not a fee. Present only so a recent-block figure cannot be spent.
    /// The live path leaves this empty and does not query block stats.
    pub recent_min_sat_vb: Option<f64>,
    /// `minrelaytxfee`. The floor the bid is raised to.
    pub relay_min_sat_vb: f64,
    /// `mempoolminfee`. Acceptance floor, not a confirmation target, and not
    /// added to the bid. Above the cap, it holds the plant.
    pub mempool_min_sat_vb: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FeeDecision {
    Broadcast {
        rate_sat_vb: f64,
        source: &'static str,
        cap_sat_vb: f64,
        next_block_min_sat_vb: f64,
    },
    /// Keep collecting. `floor_above_cap` is set when the acceptance floor,
    /// not the next-block minimum, is what sits above the cap.
    Hold {
        next_block_min_sat_vb: f64,
        cap_sat_vb: f64,
        floor_above_cap: bool,
    },
    /// No positive next-block feerate, and no recent-block figure was offered.
    NoEstimate { cap_sat_vb: f64 },
    /// No positive next-block feerate. A recent-block figure was offered and refused.
    FallbackSkip { cap_sat_vb: f64 },
}

/// `None` for dynamic means on. A set fixed rate is refused while dynamic is on
/// so the two meanings of `BARI_FEE_RATE_SAT_VB` cannot both apply.
///
/// The cap must be exactly [`DEFAULT_FEE_CAP_SAT_VB`] when set, or unset.
/// The multiplier must be exactly [`DEFAULT_FEE_MULTIPLIER`] when set, or unset.
/// A fixed rate must be finite and strictly below the cap.
///
/// `BARI_FEE_BID_SAT_VB` and `BARI_DEFER_ABOVE_SAT_VB` are not read here.
pub fn resolve_policy(
    dynamic: Option<bool>,
    fixed_rate_sat_vb: Option<f64>,
    cap_sat_vb: Option<f64>,
    multiplier: Option<f64>,
) -> Result<FeePolicy, String> {
    let cap = match cap_sat_vb {
        None => DEFAULT_FEE_CAP_SAT_VB,
        Some(cap)
            if centi_sat_vb(cap) == centi_sat_vb(DEFAULT_FEE_CAP_SAT_VB) && cap.is_finite() =>
        {
            DEFAULT_FEE_CAP_SAT_VB
        }
        Some(_) => {
            return Err(format!(
                "BARI_FEE_CAP_SAT_VB must be {DEFAULT_FEE_CAP_SAT_VB} sat/vB or unset"
            ));
        }
    };
    let multiplier = match multiplier {
        None => DEFAULT_FEE_MULTIPLIER,
        Some(multiplier)
            if milli_rate(multiplier) == milli_rate(DEFAULT_FEE_MULTIPLIER)
                && multiplier.is_finite() =>
        {
            DEFAULT_FEE_MULTIPLIER
        }
        Some(_) => {
            return Err(format!(
                "BARI_FEE_MULTIPLIER must be {DEFAULT_FEE_MULTIPLIER} or unset"
            ));
        }
    };
    if dynamic.unwrap_or(true) {
        if fixed_rate_sat_vb.is_some() {
            return Err(
                "BARI_FEE_RATE_SAT_VB is the fixed broadcast rate. Unset it to use the multiplier, or set BARI_FEE_DYNAMIC=0 to keep that fixed rate"
                    .to_owned(),
            );
        }
        return Ok(FeePolicy::Dynamic {
            cap_sat_vb: cap,
            multiplier,
        });
    }
    let (rate_sat_vb, source) = match fixed_rate_sat_vb {
        Some(rate)
            if !rate.is_finite()
                || rate <= 0.0
                || rate_exceeds_cap(rate, cap)
                || rate_equals_cap(rate, cap) =>
        {
            return Err(format!(
                "explicit fee rate must be finite, above 0, and below {DEFAULT_FEE_CAP_SAT_VB} sat/vB"
            ));
        }
        Some(rate) => (rate, FEE_SOURCE_CONFIGURED),
        None => {
            return Err(format!(
                "BARI_FEE_DYNAMIC=0 requires BARI_FEE_RATE_SAT_VB below {DEFAULT_FEE_CAP_SAT_VB} sat/vB"
            ));
        }
    };
    Ok(FeePolicy::Fixed {
        rate_sat_vb,
        source,
        cap_sat_vb: cap,
    })
}

/// What to do before building a tree. `Broadcast` is the only arm that may send.
#[derive(Clone, Debug, PartialEq)]
pub enum PreBroadcast {
    Broadcast {
        rate_sat_vb: f64,
        source: &'static str,
        cap_sat_vb: f64,
        next_block_min_sat_vb: f64,
        node_estimate_sat_vb: Option<f64>,
    },
    /// Waypoints stay collected. No tree and no PSBT.
    Hold {
        reason: &'static str,
        message: String,
        next_block_min_sat_vb: Option<f64>,
        node_estimate_sat_vb: Option<f64>,
        bid_sat_vb: Option<f64>,
        cap_sat_vb: f64,
    },
}

/// v0.6.2 gate. Read the mempool next-block minimum before a tree exists.
///
/// Fire only while that minimum is below the cap. The bid is
/// `max(min(multiplier × minimum, cap), relay floor)`. A relay or mempool
/// floor above the cap holds. No next-block answer holds. A recent-block
/// figure is not spent. `node_estimate_sat_vb` is copied through and is not
/// compared to the cap.
pub fn pre_broadcast(
    view: &NodeFeeView,
    cap_sat_vb: f64,
    multiplier: f64,
) -> Result<PreBroadcast, String> {
    let node_estimate_sat_vb = view.node_estimate_sat_vb;
    match select_bid(view, cap_sat_vb, multiplier)? {
        FeeDecision::Broadcast {
            rate_sat_vb,
            source,
            cap_sat_vb,
            next_block_min_sat_vb,
        } => Ok(PreBroadcast::Broadcast {
            rate_sat_vb,
            source,
            cap_sat_vb,
            next_block_min_sat_vb,
            node_estimate_sat_vb,
        }),
        FeeDecision::Hold {
            next_block_min_sat_vb,
            cap_sat_vb,
            floor_above_cap,
        } => {
            let bid = Some(bid_at(
                next_block_min_sat_vb,
                cap_sat_vb,
                multiplier,
                view.relay_min_sat_vb,
            ));
            if floor_above_cap {
                let floor = view.relay_min_sat_vb.max(view.mempool_min_sat_vb);
                Ok(PreBroadcast::Hold {
                    reason: HOLD_REASON_CAP,
                    message: floor_hold_message(floor, cap_sat_vb),
                    next_block_min_sat_vb: Some(next_block_min_sat_vb),
                    node_estimate_sat_vb,
                    bid_sat_vb: bid,
                    cap_sat_vb,
                })
            } else {
                Ok(PreBroadcast::Hold {
                    reason: HOLD_REASON_NEXT_BLOCK_MIN,
                    message: hold_message(next_block_min_sat_vb, cap_sat_vb),
                    next_block_min_sat_vb: Some(next_block_min_sat_vb),
                    node_estimate_sat_vb,
                    bid_sat_vb: bid,
                    cap_sat_vb,
                })
            }
        }
        FeeDecision::NoEstimate { cap_sat_vb } => Ok(PreBroadcast::Hold {
            reason: HOLD_REASON_NO_ESTIMATE,
            message: no_estimate_hold_message(cap_sat_vb),
            next_block_min_sat_vb: None,
            node_estimate_sat_vb,
            bid_sat_vb: None,
            cap_sat_vb,
        }),
        FeeDecision::FallbackSkip { cap_sat_vb } => Ok(PreBroadcast::Hold {
            reason: HOLD_REASON_FALLBACK,
            message: fallback_hold_message(cap_sat_vb),
            next_block_min_sat_vb: None,
            node_estimate_sat_vb,
            bid_sat_vb: None,
            cap_sat_vb,
        }),
    }
}

/// `true` when `rate` must not be broadcast under `cap`. An unusable number
/// does not broadcast. Equal to the cap still broadcasts.
pub fn rate_exceeds_cap(rate_sat_vb: f64, cap_sat_vb: f64) -> bool {
    match (centi_sat_vb(rate_sat_vb), centi_sat_vb(cap_sat_vb)) {
        (Some(rate), Some(cap)) => rate > cap,
        _ => true,
    }
}

fn rate_equals_cap(rate_sat_vb: f64, cap_sat_vb: f64) -> bool {
    match (centi_sat_vb(rate_sat_vb), centi_sat_vb(cap_sat_vb)) {
        (Some(rate), Some(cap)) => rate == cap,
        _ => false,
    }
}

/// `min(multiplier × next-block minimum, cap)`, in sat/vB.
///
/// The product is rounded to the nearest hundredth of a sat/vB, half away
/// from zero. A minimum at or above the cap still names the cap: that figure
/// is what the operator would have bid, and the gate holds instead of sending it
/// only when the minimum itself is at or above the cap.
///
/// This is the cap clamp only. [`bid_at`] raises the result to the relay floor.
pub fn would_bid(next_block_min_sat_vb: f64, cap_sat_vb: f64, multiplier: f64) -> f64 {
    let Some(min) = centi_sat_vb(next_block_min_sat_vb) else {
        return 0.0;
    };
    let Some(cap) = centi_sat_vb(cap_sat_vb) else {
        return 0.0;
    };
    let Some(milli) = milli_rate(multiplier) else {
        return 0.0;
    };
    // hundredths × thousandths of the multiplier, then back to hundredths.
    // Half away from zero: floor((product + 500) / 1000).
    let product = u64::from(min) * u64::from(milli);
    let half_away = u32::try_from(product.saturating_add(500) / 1_000).unwrap_or(u32::MAX);
    from_centi(half_away.min(cap))
}

/// `max(min(multiplier × next-block minimum, cap), relay floor)`.
///
/// The relay raise stops at the cap. A relay above the cap does not become
/// the named bid; the gate holds instead of sending it.
pub fn bid_at(
    next_block_min_sat_vb: f64,
    cap_sat_vb: f64,
    multiplier: f64,
    relay_sat_vb: f64,
) -> f64 {
    let capped = would_bid(next_block_min_sat_vb, cap_sat_vb, multiplier);
    let Some(bid) = centi_sat_vb(capped) else {
        return 0.0;
    };
    let Some(cap) = centi_sat_vb(cap_sat_vb) else {
        return from_centi(bid);
    };
    let Some(relay) = centi_sat_vb(relay_sat_vb).filter(|relay| *relay > 0) else {
        return from_centi(bid.min(cap));
    };
    if relay > cap {
        return from_centi(bid.min(cap));
    }
    from_centi(bid.max(relay))
}

/// Mempool next-block minimum, then the multiplier, then the cap, then the relay floor.
///
/// A positive mempool minimum is required. The bid is
/// `max(min(multiplier × minimum, cap), relay floor)`. `mempoolminfee` is not
/// added on top of that. The higher of the relay floor and `mempoolminfee`
/// holds with reason `cap` only when the minimum itself is below the cap and
/// that acceptance floor is above the cap. This function returns
/// [`FeeDecision::Hold`] when the minimum is at or above the cap, before the
/// floor is applied. No answer is `no-estimate`. A recent-block figure without
/// that answer is `fallback` and does not become a rate.
/// `node_estimate_sat_vb` is not read here.
pub fn select_bid(
    view: &NodeFeeView,
    cap_sat_vb: f64,
    multiplier: f64,
) -> Result<FeeDecision, String> {
    let cap = centi_sat_vb(cap_sat_vb).ok_or("fee cap is not a usable sat/vB rate")?;
    let _ = milli_rate(multiplier).ok_or("fee multiplier is not a usable rate")?;
    let estimate = view
        .estimate_sat_vb
        .and_then(centi_sat_vb)
        .filter(|rate| *rate > 0);
    let recent = view
        .recent_min_sat_vb
        .and_then(centi_sat_vb)
        .filter(|rate| *rate > 0);
    let Some(estimate) = estimate else {
        return Ok(if recent.is_some() {
            FeeDecision::FallbackSkip {
                cap_sat_vb: from_centi(cap),
            }
        } else {
            FeeDecision::NoEstimate {
                cap_sat_vb: from_centi(cap),
            }
        });
    };
    let min = from_centi(estimate);
    let cap_f = from_centi(cap);
    if estimate >= cap {
        return Ok(FeeDecision::Hold {
            next_block_min_sat_vb: min,
            cap_sat_vb: cap_f,
            floor_above_cap: false,
        });
    }
    let bid = bid_at(min, cap_f, multiplier, view.relay_min_sat_vb);
    let relay =
        centi_sat_vb(view.relay_min_sat_vb).ok_or("relay minimum is not a usable sat/vB rate")?;
    let mempool = centi_sat_vb(view.mempool_min_sat_vb).unwrap_or(relay);
    // `mempoolminfee` is not an additive raise. It holds only when the node
    // would refuse every rate at or under the cap.
    let acceptance = relay.max(mempool);
    let bid_centi = centi_sat_vb(bid).unwrap_or(0);
    if acceptance > cap {
        return Ok(FeeDecision::Hold {
            next_block_min_sat_vb: min,
            cap_sat_vb: cap_f,
            floor_above_cap: true,
        });
    }
    let (rate, from_low_end) = if bid_centi > relay {
        (bid_centi, true)
    } else {
        (relay, false)
    };
    Ok(FeeDecision::Broadcast {
        rate_sat_vb: from_centi(rate),
        source: if from_low_end {
            FEE_SOURCE_ESTIMATE
        } else {
            FEE_SOURCE_FLOOR
        },
        cap_sat_vb: cap_f,
        next_block_min_sat_vb: min,
    })
}

/// Same gate as [`select_bid`], with a floor above the cap named `cap`.
///
/// [`pre_broadcast`] uses this so a mempool floor above 1.1 holds for `cap`
/// while the next-block minimum is still below the cap. A minimum at or above
/// the cap stays `next-block-min`.
pub fn pre_broadcast_with_floor(
    view: &NodeFeeView,
    cap_sat_vb: f64,
    multiplier: f64,
) -> Result<PreBroadcast, String> {
    let decision = pre_broadcast(view, cap_sat_vb, multiplier)?;
    let PreBroadcast::Broadcast {
        rate_sat_vb: _,
        cap_sat_vb,
        next_block_min_sat_vb,
        node_estimate_sat_vb,
        ..
    } = &decision
    else {
        return Ok(decision);
    };
    let relay = centi_sat_vb(view.relay_min_sat_vb).unwrap_or(0);
    let mempool = centi_sat_vb(view.mempool_min_sat_vb).unwrap_or(relay);
    let floor = relay.max(mempool);
    let cap = centi_sat_vb(*cap_sat_vb).unwrap_or(0);
    if floor > cap {
        let bid = bid_at(
            *next_block_min_sat_vb,
            *cap_sat_vb,
            multiplier,
            view.relay_min_sat_vb,
        );
        return Ok(PreBroadcast::Hold {
            reason: HOLD_REASON_CAP,
            message: floor_hold_message(from_centi(floor), *cap_sat_vb),
            next_block_min_sat_vb: Some(*next_block_min_sat_vb),
            node_estimate_sat_vb: *node_estimate_sat_vb,
            bid_sat_vb: Some(bid),
            cap_sat_vb: *cap_sat_vb,
        });
    }
    Ok(decision)
}

pub fn hold_message(min_sat_vb: f64, cap_sat_vb: f64) -> String {
    format!(
        "holding, min {} is above the {} cap",
        format_sat_vb(min_sat_vb),
        format_sat_vb(cap_sat_vb),
    )
}

pub fn floor_hold_message(floor_sat_vb: f64, cap_sat_vb: f64) -> String {
    format!(
        "holding, acceptance floor {} sat/vB is above the {} cap",
        format_sat_vb(floor_sat_vb),
        format_sat_vb(cap_sat_vb),
    )
}

pub fn no_estimate_hold_message(cap_sat_vb: f64) -> String {
    format!(
        "holding, no next-block estimatesmartfee answer under the {} cap; recent-block history is not a fee",
        format_sat_vb(cap_sat_vb),
    )
}

pub fn fallback_hold_message(cap_sat_vb: f64) -> String {
    format!(
        "holding, no next-block estimatesmartfee answer under the {} cap; refusing a recent-block fallback",
        format_sat_vb(cap_sat_vb),
    )
}

/// `estimatesmartfee` `feerate` is BTC/kvB. `1e-5` BTC/kvB is 1 sat/vB.
pub fn sat_vb_from_btc_per_kvb(btc_per_kvb: f64) -> Option<f64> {
    if !btc_per_kvb.is_finite() || btc_per_kvb < 0.0 {
        return None;
    }
    let sat_vb = btc_per_kvb * 100_000.0;
    centi_sat_vb(sat_vb).map(from_centi)
}

pub fn parse_estimatesmartfee_sat_vb(value: &Value) -> Result<Option<f64>, String> {
    let Some(rate) = value.get("feerate") else {
        return Ok(None);
    };
    if rate.is_null() {
        return Ok(None);
    }
    let btc = json_f64(rate).ok_or("estimatesmartfee feerate is not numeric")?;
    Ok(sat_vb_from_btc_per_kvb(btc).filter(|rate| *rate > 0.0))
}

pub fn parse_mempool_fees(value: &Value) -> Result<(f64, f64), String> {
    let relay = required_btc_per_kvb(value, "minrelaytxfee")?;
    let mempool = required_btc_per_kvb(value, "mempoolminfee")?;
    Ok((relay, mempool))
}

/// One block is 1_000_000 vB. The template leaves the coinbase and a reserve.
pub const BLOCK_VSIZE_BUDGET: u64 = 1_000_000;

/// Coinbase plus a small reserve, subtracted from [`BLOCK_VSIZE_BUDGET`].
pub const BLOCK_RESERVE_VSIZE: u64 = 4_000;

/// One verbose `getrawmempool` entry.
///
/// `ancestor_fee_btc` is `fees.ancestor` (BTC). `ancestor_vsize` is
/// `ancestorsize` (vB). `vsize` is the entry's own virtual size.
#[derive(Clone, Debug, PartialEq)]
pub struct MempoolEntry {
    pub txid: String,
    pub vsize: u64,
    pub ancestor_fee_btc: f64,
    pub ancestor_vsize: u64,
    pub depends: Vec<String>,
}

/// Parse a verbose `getrawmempool` object. The object key is the txid.
///
/// An entry without a usable vsize or ancestor fee is skipped. The value
/// must be an object. This is one JSON value, not a stream. A live mempool
/// of about 57k transactions fits in the existing RPC client.
pub fn parse_raw_mempool(value: &Value) -> Result<Vec<MempoolEntry>, String> {
    let map = value
        .as_object()
        .ok_or("getrawmempool verbose result is not an object")?;
    let mut entries = Vec::with_capacity(map.len());
    for (txid, entry) in map {
        let Some(vsize) = entry_vsize(entry) else {
            continue;
        };
        let Some(ancestor_vsize) = entry.get("ancestorsize").and_then(json_u64) else {
            continue;
        };
        if ancestor_vsize == 0 {
            continue;
        }
        let Some(ancestor_fee_btc) = entry
            .get("fees")
            .and_then(|fees| fees.get("ancestor"))
            .and_then(json_f64)
        else {
            continue;
        };
        if !ancestor_fee_btc.is_finite() || ancestor_fee_btc < 0.0 {
            continue;
        }
        let depends = entry
            .get("depends")
            .and_then(Value::as_array)
            .map(|parents| {
                parents
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        entries.push(MempoolEntry {
            txid: txid.clone(),
            vsize,
            ancestor_fee_btc,
            ancestor_vsize,
            depends,
        });
    }
    Ok(entries)
}

/// Lowest ancestor feerate that would land in the next block, in sat/vB.
///
/// Ancestor feerate is `fees.ancestor` (BTC, converted to satoshis with
/// half-away rounding) divided by `ancestorsize` (vB). The reported rate is
/// rounded to the nearest hundredth of a sat/vB, the same grid as the gate.
///
/// Package approximation. Entries are ordered by ancestor feerate, highest
/// first. Ties break by txid so the traversal is deterministic. A package is the
/// entry plus the in-mempool ancestors named by `depends`, visited
/// transitively. The first time a package is taken, only the not-yet-included
/// members are marked, and only those members' own virtual sizes are charged.
/// A later child whose parent is already in the template adds only itself.
/// `ancestorsize` prices the package. It is not added again for every member.
/// A package that does not fit is skipped. A later smaller package may still
/// fit. Ancestor feerates are not recomputed after a package is taken.
/// Bitcoin Core's block template does that refresh. This traversal does not.
/// A missing `depends` list charges the entry's own vsize once.
///
/// The block budget is [`BLOCK_VSIZE_BUDGET`] minus [`BLOCK_RESERVE_VSIZE`].
/// An empty mempool, or a mempool with nothing that can be priced, returns
/// `relay_floor`. When every priced entry's own vsize fits in the budget, the
/// result is `max(relay floor, lowest ancestor feerate present)`. When some
/// entry is left out, the result is the lowest ancestor feerate among the
/// packages that were taken. A parent pulled in by a child is scored at the
/// child's package rate, not at the parent's own lower rate.
pub fn next_block_min_from_mempool(entries: &[MempoolEntry], relay_floor_sat_vb: f64) -> f64 {
    let relay = centi_sat_vb(relay_floor_sat_vb).unwrap_or(0);
    let mut priced = Vec::new();
    let mut index_of: HashMap<String, usize> = HashMap::new();
    for entry in entries {
        let Some(row) = price_entry(entry) else {
            continue;
        };
        if index_of.contains_key(&row.txid) {
            continue;
        }
        let index = priced.len();
        index_of.insert(row.txid.clone(), index);
        priced.push(row);
    }
    if priced.is_empty() {
        return from_centi(relay);
    }
    let budget = BLOCK_VSIZE_BUDGET.saturating_sub(BLOCK_RESERVE_VSIZE);
    let total_vsize: u64 = priced.iter().map(|row| row.vsize).sum();
    if total_vsize <= budget {
        let lowest = priced
            .iter()
            .map(|row| row.rate_centi)
            .min()
            .unwrap_or(relay);
        return from_centi(lowest.max(relay));
    }

    let mut order: Vec<usize> = (0..priced.len()).collect();
    order.sort_by(|&left, &right| {
        cmp_rate_desc(&priced[left], &priced[right])
            .then_with(|| priced[left].txid.cmp(&priced[right].txid))
    });
    let mut included = vec![false; priced.len()];
    let mut remaining = budget;
    let mut lowest_included: Option<u32> = None;
    for index in order {
        if included[index] {
            continue;
        }
        let package = ancestor_package(index, &priced, &index_of, &included);
        let size: u64 = package.iter().map(|&member| priced[member].vsize).sum();
        if size == 0 || size > remaining {
            continue;
        }
        for member in package {
            included[member] = true;
        }
        remaining = remaining.saturating_sub(size);
        let rate = priced[index].rate_centi;
        lowest_included = Some(lowest_included.map_or(rate, |kept| kept.min(rate)));
    }
    from_centi(lowest_included.unwrap_or(relay))
}

struct PricedEntry {
    txid: String,
    vsize: u64,
    ancestor_fee_sats: u64,
    ancestor_vsize: u64,
    depends: Vec<String>,
    rate_centi: u32,
}

fn price_entry(entry: &MempoolEntry) -> Option<PricedEntry> {
    if entry.vsize == 0 || entry.ancestor_vsize == 0 {
        return None;
    }
    let ancestor_fee_sats = u64::try_from(sats_from_btc(entry.ancestor_fee_btc)?).ok()?;
    let rate_centi = rate_centi(ancestor_fee_sats, entry.ancestor_vsize)?;
    Some(PricedEntry {
        txid: entry.txid.clone(),
        vsize: entry.vsize,
        ancestor_fee_sats,
        ancestor_vsize: entry.ancestor_vsize,
        depends: entry.depends.clone(),
        rate_centi,
    })
}

/// Hundredths of a sat/vB. Half away from zero.
fn rate_centi(fee_sats: u64, vsize: u64) -> Option<u32> {
    if vsize == 0 {
        return None;
    }
    let fee = u128::from(fee_sats);
    let size = u128::from(vsize);
    let rounded = fee.saturating_mul(100).saturating_add(size / 2) / size;
    u32::try_from(rounded).ok()
}

fn cmp_rate_desc(left: &PricedEntry, right: &PricedEntry) -> std::cmp::Ordering {
    let left_cross =
        u128::from(left.ancestor_fee_sats).saturating_mul(u128::from(right.ancestor_vsize));
    let right_cross =
        u128::from(right.ancestor_fee_sats).saturating_mul(u128::from(left.ancestor_vsize));
    right_cross.cmp(&left_cross)
}

fn ancestor_package(
    start: usize,
    priced: &[PricedEntry],
    index_of: &HashMap<String, usize>,
    included: &[bool],
) -> Vec<usize> {
    let mut package = Vec::new();
    let mut stack = vec![start];
    let mut seen = HashSet::new();
    while let Some(index) = stack.pop() {
        if included[index] || !seen.insert(index) {
            continue;
        }
        package.push(index);
        for parent in &priced[index].depends {
            if let Some(&parent_index) = index_of.get(parent) {
                stack.push(parent_index);
            }
        }
    }
    package
}

fn entry_vsize(entry: &Value) -> Option<u64> {
    if let Some(vsize) = entry.get("vsize").and_then(json_u64) {
        if vsize > 0 {
            return Some(vsize);
        }
    }
    let weight = entry.get("weight").and_then(json_u64)?;
    if weight == 0 {
        return None;
    }
    Some(weight.saturating_add(3) / 4)
}

fn json_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| {
            value
                .as_i64()
                .filter(|number| *number >= 0)
                .map(|number| number as u64)
        })
        .or_else(|| {
            value.as_f64().and_then(|number| {
                if number.is_finite() && number >= 0.0 && number <= u64::MAX as f64 {
                    Some(number.round() as u64)
                } else {
                    None
                }
            })
        })
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

/// `ceil(rate × vsize)` sats. 1.1 sat/vB and 159 vB is 175 sats.
pub fn broadcast_fee_sats(rate_sat_vb: f64, vsize: u64) -> Option<u64> {
    if vsize == 0 {
        return None;
    }
    let centi = centi_sat_vb(rate_sat_vb).filter(|centi| *centi > 0)?;
    let product = u128::from(centi) * u128::from(vsize);
    let sats = product.div_ceil(100);
    u64::try_from(sats).ok()
}

/// Name kept for the fund path. Same as [`broadcast_fee_sats`].
pub fn plant_fee_sats(rate_sat_vb: f64, vsize: u64) -> Option<u64> {
    broadcast_fee_sats(rate_sat_vb, vsize)
}

/// Round half away from zero. A negative fee is not a rate.
pub fn sats_from_btc(btc: f64) -> Option<i64> {
    if !btc.is_finite() || btc < 0.0 {
        return None;
    }
    let sats = (btc * 100_000_000.0).round();
    if !(0.0..=i64::MAX as f64).contains(&sats) {
        return None;
    }
    Some(sats as i64)
}

pub fn format_sat_vb(rate: f64) -> String {
    let Some(centi) = centi_sat_vb(rate) else {
        return "0.0".to_owned();
    };
    let whole = centi / 100;
    let frac = centi % 100;
    if frac == 0 {
        format!("{whole}.0")
    } else if frac % 10 == 0 {
        format!("{whole}.{}", frac / 10)
    } else {
        format!("{whole}.{frac:02}")
    }
}

pub fn fee_log_label(policy: &FeePolicy) -> String {
    match *policy {
        FeePolicy::Dynamic {
            cap_sat_vb,
            multiplier,
        } => format!(
            "fee=bid max(min(multiplier×next-block, cap), relay) cap={} multiplier={} fire_below={} target=mempool-next-block mode={}",
            format_sat_vb(cap_sat_vb),
            format_multiplier(multiplier),
            format_sat_vb(cap_sat_vb),
            FEE_ESTIMATE_MODE,
        ),
        FeePolicy::Fixed {
            rate_sat_vb,
            source,
            cap_sat_vb,
        } => format!(
            "fee=fixed rate={} cap={} source={source}",
            format_sat_vb(rate_sat_vb),
            format_sat_vb(cap_sat_vb),
        ),
    }
}

fn format_multiplier(multiplier: f64) -> String {
    let Some(milli) = milli_rate(multiplier) else {
        return "0".to_owned();
    };
    let whole = milli / 1000;
    let frac = milli % 1000;
    if frac == 0 {
        format!("{whole}")
    } else {
        format!("{whole}.{frac:03}")
            .trim_end_matches('0')
            .to_owned()
    }
}

fn required_btc_per_kvb(value: &Value, field: &str) -> Result<f64, String> {
    let raw = value
        .get(field)
        .ok_or_else(|| format!("getmempoolinfo omitted {field}"))?;
    let btc = json_f64(raw).ok_or_else(|| format!("getmempoolinfo {field} is not numeric"))?;
    sat_vb_from_btc_per_kvb(btc)
        .ok_or_else(|| format!("getmempoolinfo {field} is not a usable BTC/kvB rate"))
}

pub(crate) fn json_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_u64().map(|number| number as f64))
        .or_else(|| {
            value
                .as_i64()
                .filter(|number| *number >= 0)
                .map(|number| number as f64)
        })
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

/// Hundredths of a sat/vB. `None` when the rate cannot be used.
fn centi_sat_vb(rate: f64) -> Option<u32> {
    if !rate.is_finite() || rate < 0.0 {
        return None;
    }
    let scaled = (rate * 100.0).round();
    if !(0.0..=u32::MAX as f64).contains(&scaled) {
        return None;
    }
    Some(scaled as u32)
}

/// Thousandths. Used for the multiplier (1.5 → 1500).
fn milli_rate(rate: f64) -> Option<u32> {
    if !rate.is_finite() || rate < 0.0 {
        return None;
    }
    let scaled = (rate * 1000.0).round();
    if !(0.0..=u32::MAX as f64).contains(&scaled) {
        return None;
    }
    Some(scaled as u32)
}

fn from_centi(centi: u32) -> f64 {
    f64::from(centi) / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn view(estimate: Option<f64>, recent: Option<f64>, relay: f64, mempool: f64) -> NodeFeeView {
        NodeFeeView {
            estimate_sat_vb: estimate,
            node_estimate_sat_vb: None,
            recent_min_sat_vb: recent,
            relay_min_sat_vb: relay,
            mempool_min_sat_vb: mempool,
        }
    }

    fn broadcast(decision: &FeeDecision) -> (f64, &'static str) {
        match decision {
            FeeDecision::Broadcast {
                rate_sat_vb,
                source,
                ..
            } => (*rate_sat_vb, *source),
            other => panic!("expected a broadcast, got {other:?}"),
        }
    }

    #[test]
    fn bid_is_min_of_one_point_five_times_the_minimum_and_the_cap() {
        assert_eq!(DEFAULT_FEE_CAP_SAT_VB, 1.1);
        assert_eq!(DEFAULT_FEE_MULTIPLIER, 1.5);
        assert_eq!(FEE_ESTIMATE_CONF_TARGET, 1);
        assert_eq!(FEE_ESTIMATE_MODE, "economical");
        // 0.70 × 1.5 = 1.05, under the cap.
        assert_eq!(would_bid(0.70, 1.1, 1.5), 1.05);
        let decision = select_bid(&view(Some(0.70), Some(5.0), 0.1, 0.1), 1.1, 1.5).unwrap();
        assert_eq!(broadcast(&decision), (1.05, FEE_SOURCE_ESTIMATE));
        // 0.80 × 1.5 = 1.20, clamped to 1.1. The minimum is still below the cap.
        assert_eq!(would_bid(0.80, 1.1, 1.5), 1.1);
        let capped = select_bid(&view(Some(0.80), None, 0.1, 0.1), 1.1, 1.5).unwrap();
        assert_eq!(broadcast(&capped), (1.1, FEE_SOURCE_ESTIMATE));
        // Half-sat rounding: 0.73 × 1.5 = 1.095 → 1.10.
        assert_eq!(would_bid(0.73, 1.1, 1.5), 1.10);
    }

    #[test]
    fn a_minimum_at_or_above_the_cap_holds_and_does_not_broadcast() {
        let at_cap = select_bid(&view(Some(1.1), None, 0.1, 0.1), 1.1, 1.5).unwrap();
        match at_cap {
            FeeDecision::Hold {
                next_block_min_sat_vb,
                cap_sat_vb,
                floor_above_cap,
            } => {
                assert_eq!(next_block_min_sat_vb, 1.1);
                assert_eq!(cap_sat_vb, 1.1);
                assert!(!floor_above_cap);
            }
            other => panic!("min at the cap must hold, got {other:?}"),
        }
        let above = select_bid(&view(Some(2.2), None, 0.1, 0.1), 1.1, 1.5).unwrap();
        assert!(matches!(
            above,
            FeeDecision::Hold {
                next_block_min_sat_vb,
                floor_above_cap: false,
                ..
            } if next_block_min_sat_vb == 2.2
        ));
        let message = hold_message(2.2, 1.1);
        assert_eq!(message, "holding, min 2.2 is above the 1.1 cap");
        assert!(!message.contains("bumpfee"));
    }

    #[test]
    fn a_minimum_just_under_the_cap_fires_at_the_cap() {
        let decision = select_bid(&view(Some(1.09), None, 0.1, 0.1), 1.1, 1.5).unwrap();
        // 1.09 × 1.5 = 1.635, clamped to 1.1.
        assert_eq!(broadcast(&decision), (1.1, FEE_SOURCE_ESTIMATE));
    }

    #[test]
    fn a_recent_block_does_not_raise_or_replace_a_real_estimate() {
        let decision = select_bid(&view(Some(0.40), Some(5.0), 0.1, 0.1), 1.1, 1.5).unwrap();
        assert_eq!(broadcast(&decision), (0.60, FEE_SOURCE_ESTIMATE));
    }

    #[test]
    fn relay_floor_binds_when_the_bid_is_below_it() {
        // 0.20 × 1.5 = 0.30, floor 1.0, still under the cap.
        let decision = select_bid(&view(Some(0.20), None, 1.0, 0.1), 1.1, 1.5).unwrap();
        assert_eq!(broadcast(&decision), (1.0, FEE_SOURCE_FLOOR));
    }

    #[test]
    fn acceptance_floor_above_the_cap_holds() {
        // select_bid holds when the floor is above the cap. The reason name
        // `cap` is applied by pre_broadcast_with_floor, below.
        let decision = select_bid(&view(Some(0.40), None, 0.1, 1.2), 1.1, 1.5).unwrap();
        assert!(matches!(
            decision,
            FeeDecision::Hold {
                next_block_min_sat_vb,
                cap_sat_vb,
                floor_above_cap: true,
            } if next_block_min_sat_vb == 0.40 && cap_sat_vb == 1.1
        ));
        let pre = pre_broadcast_with_floor(&view(Some(0.40), None, 0.1, 1.2), 1.1, 1.5).unwrap();
        match pre {
            PreBroadcast::Hold {
                reason,
                message,
                bid_sat_vb,
                cap_sat_vb,
                ..
            } => {
                assert_eq!(reason, HOLD_REASON_CAP);
                assert_eq!(cap_sat_vb, 1.1);
                assert_eq!(bid_sat_vb, Some(0.60));
                assert!(message.contains("holding"), "{message}");
                assert!(message.contains("1.2"), "{message}");
            }
            PreBroadcast::Broadcast { rate_sat_vb, .. } => {
                panic!("floor above the cap must not broadcast, got {rate_sat_vb}")
            }
        }
    }

    #[test]
    fn missing_estimate_holds_and_does_not_guess_a_floor() {
        let decision = select_bid(&view(None, None, 1.0, 1.0), 1.1, 1.5).unwrap();
        assert!(matches!(
            decision,
            FeeDecision::NoEstimate { cap_sat_vb } if cap_sat_vb == 1.1
        ));
        let message = no_estimate_hold_message(1.1);
        assert!(message.contains("holding"));
        assert!(message.contains("not a fee"));
        assert!(!message.contains("bumpfee"));
    }

    #[test]
    fn a_recent_block_without_an_estimate_is_not_a_fee() {
        let decision = select_bid(&view(None, Some(1.0), 1.0, 1.0), 1.1, 1.5).unwrap();
        assert!(matches!(
            decision,
            FeeDecision::FallbackSkip { cap_sat_vb } if cap_sat_vb == 1.1
        ));
        let message = fallback_hold_message(1.1);
        assert!(message.contains("holding"));
        assert!(message.contains("fallback"));
    }

    #[test]
    fn a_zero_estimate_is_not_a_next_block_answer() {
        let decision = select_bid(&view(Some(0.0), None, 0.1, 0.1), 1.1, 1.5).unwrap();
        assert!(matches!(decision, FeeDecision::NoEstimate { .. }));
    }

    #[test]
    fn absolute_fee_is_the_ceil_of_rate_times_vsize() {
        assert_eq!(broadcast_fee_sats(1.1, 159), Some(175));
        assert_eq!(broadcast_fee_sats(0.0, 159), None);
        assert_eq!(sats_from_btc(0.00000175), Some(175));
        assert_eq!(sats_from_btc(-0.00000001), None);
    }

    #[test]
    fn btc_per_kvb_converts_to_sat_per_vb() {
        assert_eq!(sat_vb_from_btc_per_kvb(0.00001), Some(1.0));
        assert_eq!(sat_vb_from_btc_per_kvb(0.000011), Some(1.1));
        assert_eq!(sat_vb_from_btc_per_kvb(-0.00001), None);
    }

    #[test]
    fn estimatesmartfee_without_a_feerate_is_no_estimate() {
        let missing = json!({"errors": ["Insufficient data or no feerate found"], "blocks": 1});
        assert_eq!(parse_estimatesmartfee_sat_vb(&missing).unwrap(), None);
        let present = json!({"feerate": 0.000011, "blocks": 1});
        assert_eq!(parse_estimatesmartfee_sat_vb(&present).unwrap(), Some(1.1));
        assert_eq!(
            parse_estimatesmartfee_sat_vb(&json!({"feerate": 0, "blocks": 1})).unwrap(),
            None
        );
    }

    #[test]
    fn mempool_fees_are_relay_and_mempool_minimums() {
        let info = json!({"minrelaytxfee": 0.00001, "mempoolminfee": 0.000011});
        assert_eq!(parse_mempool_fees(&info).unwrap(), (1.0, 1.1));
        assert!(parse_mempool_fees(&json!({"minrelaytxfee": 0.00001})).is_err());
    }

    #[test]
    fn dynamic_is_the_default_cap_one_point_one_multiplier_one_point_five() {
        let dynamic = resolve_policy(None, None, None, None).unwrap();
        assert_eq!(
            dynamic,
            FeePolicy::Dynamic {
                cap_sat_vb: 1.1,
                multiplier: 1.5
            }
        );
        let named = resolve_policy(Some(true), None, Some(1.1), Some(1.5)).unwrap();
        assert_eq!(
            named,
            FeePolicy::Dynamic {
                cap_sat_vb: 1.1,
                multiplier: 1.5
            }
        );
        let fixed = resolve_policy(Some(false), Some(1.0), None, None).unwrap();
        assert_eq!(
            fixed,
            FeePolicy::Fixed {
                rate_sat_vb: 1.0,
                source: FEE_SOURCE_CONFIGURED,
                cap_sat_vb: 1.1,
            }
        );
        let conflict = resolve_policy(None, Some(1.0), None, None).unwrap_err();
        assert!(conflict.contains("BARI_FEE_RATE_SAT_VB"));
        assert!(conflict.contains("BARI_FEE_DYNAMIC=0"));
        assert!(resolve_policy(Some(false), Some(1.1), None, None).is_err());
        assert!(resolve_policy(Some(false), Some(1.2), None, None).is_err());
        assert!(resolve_policy(Some(false), None, None, None).is_err());
        let wrong_cap = resolve_policy(Some(true), None, Some(1.2), None).unwrap_err();
        assert!(wrong_cap.contains("BARI_FEE_CAP_SAT_VB"), "{wrong_cap}");
        assert!(wrong_cap.contains("1.1"), "{wrong_cap}");
        let wrong_multiplier = resolve_policy(Some(true), None, None, Some(2.0)).unwrap_err();
        assert!(
            wrong_multiplier.contains("BARI_FEE_MULTIPLIER"),
            "{wrong_multiplier}"
        );
    }

    #[test]
    fn startup_label_names_the_multiplier_and_the_cap() {
        let label = fee_log_label(&FeePolicy::Dynamic {
            cap_sat_vb: 1.1,
            multiplier: 1.5,
        });
        assert!(label.contains("cap=1.1"), "{label}");
        assert!(label.contains("multiplier=1.5"), "{label}");
        assert!(label.contains("fire_below=1.1"), "{label}");
        assert!(label.contains("economical"), "{label}");
        assert!(label.contains("next-block"), "{label}");
        let fixed = fee_log_label(&FeePolicy::Fixed {
            rate_sat_vb: 1.0,
            source: FEE_SOURCE_CONFIGURED,
            cap_sat_vb: 1.1,
        });
        assert!(fixed.contains("fee=fixed"), "{fixed}");
        assert!(fixed.contains("rate=1.0"), "{fixed}");
    }

    fn gate(estimate: Option<f64>, relay: f64, mempool: f64) -> PreBroadcast {
        pre_broadcast_with_floor(
            &view(estimate, None, relay, mempool),
            DEFAULT_FEE_CAP_SAT_VB,
            DEFAULT_FEE_MULTIPLIER,
        )
        .unwrap()
    }

    #[test]
    fn min_above_the_cap_holds_before_a_tree() {
        let decision = gate(Some(2.2), 0.1, 0.1);
        match decision {
            PreBroadcast::Hold {
                reason,
                message,
                next_block_min_sat_vb,
                bid_sat_vb,
                cap_sat_vb,
                ..
            } => {
                assert_eq!(reason, HOLD_REASON_NEXT_BLOCK_MIN);
                assert_eq!(next_block_min_sat_vb, Some(2.2));
                assert_eq!(cap_sat_vb, 1.1);
                assert_eq!(bid_sat_vb, Some(1.1));
                assert_eq!(message, "holding, min 2.2 is above the 1.1 cap");
            }
            PreBroadcast::Broadcast { rate_sat_vb, .. } => {
                panic!("min 2.2 must hold, got broadcast at {rate_sat_vb}")
            }
        }
    }

    #[test]
    fn min_just_under_the_cap_broadcasts_at_the_cap() {
        let decision = gate(Some(1.09), 0.1, 0.1);
        match decision {
            PreBroadcast::Broadcast {
                rate_sat_vb,
                source,
                cap_sat_vb,
                next_block_min_sat_vb,
                ..
            } => {
                assert_eq!(rate_sat_vb, 1.1);
                assert_eq!(source, FEE_SOURCE_ESTIMATE);
                assert_eq!(cap_sat_vb, 1.1);
                assert_eq!(next_block_min_sat_vb, 1.09);
                assert!(!rate_exceeds_cap(rate_sat_vb, cap_sat_vb));
            }
            PreBroadcast::Hold {
                reason, message, ..
            } => {
                panic!("min 1.09 must broadcast, held ({reason}): {message}")
            }
        }
    }

    #[test]
    fn a_drop_from_above_the_cap_to_under_it_is_a_broadcast() {
        assert!(matches!(
            gate(Some(2.2), 0.1, 0.1),
            PreBroadcast::Hold {
                reason: HOLD_REASON_NEXT_BLOCK_MIN,
                ..
            }
        ));
        assert!(matches!(
            gate(Some(0.60), 0.1, 0.1),
            PreBroadcast::Broadcast { rate_sat_vb, .. } if rate_sat_vb == 0.90
        ));
    }

    #[test]
    fn missing_next_block_answer_holds() {
        let decision = gate(None, 0.1, 0.1);
        match decision {
            PreBroadcast::Hold {
                reason, message, ..
            } => {
                assert_eq!(reason, HOLD_REASON_NO_ESTIMATE);
                assert!(message.contains("holding"), "{message}");
                assert!(message.contains("not a fee"), "{message}");
            }
            PreBroadcast::Broadcast { .. } => panic!("no estimate must not broadcast"),
        }
        let recent = pre_broadcast_with_floor(&view(None, Some(1.0), 0.1, 0.1), 1.1, 1.5).unwrap();
        assert!(matches!(
            recent,
            PreBroadcast::Hold {
                reason: HOLD_REASON_FALLBACK,
                ..
            }
        ));
    }

    #[test]
    fn btc_per_kvb_relay_floor_is_a_tenth_of_a_sat() {
        // 0.00000100 BTC/kvB = 0.1 sat/vB. 0.00000250 BTC = 250 sats.
        assert_eq!(sat_vb_from_btc_per_kvb(0.00000100), Some(0.1));
        assert_eq!(sats_from_btc(0.00000250), Some(250));
        assert_eq!(sats_from_btc(0.00000010), Some(10));
    }

    #[test]
    fn a_mempool_bid_is_labeled_mempool_not_estimate() {
        let decision = select_bid(&view(Some(0.40), None, 0.1, 0.1), 1.1, 1.5).unwrap();
        match decision {
            FeeDecision::Broadcast {
                source,
                rate_sat_vb,
                ..
            } => {
                assert_eq!(source, FEE_SOURCE_MEMPOOL);
                assert_eq!(source, "mempool");
                assert_ne!(source, "estimate");
                assert_eq!(rate_sat_vb, 0.60);
            }
            other => panic!("expected a mempool bid, got {other:?}"),
        }
        let raised = select_bid(&view(Some(0.20), None, 0.50, 0.10), 1.1, 1.5).unwrap();
        match raised {
            FeeDecision::Broadcast {
                source,
                rate_sat_vb,
                ..
            } => {
                assert_eq!(source, FEE_SOURCE_FLOOR);
                assert_eq!(rate_sat_vb, 0.50);
            }
            other => panic!("expected the relay floor, got {other:?}"),
        }
    }

    #[test]
    fn mempool_minimum_does_not_raise_the_bid_above_the_relay_floor() {
        // 0.20 × 1.5 = 0.30. mempoolminfee 0.80 must not become the bid.
        let decision = select_bid(&view(Some(0.20), None, 0.1, 0.80), 1.1, 1.5).unwrap();
        assert_eq!(broadcast(&decision), (0.30, FEE_SOURCE_ESTIMATE));
        assert_eq!(bid_at(0.20, 1.1, 1.5, 0.1), 0.30);
        // 0.04 × 1.5 = 0.06, raised to the 0.10 relay floor, still under the cap.
        let floored = select_bid(&view(Some(0.04), None, 0.10, 0.10), 1.1, 1.5).unwrap();
        assert_eq!(broadcast(&floored), (0.10, FEE_SOURCE_FLOOR));
        assert_eq!(bid_at(0.04, 1.1, 1.5, 0.10), 0.10);
    }

    #[test]
    fn a_missing_node_estimate_does_not_block_a_mempool_minimum() {
        let mut quoted = view(Some(0.30), None, 0.1, 0.1);
        quoted.node_estimate_sat_vb = None;
        let decision = select_bid(&quoted, 1.1, 1.5).unwrap();
        // 0.30 × 1.5 = 0.45.
        assert_eq!(broadcast(&decision), (0.45, FEE_SOURCE_ESTIMATE));
        let held = select_bid(&view(Some(1.12), None, 0.1, 0.1), 1.1, 1.5).unwrap();
        assert!(matches!(
            held,
            FeeDecision::Hold {
                next_block_min_sat_vb,
                floor_above_cap: false,
                ..
            } if next_block_min_sat_vb == 1.12
        ));
        assert_eq!(
            hold_message(1.12, 1.1),
            "holding, min 1.12 is above the 1.1 cap"
        );
    }

    fn tx(
        txid: &str,
        vsize: u64,
        ancestor_vsize: u64,
        fee_sats: u64,
        depends: &[&str],
    ) -> MempoolEntry {
        MempoolEntry {
            txid: txid.to_owned(),
            vsize,
            ancestor_fee_btc: f64::from(u32::try_from(fee_sats).unwrap_or(u32::MAX))
                / 100_000_000.0,
            ancestor_vsize,
            depends: depends.iter().map(|parent| (*parent).to_owned()).collect(),
        }
    }

    #[test]
    fn empty_mempool_is_the_relay_floor() {
        assert_eq!(next_block_min_from_mempool(&[], 0.1), 0.1);
        let unpriced = MempoolEntry {
            txid: "x".to_owned(),
            vsize: 0,
            ancestor_fee_btc: 0.00000250,
            ancestor_vsize: 0,
            depends: Vec::new(),
        };
        assert_eq!(next_block_min_from_mempool(&[unpriced], 0.1), 0.1);
    }

    #[test]
    fn a_mempool_that_fits_in_one_block_uses_the_lowest_feerate() {
        let entries = vec![
            tx("high", 1_000, 1_000, 2_000, &[]),
            tx("mid", 1_000, 1_000, 500, &[]),
            tx("low", 1_000, 1_000, 300, &[]),
        ];
        // 2.0, 0.5, 0.30. Everything fits. Relay does not raise 0.30.
        assert_eq!(next_block_min_from_mempool(&entries, 0.1), 0.30);
        let below_relay = vec![tx("dust", 1_000, 1_000, 50, &[])];
        // 0.05 is present, and the relay floor is higher.
        assert_eq!(next_block_min_from_mempool(&below_relay, 0.1), 0.1);
    }

    #[test]
    fn a_full_block_stops_at_the_included_boundary() {
        let mut entries = Vec::new();
        for index in 0..90 {
            entries.push(tx(&format!("a{index:02}"), 10_000, 10_000, 30_000, &[]));
        }
        for index in 0..9 {
            entries.push(tx(&format!("b{index}"), 10_000, 10_000, 8_000, &[]));
        }
        entries.push(tx("tail", 10_000, 10_000, 2_500, &[]));
        // 90 × 10_000 at 3.0 and 9 × 10_000 at 0.80 fill 990_000 of 996_000.
        // The 0.25 tail does not fit. The boundary is 0.80, not 0.25.
        assert_eq!(next_block_min_from_mempool(&entries, 0.1), 0.80);
    }

    #[test]
    fn an_ancestor_package_is_charged_once() {
        // Parent 400_000 vB at 5.0. Child package is 800_000 vB at 4.0.
        // After the parent is in, the child adds only its own 400_000 vB.
        // Charging ancestorsize again would leave the child out and the
        // boundary would be 5.0. The tail at 0.2 does not fit.
        let entries = vec![
            tx("parent", 400_000, 400_000, 2_000_000, &[]),
            tx("child", 400_000, 800_000, 3_200_000, &["parent"]),
            tx("filler", 300_000, 300_000, 900_000, &[]),
            tx("tail", 300_000, 300_000, 60_000, &[]),
        ];
        assert_eq!(next_block_min_from_mempool(&entries, 0.1), 4.0);
    }

    #[test]
    fn verbose_mempool_json_converts_btc_to_sat_per_vb() {
        let raw = json!({
            "aa": {
                "vsize": 1000,
                "weight": 4000,
                "ancestorsize": 1000,
                "fees": {"base": 0.00000250, "ancestor": 0.00000250},
                "depends": []
            },
            "bb": {
                "weight": 561,
                "ancestorsize": 141,
                "fees": {"ancestor": 0.00000141},
                "depends": ["aa"]
            }
        });
        let entries = parse_raw_mempool(&raw).unwrap();
        assert_eq!(entries.len(), 2);
        let weighted = entries.iter().find(|entry| entry.txid == "bb").unwrap();
        // ceil(561 / 4) = 141. 141 sats / 141 vB = 1.0 sat/vB.
        assert_eq!(weighted.vsize, 141);
        // Both fit. Lowest ancestor feerate is 0.25. Relay floor is 0.1.
        assert_eq!(next_block_min_from_mempool(&entries, 0.1), 0.25);
        assert_eq!(
            sat_vb_from_btc_per_kvb(0.00000100),
            Some(0.1),
            "minrelaytxfee 0.00000100 BTC/kvB is 0.1 sat/vB"
        );
    }

    #[test]
    fn a_mempool_minimum_at_the_cap_holds() {
        let decision = gate(Some(1.12), 0.1, 0.1);
        match decision {
            PreBroadcast::Hold {
                reason,
                message,
                next_block_min_sat_vb,
                ..
            } => {
                assert_eq!(reason, HOLD_REASON_NEXT_BLOCK_MIN);
                assert_eq!(next_block_min_sat_vb, Some(1.12));
                assert_eq!(message, "holding, min 1.12 is above the 1.1 cap");
            }
            PreBroadcast::Broadcast { rate_sat_vb, .. } => {
                panic!("min 1.12 must hold, got broadcast at {rate_sat_vb}")
            }
        }
        let planted = gate(Some(0.30), 0.1, 0.1);
        match planted {
            PreBroadcast::Broadcast {
                rate_sat_vb,
                next_block_min_sat_vb,
                ..
            } => {
                assert_eq!(next_block_min_sat_vb, 0.30);
                assert_eq!(rate_sat_vb, 0.45);
            }
            PreBroadcast::Hold { message, .. } => {
                panic!("min 0.30 must plant, held: {message}")
            }
        }
    }
}
