//! Bitcoin Core JSON-RPC adapter for the live plant.
//!
//! `BARI_TIP_SOURCE=bitcoin-gossip` uses this plant and does not call
//! [`ChainBackend::best_tip`] to advance the tip. The tip is Bitcoin gossip.
//! The unset `rpc` source still polls `best_tip`. The test plane does not
//! construct this type. `BARI_TIP_SOURCE=synth` uses
//! [`crate::synth_commit::SynthCommit`] and refuses Core plant settings.

use crate::clock::{
    ChainBackend, ChainError, ChainPostCheck, ChainSync, ChainTip, ChainWallet, Deferral, FeeQuote,
    PlantError, PlantReceipt, SealClearance, SkipReason, TxPlace, WalletCoin,
};
use crate::fee::{self, FeeDecision, FeePolicy, NodeFeeView};
use crate::ops;
use crate::proof::{commitment_script, CanonicalPathBody};
use crate::storage::StoredInclusion;
use crate::utxo::{self, PlantInputPlan, SelectError};
use crate::Hash32;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use ureq::{Agent, AgentBuilder};
use url::Url;

pub use crate::utxo::UtxoPolicy;

/// How long the operator page waits for one node read.
///
/// `getrawmempool true` returns every mempool transaction as one JSON value.
/// A live mempool is tens of thousands of entries. Two seconds is shorter
/// than that read, so the page used to time out and say the node was not
/// reachable on the same node that had just broadcast a tree. The page and
/// the plant now share this floor. A configured plant timeout above it is
/// kept. Wallet and sync reads are small and use the same budget so one
/// slow call does not outlive the others.
pub const QUOTE_READ_BUDGET: Duration = Duration::from_secs(30);

/// Page-side timeout for [`ChainBackend::fee_quote`], [`ChainBackend::chain_wallet`],
/// and [`ChainBackend::chain_sync`]. At least [`QUOTE_READ_BUDGET`].
pub const OPS_READ_TIMEOUT: Duration = QUOTE_READ_BUDGET;

#[derive(Clone, Debug)]
pub struct RpcAuth {
    pub username: String,
    pub password: String,
}

impl RpcAuth {
    pub fn from_cookie(cookie: &str) -> Result<Self, ChainError> {
        let (username, password) = cookie
            .trim()
            .split_once(':')
            .ok_or_else(|| ChainError("Bitcoin cookie must contain user:password".to_owned()))?;
        if username.is_empty() || password.is_empty() {
            return Err(ChainError(
                "Bitcoin cookie user and password must be non-empty".to_owned(),
            ));
        }
        Ok(Self {
            username: username.to_owned(),
            password: password.to_owned(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct BitcoinRpcConfig {
    pub rpc_url: Url,
    pub wallet_name: String,
    pub expected_network: String,
    pub auth: RpcAuth,
    /// Set only with `BARI_FEE_DYNAMIC=0`. Refused while the multiplier path is on.
    /// A rate at or above the 1.1 sat/vB cap is refused.
    pub fee_rate_sat_vb: Option<f64>,
    /// Unset means the multiplier path.
    pub fee_dynamic: Option<bool>,
    /// Cap. Unset means 1.1. Any other value is refused.
    pub fee_cap_sat_vb: Option<f64>,
    /// Multiplier. Unset means 1.5. Any other value is refused.
    pub fee_multiplier: Option<f64>,
    pub utxo_policy: UtxoPolicy,
    pub timeout: Duration,
}

#[derive(Clone)]
pub struct BitcoinRpc {
    client: Agent,
    node_url: Url,
    wallet_url: Url,
    auth: RpcAuth,
    expected_network: String,
    fee_policy: FeePolicy,
    utxo_policy: UtxoPolicy,
    /// Next round-robin slot. Shared across clones. A restart starts at zero.
    utxo_cursor: Arc<AtomicU64>,
    /// Read budget for the operator page. The plant uses `config.timeout`.
    /// `getrawmempool true` is one JSON body of the whole mempool, and the
    /// page must not share the plant's 30 s budget: a 2 s page timeout then
    /// reports the node as not reachable while a plant on the same node succeeds.
    quote_client: Agent,
}

#[derive(Debug, Deserialize)]
struct RpcEnvelope {
    result: Option<Value>,
    error: Option<RpcFailure>,
}

#[derive(Debug, Deserialize)]
struct RpcFailure {
    code: i64,
    message: String,
}

impl BitcoinRpc {
    pub fn new(config: BitcoinRpcConfig) -> Result<Self, ChainError> {
        if config.wallet_name.is_empty() {
            return Err(ChainError("BARI_BITCOIN_WALLET is required".to_owned()));
        }
        if config.expected_network.is_empty() {
            return Err(ChainError("BARI_BITCOIN_NETWORK is required".to_owned()));
        }
        let fee_policy = fee::resolve_policy(
            config.fee_dynamic,
            config.fee_rate_sat_vb,
            config.fee_cap_sat_vb,
            config.fee_multiplier,
        )
        .map_err(ChainError)?;
        let mut wallet_url = config.rpc_url.clone();
        {
            let mut segments = wallet_url
                .path_segments_mut()
                .map_err(|_| ChainError("Bitcoin RPC URL cannot be a base URL".to_owned()))?;
            segments.pop_if_empty();
            segments.push("wallet");
            segments.push(&config.wallet_name);
        }
        let client = AgentBuilder::new()
            .timeout_connect(config.timeout)
            .timeout_read(config.timeout)
            .timeout_write(config.timeout)
            .build();
        // The page reads the same mempool the plant reads. The plant waits
        // `config.timeout` (30 s). The page used to stop at 2 s and print
        // "not read · node not reachable" while the node was answering.
        let quote_budget = config.timeout.max(QUOTE_READ_BUDGET);
        let quote_client = AgentBuilder::new()
            .timeout_connect(quote_budget)
            .timeout_read(quote_budget)
            .timeout_write(quote_budget)
            .build();
        Ok(Self {
            client,
            node_url: config.rpc_url,
            wallet_url,
            auth: config.auth,
            expected_network: config.expected_network,
            fee_policy,
            utxo_policy: config.utxo_policy,
            utxo_cursor: Arc::new(AtomicU64::new(0)),
            quote_client,
        })
    }

    pub fn fee_log_label(&self) -> String {
        fee::fee_log_label(&self.fee_policy)
    }

    fn header_height(&self, display_hash: &str) -> Result<u32, ChainError> {
        let header = self.node_call("getblockheader", json!([display_hash, true]))?;
        u32::try_from(required_u64(&header, "height")?)
            .map_err(|_| ChainError("block height exceeds u32".to_owned()))
    }

    fn node_call(&self, method: &str, params: Value) -> Result<Value, ChainError> {
        self.call(&self.client, &self.node_url, method, params)
    }

    fn wallet_call(&self, method: &str, params: Value) -> Result<Value, ChainError> {
        self.call(&self.client, &self.wallet_url, method, params)
    }

    /// Same RPC, on the page's read budget. The plant client is not used.
    fn quote_node_call(&self, method: &str, params: Value) -> Result<Value, ChainError> {
        self.call(&self.quote_client, &self.node_url, method, params)
    }

    fn quote_wallet_call(&self, method: &str, params: Value) -> Result<Value, ChainError> {
        self.call(&self.quote_client, &self.wallet_url, method, params)
    }

    fn call(
        &self,
        client: &Agent,
        url: &Url,
        method: &str,
        params: Value,
    ) -> Result<Value, ChainError> {
        let authorization = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", self.auth.username, self.auth.password));
        let response = client
            .post(url.as_str())
            .set("Authorization", &format!("Basic {authorization}"))
            .send_json(json!({
                "jsonrpc": "2.0",
                "id": concat!("bari-v", env!("CARGO_PKG_VERSION")),
                "method": method,
                "params": params,
            }));
        let response = match response {
            Ok(response) => response,
            Err(ureq::Error::Status(_, response)) => response,
            Err(ureq::Error::Transport(error)) => {
                return Err(ChainError(format!("{method} transport error: {error}")))
            }
        };
        let status = response.status();
        let envelope: RpcEnvelope = response
            .into_json()
            .map_err(|error| ChainError(format!("{method} invalid JSON ({status}): {error}")))?;
        if let Some(error) = envelope.error {
            return Err(ChainError(format!(
                "{method} RPC error {}: {}",
                error.code, error.message
            )));
        }
        envelope
            .result
            .ok_or_else(|| ChainError(format!("{method} returned neither result nor error")))
    }

    fn defer_fee(
        reason: &str,
        message: String,
        needed_sat_vb: Option<f64>,
        next_block_min_sat_vb: Option<f64>,
        node_estimate_sat_vb: Option<f64>,
        cap_sat_vb: f64,
        decision_height: Option<u32>,
    ) -> PlantError {
        PlantError::defer(Deferral {
            reason: reason.to_owned(),
            message,
            decision_height,
            next_block_min_sat_vb,
            node_estimate_sat_vb,
            needed_sat_vb,
            cap_sat_vb: Some(cap_sat_vb),
        })
    }

    /// Mempool next-block minimum, plus the acceptance floors.
    /// `estimatesmartfee` is a reference. Its failure does not fail this view.
    /// Recent blocks are not read.
    ///
    /// `getrawmempool true` is parsed as one JSON value. A live mempool is
    /// about 57k transactions. The plant and the operator page both read that
    /// body. The page uses [`Self::quote_node_call`], whose budget is at least
    /// [`QUOTE_READ_BUDGET`], so a slow mempool does not become "node not reachable".
    fn node_fee_view_with(
        &self,
        node: impl Fn(&str, Value) -> Result<Value, ChainError>,
    ) -> Result<NodeFeeView, ChainError> {
        let mempool = node("getmempoolinfo", json!([]))?;
        let (relay_min_sat_vb, mempool_min_sat_vb) =
            fee::parse_mempool_fees(&mempool).map_err(ChainError)?;
        let raw = node("getrawmempool", json!([true]))?;
        let entries = fee::parse_raw_mempool(&raw).map_err(ChainError)?;
        let next_block_min = fee::next_block_min_from_mempool(&entries, relay_min_sat_vb);
        Ok(NodeFeeView {
            estimate_sat_vb: Some(next_block_min),
            node_estimate_sat_vb: Self::node_estimate_reference_with(&node),
            recent_min_sat_vb: None,
            relay_min_sat_vb,
            mempool_min_sat_vb,
        })
    }

    fn node_fee_view(&self) -> Result<NodeFeeView, ChainError> {
        self.node_fee_view_with(|method, params| self.node_call(method, params))
    }

    /// `estimatesmartfee` 1 economical. Logged for the operator. Not the gate.
    fn node_estimate_reference_with(
        node: &impl Fn(&str, Value) -> Result<Value, ChainError>,
    ) -> Option<f64> {
        let estimated = match node(
            "estimatesmartfee",
            json!([fee::FEE_ESTIMATE_CONF_TARGET, fee::FEE_ESTIMATE_MODE]),
        ) {
            Ok(estimated) => estimated,
            Err(error) => {
                ops::warn(
                    ops::FEE_ESTIMATE,
                    json!({
                        "message": format!(
                            "estimatesmartfee failed: {error}; the mempool minimum still gates"
                        ),
                    }),
                );
                return None;
            }
        };
        match fee::parse_estimatesmartfee_sat_vb(&estimated) {
            Ok(rate) => rate,
            Err(error) => {
                ops::warn(
                    ops::FEE_ESTIMATE,
                    json!({
                        "message": format!(
                            "estimatesmartfee: {error}; the mempool minimum still gates"
                        ),
                    }),
                );
                None
            }
        }
    }

    #[allow(clippy::result_large_err)]
    fn decide_fee(&self) -> Result<(FeeDecision, Option<f64>, f64), PlantError> {
        match self.fee_policy {
            FeePolicy::Fixed {
                rate_sat_vb,
                source,
                cap_sat_vb,
            } => Ok((
                FeeDecision::Broadcast {
                    rate_sat_vb,
                    source,
                    cap_sat_vb,
                    next_block_min_sat_vb: rate_sat_vb,
                },
                None,
                0.0,
            )),
            FeePolicy::Dynamic {
                cap_sat_vb,
                multiplier,
            } => {
                let view = self.node_fee_view().map_err(|error| {
                    Self::defer_fee(
                        "fee-view",
                        format!(
                            "fee view failed: {error}; holding; no tree; no psbt; not replaced"
                        ),
                        None,
                        None,
                        None,
                        cap_sat_vb,
                        None,
                    )
                })?;
                let estimate = view.node_estimate_sat_vb;
                let relay = view.relay_min_sat_vb;
                let decision = fee::select_bid(&view, cap_sat_vb, multiplier).map_err(|error| {
                    Self::defer_fee(
                        "fee-view",
                        format!("{error}; holding; no tree; no psbt; not replaced"),
                        None,
                        None,
                        estimate,
                        cap_sat_vb,
                        None,
                    )
                })?;
                Ok((decision, estimate, relay))
            }
        }
    }

    /// Choose the rate, then fund once. At or above the cap, return before any broadcast.
    #[allow(clippy::result_large_err)]
    fn create_and_broadcast(&self, root: &Hash32) -> Result<PlantReceipt, PlantError> {
        let (decision, node_estimate, relay) = self.decide_fee()?;
        let (rate_sat_vb, source, cap_sat_vb) = match decision {
            FeeDecision::Hold {
                next_block_min_sat_vb,
                cap_sat_vb,
                ..
            } => {
                return Err(Self::defer_fee(
                    fee::HOLD_REASON_NEXT_BLOCK_MIN,
                    fee::hold_message(next_block_min_sat_vb, cap_sat_vb),
                    Some(fee::bid_at(
                        next_block_min_sat_vb,
                        cap_sat_vb,
                        fee::DEFAULT_FEE_MULTIPLIER,
                        relay,
                    )),
                    Some(next_block_min_sat_vb),
                    node_estimate,
                    cap_sat_vb,
                    None,
                ));
            }
            FeeDecision::NoEstimate { cap_sat_vb } => {
                return Err(Self::defer_fee(
                    fee::HOLD_REASON_NO_ESTIMATE,
                    fee::no_estimate_hold_message(cap_sat_vb),
                    None,
                    None,
                    node_estimate,
                    cap_sat_vb,
                    None,
                ));
            }
            FeeDecision::FallbackSkip { cap_sat_vb } => {
                return Err(Self::defer_fee(
                    fee::HOLD_REASON_FALLBACK,
                    fee::fallback_hold_message(cap_sat_vb),
                    None,
                    None,
                    node_estimate,
                    cap_sat_vb,
                    None,
                ));
            }
            FeeDecision::Broadcast {
                rate_sat_vb,
                source,
                cap_sat_vb,
                ..
            } => (rate_sat_vb, source, Some(cap_sat_vb)),
        };
        self.fund_and_broadcast(root, rate_sat_vb, source, cap_sat_vb, None)
    }

    /// Fund once at a rate the gate already accepted. Never broadcasts above 1.1.
    #[allow(clippy::result_large_err)]
    fn fund_and_broadcast(
        &self,
        root: &Hash32,
        rate_sat_vb: f64,
        source: &str,
        cap_sat_vb: Option<f64>,
        decision_height: Option<u32>,
    ) -> Result<PlantReceipt, PlantError> {
        let ceiling = cap_sat_vb
            .unwrap_or(fee::DEFAULT_FEE_CAP_SAT_VB)
            .min(fee::DEFAULT_FEE_CAP_SAT_VB);
        if fee::rate_exceeds_cap(rate_sat_vb, ceiling) {
            return Err(Self::defer_fee(
                fee::DEFER_REASON_CAP,
                fee::floor_hold_message(rate_sat_vb, ceiling),
                Some(rate_sat_vb),
                None,
                None,
                ceiling,
                decision_height,
            ));
        }
        let fail = |message: String| {
            let reason = plant_skip_reason(&message);
            if reason == SkipReason::Fee {
                PlantError::defer(Deferral {
                    reason: fee::DEFER_REASON_NO_CONFIRMED_UTXO.to_owned(),
                    message: format!("{message}; deferred; not replaced"),
                    decision_height,
                    next_block_min_sat_vb: None,
                    node_estimate_sat_vb: None,
                    needed_sat_vb: Some(rate_sat_vb),
                    cap_sat_vb: Some(ceiling),
                })
            } else {
                PlantError::with_fee(reason, message, Some(rate_sat_vb), Some(source.to_owned()))
            }
        };
        let payload = format!("4241524902{}", hex::encode(root));
        let listed = self
            .wallet_call(
                "listunspent",
                json!([self.utxo_policy.min_conf, 9999999, [], false]),
            )
            .map_err(|error| fail(error.to_string()))?;
        let utxos = utxo::parse_listunspent(&listed)
            .map_err(|error| fail(format!("listunspent: {error}")))?;
        let cursor = self.utxo_cursor.load(Ordering::Relaxed);
        let plan = match utxo::select_plant_inputs(&utxos, &self.utxo_policy, rate_sat_vb, cursor) {
            Ok(plan) => plan,
            Err(error) => {
                return Err(PlantError::defer(Deferral {
                    reason: fee::DEFER_REASON_NO_CONFIRMED_UTXO.to_owned(),
                    message: error.defer_message(),
                    decision_height,
                    next_block_min_sat_vb: None,
                    node_estimate_sat_vb: None,
                    needed_sat_vb: Some(rate_sat_vb),
                    cap_sat_vb: Some(ceiling),
                }));
            }
        };
        self.utxo_cursor.store(plan.next_cursor, Ordering::Relaxed);
        note_utxo_plan(&self.utxo_policy, &plan);
        let inputs = plan
            .inputs
            .iter()
            .map(|input| json!({"txid": input.txid, "vout": input.vout}))
            .collect::<Vec<_>>();
        let options = funding_options(rate_sat_vb, self.utxo_policy.min_conf);
        let funded = self
            .wallet_call(
                "walletcreatefundedpsbt",
                json!([inputs, [{"data": payload}], 0, options, true]),
            )
            .map_err(|error| fail(error.to_string()))?;
        let psbt = required_str(&funded, "psbt").map_err(|error| fail(error.to_string()))?;
        let fee_btc = required_f64(&funded, "fee").map_err(|error| fail(error.to_string()))?;

        let processed = self
            .wallet_call(
                "walletprocesspsbt",
                json!([psbt, true, "DEFAULT", true, true]),
            )
            .map_err(|error| fail(error.to_string()))?;
        if processed.get("complete").and_then(Value::as_bool) != Some(true) {
            return Err(fail(
                "walletprocesspsbt did not produce a complete transaction".to_owned(),
            ));
        }
        let raw_hex = required_str(&processed, "hex").map_err(|error| fail(error.to_string()))?;
        let decoded = self
            .node_call("decoderawtransaction", json!([raw_hex]))
            .map_err(|error| fail(error.to_string()))?;
        let vsize = required_u64(&decoded, "vsize").map_err(|error| fail(error.to_string()))?;
        if vsize == 0 {
            return Err(fail("decoded transaction has zero vsize".to_owned()));
        }
        let actual_sats = fee::sats_from_btc(fee_btc).ok_or_else(|| {
            Self::defer_fee(
                "underfunded",
                "funded amount could not be read as sats; deferred; no broadcast; not replaced"
                    .to_owned(),
                Some(rate_sat_vb),
                None,
                None,
                ceiling,
                decision_height,
            )
        })?;
        let expected_sats = fee::plant_fee_sats(rate_sat_vb, vsize).ok_or_else(|| {
            Self::defer_fee(
                fee::DEFER_REASON_CAP,
                format!(
                    "bid could not be converted for {vsize} vB; deferred; not broadcast; not replaced"
                ),
                Some(rate_sat_vb),
                None,
                None,
                ceiling,
                decision_height,
            )
        })?;
        if actual_sats < 0 || (actual_sats as u64) < expected_sats {
            return Err(Self::defer_fee(
                "underfunded",
                format!(
                    "funded amount {actual_sats} sats is below the {expected_sats} sat bid for {vsize} vB; deferred; not broadcast; not replaced"
                ),
                Some(rate_sat_vb),
                None,
                None,
                ceiling,
                decision_height,
            ));
        }
        let expected_script = hex::encode(commitment_script(root));
        let vout = find_exact_zero_value_output(&decoded, &expected_script)
            .map_err(|error| fail(error.to_string()))?;
        let decoded_txid =
            required_str(&decoded, "txid").map_err(|error| fail(error.to_string()))?;
        let broadcast_txid = self
            .node_call("sendrawtransaction", json!([raw_hex]))
            .map_err(|error| fail(error.to_string()))?;
        let broadcast_txid = broadcast_txid
            .as_str()
            .ok_or_else(|| fail("sendrawtransaction returned a non-string txid".to_owned()))?;
        if broadcast_txid != decoded_txid {
            return Err(fail(
                "broadcast txid differs from the decoded transaction id".to_owned(),
            ));
        }
        let txid =
            natural_from_display_hex(broadcast_txid).map_err(|error| fail(error.to_string()))?;
        Ok(PlantReceipt {
            txid,
            vout,
            fee_rate_sat_vb: rate_sat_vb,
            fee_source: source.to_owned(),
            fee_cap_sat_vb: Some(ceiling),
            fee_sat: u64::try_from(actual_sats).ok(),
            vsize: u32::try_from(vsize).ok(),
        })
    }

    /// Confirmed-input preview. Does not advance the cursor and does not build a PSBT.
    /// `None` means an input is available. `Some` means wait in the pool.
    fn input_deferral(
        &self,
        rate_sat_vb: f64,
        cap_sat_vb: Option<f64>,
        decision_height: Option<u32>,
    ) -> Option<Deferral> {
        let listed = match self.wallet_call(
            "listunspent",
            json!([self.utxo_policy.min_conf, 9999999, [], false]),
        ) {
            Ok(listed) => listed,
            Err(error) => {
                return Some(Deferral {
                    reason: "utxo-view".to_owned(),
                    message: format!(
                        "listunspent failed: {error}; holding; no tree; no psbt; not replaced"
                    ),
                    decision_height,
                    next_block_min_sat_vb: None,
                    node_estimate_sat_vb: None,
                    needed_sat_vb: Some(rate_sat_vb),
                    cap_sat_vb,
                });
            }
        };
        let utxos = match utxo::parse_listunspent(&listed) {
            Ok(utxos) => utxos,
            Err(error) => {
                return Some(Deferral {
                    reason: "utxo-view".to_owned(),
                    message: format!(
                        "listunspent: {error}; holding; no tree; no psbt; not replaced"
                    ),
                    decision_height,
                    next_block_min_sat_vb: None,
                    node_estimate_sat_vb: None,
                    needed_sat_vb: Some(rate_sat_vb),
                    cap_sat_vb,
                });
            }
        };
        let cursor = self.utxo_cursor.load(Ordering::Relaxed);
        match utxo::select_plant_inputs(&utxos, &self.utxo_policy, rate_sat_vb, cursor) {
            Ok(_) => None,
            Err(error) => Some(Deferral {
                reason: fee::DEFER_REASON_NO_CONFIRMED_UTXO.to_owned(),
                message: error.defer_message(),
                decision_height,
                next_block_min_sat_vb: None,
                node_estimate_sat_vb: None,
                needed_sat_vb: Some(rate_sat_vb),
                cap_sat_vb,
            }),
        }
    }

    fn dynamic_seal_clearance(
        &self,
        cap_sat_vb: f64,
        multiplier: f64,
        decision_height: u32,
    ) -> Result<SealClearance, ChainError> {
        let view = match self.node_fee_view() {
            Ok(view) => view,
            Err(error) => {
                return Ok(SealClearance::Defer(Deferral {
                    reason: "fee-view".to_owned(),
                    message: format!(
                        "fee view failed: {error}; holding; no tree; no psbt; not replaced"
                    ),
                    decision_height: Some(decision_height),
                    next_block_min_sat_vb: None,
                    node_estimate_sat_vb: None,
                    needed_sat_vb: None,
                    cap_sat_vb: Some(cap_sat_vb),
                }));
            }
        };
        let pre =
            fee::pre_broadcast_with_floor(&view, cap_sat_vb, multiplier).map_err(ChainError)?;
        if matches!(pre, fee::PreBroadcast::Hold { .. }) {
            return Ok(clearance_after_fee(pre, Ok(()), Some(decision_height)));
        }
        let rate = match &pre {
            fee::PreBroadcast::Broadcast { rate_sat_vb, .. } => *rate_sat_vb,
            fee::PreBroadcast::Hold { .. } => unreachable!("hold returned above"),
        };
        if let Some(mut deferral) =
            self.input_deferral(rate, Some(cap_sat_vb), Some(decision_height))
        {
            deferral.node_estimate_sat_vb = view.node_estimate_sat_vb;
            return Ok(SealClearance::Defer(deferral));
        }
        Ok(clearance_after_fee(pre, Ok(()), Some(decision_height)))
    }
}

/// Fee gate, then the selector. A next-block hold does not consult the selector.
/// `input` is `Ok` when a confirmed input can fund the bid.
pub(crate) fn clearance_after_fee(
    pre: fee::PreBroadcast,
    input: Result<(), SelectError>,
    decision_height: Option<u32>,
) -> SealClearance {
    match pre {
        fee::PreBroadcast::Hold {
            reason,
            message,
            next_block_min_sat_vb,
            node_estimate_sat_vb,
            bid_sat_vb,
            cap_sat_vb,
        } => SealClearance::Defer(Deferral {
            reason: reason.to_owned(),
            message,
            decision_height,
            next_block_min_sat_vb,
            node_estimate_sat_vb,
            needed_sat_vb: bid_sat_vb,
            cap_sat_vb: Some(cap_sat_vb),
        }),
        fee::PreBroadcast::Broadcast {
            rate_sat_vb,
            source,
            cap_sat_vb,
            next_block_min_sat_vb,
            node_estimate_sat_vb,
        } => match input {
            Err(error) => SealClearance::Defer(Deferral {
                reason: fee::DEFER_REASON_NO_CONFIRMED_UTXO.to_owned(),
                message: error.defer_message(),
                decision_height,
                next_block_min_sat_vb: None,
                node_estimate_sat_vb,
                needed_sat_vb: Some(rate_sat_vb),
                cap_sat_vb: Some(cap_sat_vb),
            }),
            Ok(()) => SealClearance::PlantAt {
                rate_sat_vb,
                source: source.to_owned(),
                cap_sat_vb: Some(cap_sat_vb),
                next_block_min_sat_vb: Some(next_block_min_sat_vb),
                node_estimate_sat_vb,
            },
        },
    }
}

impl ChainBackend for BitcoinRpc {
    fn network(&self) -> Result<String, ChainError> {
        let info = self.node_call("getblockchaininfo", json!([]))?;
        Ok(required_str(&info, "chain")?.to_owned())
    }

    fn initial_block_download(&self) -> Result<bool, ChainError> {
        let info = self.node_call("getblockchaininfo", json!([]))?;
        info.get("initialblockdownload")
            .and_then(Value::as_bool)
            .ok_or_else(|| {
                ChainError("getblockchaininfo omitted boolean initialblockdownload".to_owned())
            })
    }

    fn best_tip(&self) -> Result<ChainTip, ChainError> {
        let hash = self.node_call("getbestblockhash", json!([]))?;
        let display_hash = hash
            .as_str()
            .ok_or_else(|| ChainError("getbestblockhash returned a non-string".to_owned()))?;
        let header = self.node_call("getblockheader", json!([display_hash, true]))?;
        let height = u32::try_from(required_u64(&header, "height")?)
            .map_err(|_| ChainError("block height exceeds u32".to_owned()))?;
        let previous_hash = header
            .get("previousblockhash")
            .and_then(Value::as_str)
            .map(natural_from_display_hex)
            .transpose()?;
        let time = header
            .get("time")
            .and_then(Value::as_u64)
            .and_then(ntime_seconds);
        Ok(ChainTip {
            hash: natural_from_display_hex(display_hash)?,
            previous_hash,
            height,
            time,
        })
    }

    fn plant(&self, root: &Hash32) -> Result<PlantReceipt, PlantError> {
        self.create_and_broadcast(root)
    }

    fn seal_clearance(&self, decision: &ChainTip) -> Result<SealClearance, ChainError> {
        match self.fee_policy {
            FeePolicy::Fixed {
                rate_sat_vb,
                source,
                cap_sat_vb,
            } => {
                let cap = Some(cap_sat_vb);
                if let Some(deferral) = self.input_deferral(rate_sat_vb, cap, Some(decision.height))
                {
                    Ok(SealClearance::Defer(deferral))
                } else {
                    Ok(SealClearance::PlantAt {
                        rate_sat_vb,
                        source: source.to_owned(),
                        cap_sat_vb: cap,
                        next_block_min_sat_vb: None,
                        node_estimate_sat_vb: None,
                    })
                }
            }
            FeePolicy::Dynamic {
                cap_sat_vb,
                multiplier,
            } => self.dynamic_seal_clearance(cap_sat_vb, multiplier, decision.height),
        }
    }

    fn plant_cleared(
        &self,
        root: &Hash32,
        clearance: &SealClearance,
    ) -> Result<PlantReceipt, PlantError> {
        match clearance {
            SealClearance::Defer(deferral) => Err(PlantError::defer(deferral.clone())),
            SealClearance::PlantAt {
                rate_sat_vb,
                source,
                cap_sat_vb,
                ..
            } => self.fund_and_broadcast(root, *rate_sat_vb, source, *cap_sat_vb, None),
            SealClearance::Clear => self.plant(root),
        }
    }

    fn included_in_tip(
        &self,
        txid: &Hash32,
        tip: &ChainTip,
    ) -> Result<Option<StoredInclusion>, ChainError> {
        let display_block = display_hex_from_natural(&tip.hash);
        let display_txid = display_hex_from_natural(txid);
        let block = self.node_call("getblock", json!([display_block, 1]))?;
        if required_i64(&block, "confirmations")? < 1 {
            return Ok(None);
        }
        let found = block
            .get("tx")
            .and_then(Value::as_array)
            .ok_or_else(|| ChainError("getblock did not return a tx array".to_owned()))?
            .iter()
            .any(|candidate| candidate.as_str() == Some(&display_txid));
        Ok(found.then_some(StoredInclusion {
            block_hash: tip.hash,
            block_height: tip.height,
        }))
    }

    fn block_is_selected(&self, block_hash: &Hash32) -> Result<bool, ChainError> {
        let header = self.node_call(
            "getblockheader",
            json!([display_hex_from_natural(block_hash), true]),
        )?;
        Ok(required_i64(&header, "confirmations")? >= 0)
    }

    fn post_check(&self, body: &CanonicalPathBody) -> Result<ChainPostCheck, ChainError> {
        let actual_network = self.network()?;
        if actual_network != self.expected_network {
            return Err(ChainError(format!(
                "ChainView expected {}, connected node reports {actual_network}",
                self.expected_network
            )));
        }
        let display_txid = display_hex_from_natural(&body.txid);
        let display_block = display_hex_from_natural(&body.block_hash);
        let transaction =
            self.node_call("getrawtransaction", json!([display_txid, 1, display_block]))?;
        let transaction_in_block = transaction
            .get("txid")
            .and_then(Value::as_str)
            .is_some_and(|value| value == display_txid)
            && transaction
                .get("blockhash")
                .and_then(Value::as_str)
                .is_some_and(|value| value == display_block)
            && transaction.get("in_active_chain").and_then(Value::as_bool) == Some(true);

        let output = transaction
            .get("vout")
            .and_then(Value::as_array)
            .and_then(|outputs| {
                outputs.iter().find(|output| {
                    output.get("n").and_then(Value::as_u64) == Some(u64::from(body.vout))
                })
            });
        let output_exists = output.is_some();
        let zero_value = output
            .and_then(|output| output.get("value"))
            .and_then(Value::as_f64)
            == Some(0.0);
        let expected_script = hex::encode(commitment_script(&body.root));
        let exact_script = output
            .and_then(|output| output.get("scriptPubKey"))
            .and_then(|script| script.get("hex"))
            .and_then(Value::as_str)
            == Some(expected_script.as_str());

        let header = self.node_call("getblockheader", json!([display_block, true]))?;
        let selected_chain = required_i64(&header, "confirmations")? >= 0;
        let authenticated_height = if selected_chain {
            Some(
                u32::try_from(required_u64(&header, "height")?)
                    .map_err(|_| ChainError("authenticated height exceeds u32".to_owned()))?,
            )
        } else {
            None
        };
        Ok(ChainPostCheck {
            transaction_in_block,
            output_exists,
            zero_value,
            exact_script,
            selected_chain,
            carried_height: body.block_height,
            authenticated_height,
            height_matches: authenticated_height.map(|height| height == body.block_height),
        })
    }

    fn tx_place(&self, txid: &Hash32) -> Result<Option<TxPlace>, ChainError> {
        let display = display_hex_from_natural(txid);
        let tx = match self.node_call("getrawtransaction", json!([display, 2])) {
            Ok(tx) => tx,
            Err(error) if missing_tx(&error) => return Ok(Some(TxPlace::absent())),
            Err(_) => match self.node_call("getrawtransaction", json!([display, true])) {
                Ok(tx) => tx,
                Err(error) if missing_tx(&error) => return Ok(Some(TxPlace::absent())),
                Err(error) => return Err(error),
            },
        };
        if let Some(hash) = tx.get("blockhash").and_then(Value::as_str) {
            let height = self.header_height(hash)?;
            let block_hash = natural_from_display_hex(hash).ok();
            return Ok(Some(TxPlace {
                inclusion_height: Some(height),
                block_hash,
                in_mempool: false,
                fee_sats: fee_sats_of(&tx),
            }));
        }
        Ok(Some(TxPlace {
            inclusion_height: None,
            block_hash: None,
            in_mempool: true,
            fee_sats: fee_sats_of(&tx),
        }))
    }

    fn chain_wallet(&self) -> Result<Option<ChainWallet>, ChainError> {
        let listed = self.quote_wallet_call("listunspent", json!([0, 9999999]))?;
        parse_chain_wallet(&listed, &self.utxo_policy)
            .map(Some)
            .map_err(ChainError)
    }

    fn chain_sync(&self) -> Result<Option<ChainSync>, ChainError> {
        let info = self.quote_node_call("getblockchaininfo", json!([]))?;
        let blocks = u32::try_from(required_u64(&info, "blocks")?)
            .map_err(|_| ChainError("block height exceeds u32".to_owned()))?;
        let headers = u32::try_from(required_u64(&info, "headers")?)
            .map_err(|_| ChainError("header height exceeds u32".to_owned()))?;
        let initialblockdownload = info
            .get("initialblockdownload")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Ok(Some(ChainSync {
            blocks,
            headers,
            initialblockdownload,
        }))
    }

    fn fee_observation(&self) -> Result<Option<f64>, ChainError> {
        Ok(self.node_fee_view()?.estimate_sat_vb)
    }

    fn fee_quote(&self) -> Result<Option<FeeQuote>, ChainError> {
        let (cap, multiplier, fixed) = match self.fee_policy {
            FeePolicy::Dynamic {
                cap_sat_vb,
                multiplier,
            } => (cap_sat_vb, multiplier, None),
            FeePolicy::Fixed {
                rate_sat_vb,
                cap_sat_vb,
                ..
            } => (cap_sat_vb, fee::DEFAULT_FEE_MULTIPLIER, Some(rate_sat_vb)),
        };
        if let Some(rate) = fixed {
            return Ok(Some(FeeQuote {
                min_sat_vb: None,
                node_estimate_sat_vb: None,
                bid_sat_vb: Some(rate),
                floor_sat_vb: None,
                source: None,
                hold_reason: String::new(),
            }));
        }
        // The page read. A plant uses `node_fee_view` on the plant client.
        let view =
            self.node_fee_view_with(|method, params| self.quote_node_call(method, params))?;
        let pre = fee::pre_broadcast_with_floor(&view, cap, multiplier).map_err(ChainError)?;
        Ok(Some(match pre {
            fee::PreBroadcast::Broadcast {
                rate_sat_vb,
                next_block_min_sat_vb,
                node_estimate_sat_vb,
                source,
                ..
            } => FeeQuote {
                min_sat_vb: Some(next_block_min_sat_vb),
                node_estimate_sat_vb,
                bid_sat_vb: Some(rate_sat_vb),
                floor_sat_vb: Some(view.relay_min_sat_vb.max(view.mempool_min_sat_vb)),
                source: Some(source.to_owned()),
                hold_reason: String::new(),
            },
            fee::PreBroadcast::Hold {
                message,
                next_block_min_sat_vb,
                node_estimate_sat_vb,
                bid_sat_vb,
                ..
            } => FeeQuote {
                min_sat_vb: next_block_min_sat_vb,
                node_estimate_sat_vb,
                bid_sat_vb,
                floor_sat_vb: Some(view.relay_min_sat_vb.max(view.mempool_min_sat_vb)),
                source: None,
                hold_reason: message,
            },
        }))
    }
}

fn missing_tx(error: &ChainError) -> bool {
    let text = error.to_string();
    text.contains("No such mempool") || text.contains("error -5")
}

fn fee_sats_of(tx: &Value) -> Option<u64> {
    let btc = tx.get("fee").and_then(fee::json_f64)?;
    let sats = fee::sats_from_btc(btc)?;
    u64::try_from(sats).ok()
}

/// `listunspent` only. Confirmed outputs have at least `policy.min_conf`
/// confirmations. The rest are unconfirmed. The two sets do not overlap.
/// Addresses are not read.
pub(crate) fn parse_chain_wallet(
    unspent: &Value,
    policy: &UtxoPolicy,
) -> Result<ChainWallet, String> {
    let utxos = utxo::parse_listunspent(unspent)?;
    let mut confirmed_sat = 0u64;
    let mut unconfirmed_sat = 0u64;
    let mut confirmed_coins = 0u32;
    let mut unconfirmed_coins = 0u32;
    let mut coins = Vec::with_capacity(utxos.len());
    for utxo in &utxos {
        if utxo.confirmations >= policy.min_conf {
            confirmed_sat = confirmed_sat.saturating_add(utxo.amount_sat);
            confirmed_coins = confirmed_coins.saturating_add(1);
        } else {
            unconfirmed_sat = unconfirmed_sat.saturating_add(utxo.amount_sat);
            unconfirmed_coins = unconfirmed_coins.saturating_add(1);
        }
        coins.push(WalletCoin {
            amount_sat: utxo.amount_sat,
            confirmations: utxo.confirmations,
            spendable: utxo.spendable,
            safe: utxo.safe,
        });
    }
    Ok(ChainWallet {
        confirmed_sat,
        unconfirmed_sat,
        confirmed_coins,
        unconfirmed_coins,
        coins,
        min_conf: policy.min_conf,
        reserve_min: policy.reserve_min,
        reserve_max: policy.reserve_max,
        consolidate: policy.consolidate,
    })
}

fn funding_options(fee_rate_sat_vb: f64, min_conf: u64) -> Value {
    json!({
        "add_inputs": false,
        "include_unsafe": false,
        "lockUnspents": false,
        "minconf": min_conf,
        "replaceable": false,
        "fee_rate": fee::format_sat_vb(fee_rate_sat_vb),
    })
}

fn note_utxo_plan(policy: &UtxoPolicy, plan: &PlantInputPlan) {
    let inputs = plan
        .inputs
        .iter()
        .map(|input| {
            json!({
                "txid": input.txid,
                "vout": input.vout,
                "amount_sat": input.amount_sat,
                "confirmations": input.confirmations,
            })
        })
        .collect::<Vec<_>>();
    let fields = json!({
        "mode": plan.mode.as_str(),
        "min_conf": policy.min_conf,
        "reserve_min": policy.reserve_min,
        "reserve_max": policy.reserve_max,
        "confirmed": plan.confirmed,
        "thin": plan.thin,
        "consolidate": policy.consolidate,
        "inputs": inputs,
    });
    if plan.thin {
        ops::warn(ops::PLANT_UTXO, fields);
    } else {
        ops::info(ops::PLANT_UTXO, fields);
    }
}

/// Fee-skip stays the existing miss. A message that names a fee, missing
/// funds, or no available coin is `fee`. Anything else stays `other`.
/// This does not add a replace, bump, or repair path.
pub(crate) fn plant_skip_reason(message: &str) -> SkipReason {
    let lowered = message.to_ascii_lowercase();
    if lowered.contains("fee")
        || lowered.contains("insufficient funds")
        || lowered.contains("not enough funds")
        || lowered.contains("no available")
    {
        SkipReason::Fee
    } else {
        SkipReason::Other
    }
}

fn find_exact_zero_value_output(decoded: &Value, expected_script: &str) -> Result<u32, ChainError> {
    let outputs = decoded
        .get("vout")
        .and_then(Value::as_array)
        .ok_or_else(|| ChainError("decoderawtransaction did not return vout".to_owned()))?;
    let mut matches = outputs.iter().filter(|output| {
        output.get("value").and_then(Value::as_f64) == Some(0.0)
            && output
                .get("scriptPubKey")
                .and_then(|script| script.get("hex"))
                .and_then(Value::as_str)
                == Some(expected_script)
    });
    let found = matches.next().ok_or_else(|| {
        ChainError("funded transaction lacks the exact zero-value BARI output".to_owned())
    })?;
    if matches.next().is_some() {
        return Err(ChainError(
            "funded transaction contains the BARI output more than once".to_owned(),
        ));
    }
    let number = required_u64(found, "n")?;
    u32::try_from(number).map_err(|_| ChainError("vout exceeds u32".to_owned()))
}

pub fn natural_from_display_hex(display: &str) -> Result<Hash32, ChainError> {
    if display.len() != 64 {
        return Err(ChainError(
            "Bitcoin RPC hash must contain exactly 64 hexadecimal characters".to_owned(),
        ));
    }
    let mut bytes = hex::decode(display)
        .map_err(|error| ChainError(format!("Bitcoin RPC hash is invalid hex: {error}")))?;
    bytes.reverse();
    bytes
        .try_into()
        .map_err(|_| ChainError("Bitcoin RPC hash did not decode to 32 bytes".to_owned()))
}

pub fn display_hex_from_natural(natural: &Hash32) -> String {
    crate::encode_bitcoin_id(natural)
}

/// Section 9 range for Bitcoin `nTime`: `0 … 2^32 − 1` unix seconds.
pub(crate) fn ntime_seconds(secs: u64) -> Option<u64> {
    u32::try_from(secs).ok().map(u64::from)
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, ChainError> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| {
        ChainError(format!(
            "RPC result field {field} is missing or not a string"
        ))
    })
}

fn required_u64(value: &Value, field: &str) -> Result<u64, ChainError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| ChainError(format!("RPC result field {field} is missing or not a u64")))
}

fn required_i64(value: &Value, field: &str) -> Result<i64, ChainError> {
    value
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| ChainError(format!("RPC result field {field} is missing or not an i64")))
}

fn required_f64(value: &Value, field: &str) -> Result<f64, ChainError> {
    value.get(field).and_then(Value::as_f64).ok_or_else(|| {
        ChainError(format!(
            "RPC result field {field} is missing or not numeric"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ntime_seconds_keeps_the_section_9_range() {
        assert_eq!(ntime_seconds(1_720_000_000), Some(1_720_000_000));
        assert_eq!(ntime_seconds(0), Some(0));
        assert_eq!(ntime_seconds(4_294_967_295), Some(4_294_967_295));
        assert_eq!(ntime_seconds(4_294_967_296), None);
    }

    #[test]
    fn rpc_display_order_and_natural_body_order_round_trip() {
        let natural: Hash32 = std::array::from_fn(|index| index as u8);
        let display = display_hex_from_natural(&natural);
        assert_eq!(
            display,
            "1f1e1d1c1b1a191817161514131211100f0e0d0c0b0a09080706050403020100"
        );
        assert_eq!(natural_from_display_hex(&display).unwrap(), natural);
    }

    #[test]
    fn a_plant_is_not_replaceable_and_there_is_no_fee_bump() {
        assert_eq!(funding_options(1.0, 1)["replaceable"], false);
        let source = include_str!("bitcoin_rpc.rs");
        let bump = format!("{}{}", "bump", "fee");
        let priority = format!("{}{}", "prioritise", "transaction");
        let priority_us = format!("{}{}", "prioritize", "transaction");
        assert!(!source.contains(&format!("\"{bump}\"")));
        assert!(!source.contains(&format!("{bump}(")));
        assert!(!source.to_ascii_lowercase().contains(&priority));
        assert!(!source.to_ascii_lowercase().contains(&priority_us));
        assert!(source.contains("\"replaceable\": false"));
    }

    #[test]
    fn funding_options_use_the_already_chosen_rate() {
        assert_eq!(
            funding_options(7.5, 1),
            json!({
                "add_inputs": false,
                "include_unsafe": false,
                "lockUnspents": false,
                "minconf": 1,
                "replaceable": false,
                "fee_rate": "7.5"
            })
        );
        assert_eq!(funding_options(0.6, 1)["fee_rate"], "0.6");
    }

    #[test]
    fn seal_reads_estimatesmartfee_and_not_the_previous_block_p10() {
        let source = include_str!("bitcoin_rpc.rs");
        assert!(source.contains("estimatesmartfee"));
        assert!(source.contains("getmempoolinfo"));
        assert!(source.contains("getrawmempool"));
        assert!(source.contains("next_block_min_from_mempool"));
        let retired = format!("{}-{}", "prev-block", "p10");
        assert!(!source.contains(&retired));
        assert!(source.contains("FEE_ESTIMATE_MODE"));
        assert!(source.contains("\"replaceable\": false"));
    }

    fn no_confirmed() -> utxo::SelectError {
        utxo::SelectError::NoConfirmed {
            min_conf: 1,
            confirmed: 0,
            reserve_min: 2,
            reserve_max: 3,
        }
    }

    fn view(estimate: Option<f64>) -> fee::NodeFeeView {
        fee::NodeFeeView {
            estimate_sat_vb: estimate,
            node_estimate_sat_vb: None,
            recent_min_sat_vb: None,
            relay_min_sat_vb: 0.1,
            mempool_min_sat_vb: 0.1,
        }
    }

    #[test]
    fn missing_confirmed_utxo_defers_and_a_wallet_lock_is_not() {
        let missing = no_confirmed().defer_message();
        assert!(missing.contains("deferred"), "{missing}");
        assert!(missing.contains("no tree"), "{missing}");
        assert!(missing.contains("not replaced"), "{missing}");
        let pre = fee::pre_broadcast_with_floor(&view(Some(0.40)), 1.1, 1.5).unwrap();
        let deferred = clearance_after_fee(pre, Err(no_confirmed()), Some(101));
        match deferred {
            SealClearance::Defer(deferral) => {
                assert_eq!(deferral.reason, fee::DEFER_REASON_NO_CONFIRMED_UTXO);
                assert_eq!(deferral.decision_height, Some(101));
                assert!(
                    deferral.message.contains("confirmed=0"),
                    "{}",
                    deferral.message
                );
            }
            other => panic!("missing input must defer, got {other:?}"),
        }
        assert_eq!(
            plant_skip_reason("walletprocesspsbt RPC error -13: wallet locked"),
            SkipReason::Other
        );
        assert_eq!(
            plant_skip_reason("walletcreatefundedpsbt RPC error -4: Insufficient funds"),
            SkipReason::Fee
        );
    }

    #[test]
    fn fee_policy_defaults_to_dynamic_with_a_one_point_one_sat_cap() {
        let config = |expected_network: &str,
                      fee_dynamic,
                      fee_rate_sat_vb,
                      fee_cap_sat_vb|
         -> BitcoinRpcConfig {
            BitcoinRpcConfig {
                rpc_url: Url::parse("http://127.0.0.1:38332").unwrap(),
                wallet_name: "dedicated-signet-wallet".to_owned(),
                expected_network: expected_network.to_owned(),
                auth: RpcAuth {
                    username: "user".to_owned(),
                    password: "password".to_owned(),
                },
                fee_rate_sat_vb,
                fee_dynamic,
                fee_cap_sat_vb,
                fee_multiplier: None,
                utxo_policy: UtxoPolicy::standard(),
                timeout: Duration::from_secs(1),
            }
        };

        let default = BitcoinRpc::new(config("signet", None, None, None)).unwrap();
        assert_eq!(
            default.fee_policy,
            fee::FeePolicy::Dynamic {
                cap_sat_vb: fee::DEFAULT_FEE_CAP_SAT_VB,
                multiplier: fee::DEFAULT_FEE_MULTIPLIER,
            }
        );
        assert_eq!(fee::DEFAULT_FEE_CAP_SAT_VB, 1.1);
        assert_eq!(fee::DEFAULT_FEE_MULTIPLIER, 1.5);
        let label = default.fee_log_label();
        assert!(label.contains("cap=1.1"), "{label}");
        assert!(label.contains("multiplier=1.5"), "{label}");
        assert!(label.contains("fire_below=1.1"), "{label}");
        assert!(label.contains("next-block"), "{label}");

        let named = BitcoinRpc::new(config("signet", None, None, Some(1.1))).unwrap();
        assert_eq!(
            named.fee_policy,
            fee::FeePolicy::Dynamic {
                cap_sat_vb: 1.1,
                multiplier: 1.5
            }
        );
        let above = BitcoinRpc::new(config("signet", None, None, Some(1.2)))
            .err()
            .unwrap();
        assert!(above.0.contains("BARI_FEE_CAP_SAT_VB"), "{above:?}");
        assert!(above.0.contains("1.1"), "{above:?}");

        let synthetic = BitcoinRpc::new(config("synthetic", None, None, None)).unwrap();
        assert_eq!(synthetic.expected_network, "synthetic");

        let mainnet = BitcoinRpc::new(config("main", None, None, None)).unwrap();
        assert_eq!(mainnet.expected_network, "main");

        let error = BitcoinRpc::new(config("", None, None, None)).err().unwrap();
        assert!(error.0.contains("BARI_BITCOIN_NETWORK is required"));
    }

    #[test]
    fn high_minimum_holds_without_asking_the_selector() {
        let pre = fee::pre_broadcast_with_floor(&view(Some(2.2)), 1.1, 1.5).unwrap();
        let clearance = clearance_after_fee(pre, Err(no_confirmed()), Some(103));
        match clearance {
            SealClearance::Defer(deferral) => {
                assert_eq!(deferral.reason, fee::HOLD_REASON_NEXT_BLOCK_MIN);
                assert_eq!(deferral.decision_height, Some(103));
                assert_eq!(deferral.next_block_min_sat_vb, Some(2.2));
                assert_eq!(deferral.cap_sat_vb, Some(1.1));
                assert_eq!(deferral.message, "holding, min 2.2 is above the 1.1 cap");
            }
            other => panic!("min 2.2 must hold before the selector, got {other:?}"),
        }
    }

    #[test]
    fn min_under_the_cap_with_an_input_broadcasts_at_or_under_the_cap() {
        let pre = fee::pre_broadcast_with_floor(&view(Some(0.60)), 1.1, 1.5).unwrap();
        match clearance_after_fee(pre, Ok(()), Some(104)) {
            SealClearance::PlantAt {
                rate_sat_vb,
                cap_sat_vb,
                next_block_min_sat_vb,
                ..
            } => {
                assert_eq!(rate_sat_vb, 0.90);
                assert_eq!(cap_sat_vb, Some(1.1));
                assert_eq!(next_block_min_sat_vb, Some(0.60));
                assert!(!fee::rate_exceeds_cap(rate_sat_vb, 1.1));
            }
            other => panic!("min 0.60 with an input must broadcast, got {other:?}"),
        }
    }

    #[test]
    fn floor_above_the_cap_is_not_a_broadcast() {
        let high_floor = fee::NodeFeeView {
            estimate_sat_vb: Some(0.40),
            node_estimate_sat_vb: Some(1.12),
            recent_min_sat_vb: None,
            relay_min_sat_vb: 0.1,
            mempool_min_sat_vb: 1.2,
        };
        let pre = fee::pre_broadcast_with_floor(&high_floor, 1.1, 1.5).unwrap();
        match clearance_after_fee(pre, Ok(()), Some(105)) {
            SealClearance::Defer(deferral) => {
                assert_eq!(deferral.reason, fee::HOLD_REASON_CAP);
                assert_eq!(deferral.next_block_min_sat_vb, Some(0.40));
                assert!(deferral.message.contains("holding"), "{}", deferral.message);
            }
            other => panic!("floor above the cap must not broadcast, got {other:?}"),
        }
    }

    #[test]
    fn wallet_parse_splits_listunspent_at_min_conf_and_drops_addresses() {
        let tx = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let unspent = json!([
            {"txid": tx, "vout": 0, "amount": 0.00000100, "confirmations": 3, "spendable": true, "safe": true},
            {"txid": tx, "vout": 1, "amount": 0.00000050, "confirmations": 1, "spendable": true, "safe": true},
            {"txid": tx, "vout": 2, "amount": 0.00000025, "confirmations": 0, "spendable": true, "safe": true, "address": "bc1qhidden"}
        ]);
        let wallet = parse_chain_wallet(&unspent, &UtxoPolicy::standard()).unwrap();
        assert_eq!(wallet.confirmed_sat, 150);
        assert_eq!(wallet.unconfirmed_sat, 25);
        assert_eq!(wallet.confirmed_sat + wallet.unconfirmed_sat, 175);
        assert_eq!(wallet.confirmed_coins, 2);
        assert_eq!(wallet.unconfirmed_coins, 1);
        assert_eq!(wallet.min_conf, 1);
        assert_eq!(wallet.coins.len(), 3);
    }

    /// A `getrawmempool true` body that takes longer than the old 2 s page
    /// cutoff. The page used to report that as "node not reachable" on a node
    /// that had just broadcast. The quote budget waits for the body.
    #[test]
    fn a_slow_mempool_read_still_returns_the_next_block_minimum() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            // getmempoolinfo, then getrawmempool, then estimatesmartfee.
            // The mempool accept waits longer than the retired 2 s page
            // cutoff. The client is blocked in the read for that whole wait.
            for call in 0..3 {
                if call == 1 {
                    std::thread::sleep(Duration::from_millis(2_500));
                }
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buf = [0u8; 2048];
                loop {
                    let read = std::io::Read::read(&mut stream, &mut buf).unwrap_or(0);
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buf[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                let text = String::from_utf8_lossy(&request);
                let result = if text.contains("getrawmempool") {
                    json!({
                        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa": {
                            "vsize": 200,
                            "ancestorsize": 200,
                            "fees": {"ancestor": 0.00000082},
                            "depends": []
                        }
                    })
                } else if text.contains("estimatesmartfee") {
                    json!({"feerate": 0.00001012, "blocks": 1})
                } else {
                    json!({"minrelaytxfee": 0.00000100, "mempoolminfee": 0.00000100})
                };
                let payload = serde_json::to_vec(&json!({
                    "result": result,
                    "error": null,
                    "id": concat!("bari-v", env!("CARGO_PKG_VERSION"))
                }))
                .unwrap();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                );
                std::io::Write::write_all(&mut stream, response.as_bytes()).unwrap();
                std::io::Write::write_all(&mut stream, &payload).unwrap();
            }
        });
        let rpc = BitcoinRpc::new(BitcoinRpcConfig {
            rpc_url: Url::parse(&format!("http://{address}/")).unwrap(),
            wallet_name: "dedicated-signet-wallet".to_owned(),
            expected_network: "main".to_owned(),
            auth: RpcAuth {
                username: "user".to_owned(),
                password: "password".to_owned(),
            },
            fee_rate_sat_vb: None,
            fee_dynamic: None,
            fee_cap_sat_vb: None,
            fee_multiplier: None,
            utxo_policy: UtxoPolicy::standard(),
            // The plant timeout can be short. The page quote does not use it.
            timeout: Duration::from_secs(1),
        })
        .unwrap();
        let started = std::time::Instant::now();
        let quote = rpc.fee_quote().unwrap().expect("a quote");
        assert!(
            started.elapsed() >= Duration::from_millis(2_500),
            "the quote waits out a mempool body slower than the old 2 s cutoff"
        );
        assert!(started.elapsed() < QUOTE_READ_BUDGET);
        // 82 sat / 200 vB = 0.41 sat/vB, the rate a live tree was sent at.
        assert_eq!(quote.min_sat_vb, Some(0.41));
        assert_eq!(quote.bid_sat_vb, Some(0.62));
        assert!(quote.hold_reason.is_empty(), "{}", quote.hold_reason);
        // A 2 s page cutoff would have failed this read.
        assert!(OPS_READ_TIMEOUT >= QUOTE_READ_BUDGET);
        assert!(OPS_READ_TIMEOUT > Duration::from_secs(2));
        server.join().unwrap();
    }

    #[test]
    fn exact_output_search_rejects_missing_or_duplicate_scripts() {
        let script = hex::encode(commitment_script(&[0x11; 32]));
        let one = json!({"vout":[
            {"n":0,"value":0.1,"scriptPubKey":{"hex":"0014"}},
            {"n":1,"value":0.0,"scriptPubKey":{"hex":script}},
        ]});
        assert_eq!(find_exact_zero_value_output(&one, &script).unwrap(), 1);

        let duplicate = json!({"vout":[
            {"n":1,"value":0.0,"scriptPubKey":{"hex":script}},
            {"n":2,"value":0.0,"scriptPubKey":{"hex":script}},
        ]});
        assert!(find_exact_zero_value_output(&duplicate, &script).is_err());
    }
}
